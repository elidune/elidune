//! Client-IP resolution and auth attempt limits.
//!
//! IP quotas use `tower_governor`, which keeps its buckets in memory. The
//! per-identifier login/reset window uses the same process-local storage.
//! Redis is used for 2FA codes, the Z39.50 cache, and task status, not for
//! request quotas. A library deployment runs one API process (the binary or
//! the all-in-one image), so an in-memory window matches the existing limiter
//! and is cleared by `DELETE /auth/rate-limit/{login}` on that process.
//! Several API replicas would each keep their own window.

use std::collections::{HashMap, VecDeque};
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::extract::ConnectInfo;
use axum::http::{HeaderMap, Request};
use ipnet::IpNet;
use tower_governor::key_extractor::KeyExtractor;
use tower_governor::GovernorError;

use crate::models::user::UserClaims;

/// Requests-per-minute expressed as the governor replenish interval.
///
/// `tower_governor`'s `per_second(n)` is "one token every n seconds", not
/// "n requests per second". Callers pass a count per minute; this returns the
/// matching one-token period (at least 1 ms).
#[must_use]
pub fn replenish_period(per_minute: u64) -> Duration {
    let per_minute = per_minute.max(1);
    Duration::from_millis((60_000 / per_minute).max(1))
}

/// Parse `server.trusted_proxies`: comma-separated CIDRs or single addresses.
/// Empty / unset yields an empty list (forwarding headers are ignored).
pub fn parse_trusted_proxies(raw: Option<&str>) -> Result<Vec<IpNet>, String> {
    let Some(raw) = raw.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(Vec::new());
    };
    let mut nets = Vec::new();
    for part in raw.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if let Ok(net) = part.parse::<IpNet>() {
            nets.push(net);
            continue;
        }
        if let Ok(ip) = part.parse::<IpAddr>() {
            nets.push(IpNet::from(ip));
            continue;
        }
        return Err(format!("invalid CIDR in server.trusted_proxies ({part}); expected a CIDR such as 10.0.0.0/8 or a single IP"));
    }
    Ok(nets)
}

fn ip_is_trusted(ip: IpAddr, trusted: &[IpNet]) -> bool {
    trusted.iter().any(|net| net.contains(&ip))
}

/// Trim quotes, brackets, and an optional port. `unknown` / obfuscated values are skipped.
fn parse_ip_token(raw: &str) -> Option<IpAddr> {
    let s = raw.trim().trim_matches('"').trim();
    if s.is_empty() || s.eq_ignore_ascii_case("unknown") || s.starts_with('_') {
        return None;
    }
    if let Some(rest) = s.strip_prefix('[') {
        let end = rest.find(']')?;
        return rest[..end].parse().ok();
    }
    if let Some((host, _port)) = s.rsplit_once(':') {
        if host.contains('.') && !host.contains(':') {
            return host.parse().ok();
        }
    }
    s.parse().ok()
}

fn header_chain(headers: &HeaderMap, name: &str) -> Vec<IpAddr> {
    let mut ips = Vec::new();
    for value in headers.get_all(name) {
        let Ok(text) = value.to_str() else {
            continue;
        };
        for part in text.split(',') {
            if let Some(ip) = parse_ip_token(part) {
                ips.push(ip);
            }
        }
    }
    ips
}

fn split_comma_outside_quotes(input: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut quoted = false;
    for (idx, ch) in input.char_indices() {
        match ch {
            '"' => quoted = !quoted,
            ',' if !quoted => {
                parts.push(input[start..idx].trim());
                start = idx + 1;
            }
            _ => {}
        }
    }
    let tail = input[start..].trim();
    if !tail.is_empty() {
        parts.push(tail);
    }
    parts
}

fn forwarded_chain(headers: &HeaderMap) -> Vec<IpAddr> {
    let mut ips = Vec::new();
    for value in headers.get_all("forwarded") {
        let Ok(text) = value.to_str() else {
            continue;
        };
        for element in split_comma_outside_quotes(text) {
            for param in element.split(';') {
                let Some((key, raw)) = param.trim().split_once('=') else {
                    continue;
                };
                if key.trim().eq_ignore_ascii_case("for") {
                    if let Some(ip) = parse_ip_token(raw.trim()) {
                        ips.push(ip);
                    }
                }
            }
        }
    }
    ips
}

/// Right-to-left: skip trusted hops, return the first untrusted address.
/// If every hop is trusted, the left-most address is the client.
fn client_from_chain(ips: &[IpAddr], trusted: &[IpNet]) -> Option<IpAddr> {
    if ips.is_empty() {
        return None;
    }
    for ip in ips.iter().rev() {
        if !ip_is_trusted(*ip, trusted) {
            return Some(*ip);
        }
    }
    ips.first().copied()
}

/// Client address for rate-limit keys.
///
/// Forwarding headers are read only when `peer` is inside `trusted`. The
/// `X-Forwarded-For` chain is walked right-to-left. `X-Real-IP`, then
/// `Forwarded`, are fallbacks. An empty trusted list, or an untrusted peer,
/// ignores those headers and returns the peer.
#[must_use]
pub fn resolve_client_ip(
    headers: &HeaderMap,
    peer: Option<IpAddr>,
    trusted: &[IpNet],
) -> Option<IpAddr> {
    let peer = peer?;
    if trusted.is_empty() || !ip_is_trusted(peer, trusted) {
        return Some(peer);
    }
    if let Some(ip) = client_from_chain(&header_chain(headers, "x-forwarded-for"), trusted) {
        return Some(ip);
    }
    if let Some(ip) = header_chain(headers, "x-real-ip")
        .into_iter()
        .find(|ip| !ip_is_trusted(*ip, trusted))
    {
        return Some(ip);
    }
    if let Some(ip) = client_from_chain(&forwarded_chain(headers), trusted) {
        return Some(ip);
    }
    Some(peer)
}

fn peer_ip<T>(req: &Request<T>) -> Option<IpAddr> {
    req.extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|info| info.0.ip())
}

/// Rate-limit key: peer IP, or the client behind a trusted proxy.
#[derive(Clone, Debug)]
pub struct TrustedProxyKeyExtractor {
    pub trusted: Arc<Vec<IpNet>>,
}

impl KeyExtractor for TrustedProxyKeyExtractor {
    type Key = IpAddr;

    fn extract<T>(&self, req: &Request<T>) -> Result<Self::Key, GovernorError> {
        resolve_client_ip(req.headers(), peer_ip(req), &self.trusted)
            .ok_or(GovernorError::UnableToExtractKey)
    }
}

/// Authenticated auth routes (`/auth/me`, `/auth/profile`, …).
///
/// A verified bearer token keys the bucket by user id. Anything else falls
/// back to the client IP so anonymous calls still have a ceiling.
#[derive(Clone, Debug)]
pub struct SessionKeyExtractor {
    pub trusted: Arc<Vec<IpNet>>,
    pub jwt_secret: Arc<str>,
}

fn bearer_user_id(headers: &HeaderMap, secret: &str) -> Option<i64> {
    let value = headers
        .get(axum::http::header::AUTHORIZATION)?
        .to_str()
        .ok()?;
    let token = value.strip_prefix("Bearer ")?;
    UserClaims::from_token(token, secret)
        .ok()
        .map(|claims| claims.user_id)
}

impl KeyExtractor for SessionKeyExtractor {
    type Key = String;

    fn extract<T>(&self, req: &Request<T>) -> Result<Self::Key, GovernorError> {
        if let Some(user_id) = bearer_user_id(req.headers(), &self.jwt_secret) {
            return Ok(format!("user:{user_id}"));
        }
        resolve_client_ip(req.headers(), peer_ip(req), &self.trusted)
            .map(|ip| format!("ip:{ip}"))
            .ok_or(GovernorError::UnableToExtractKey)
    }
}

/// Trimmed, lowercased login or email. Empty means "do not key".
#[must_use]
pub fn normalize_identifier(raw: &str) -> String {
    raw.trim().to_lowercase()
}

/// In-memory sliding window of failed login / password-reset attempts.
///
/// `max_failures` attempts are allowed inside `window`. The next attempt is
/// rejected until the oldest failure leaves the window. This is a temporary
/// block, not an account lock: [`AuthAttemptLimiter::clear`] drops it immediately.
#[derive(Debug)]
pub struct AuthAttemptLimiter {
    max_failures: usize,
    window: Duration,
    failures: Mutex<HashMap<String, VecDeque<Instant>>>,
}

impl AuthAttemptLimiter {
    #[must_use]
    pub fn new(max_failures: u32, window: Duration) -> Self {
        Self {
            max_failures: usize::try_from(max_failures.max(1)).unwrap_or(usize::MAX),
            window: if window.is_zero() {
                Duration::from_secs(1)
            } else {
                window
            },
            failures: Mutex::new(HashMap::new()),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, VecDeque<Instant>>> {
        self.failures.lock().unwrap_or_else(|err| err.into_inner())
    }

    fn prune_stamps(stamps: &mut VecDeque<Instant>, window: Duration) {
        let now = Instant::now();
        while stamps
            .front()
            .is_some_and(|stamp| now.saturating_duration_since(*stamp) >= window)
        {
            stamps.pop_front();
        }
    }

    /// Seconds until another attempt is allowed, when the window is full.
    #[must_use]
    pub fn retry_after(&self, key: &str) -> Option<u64> {
        if key.is_empty() {
            return None;
        }
        let mut guard = self.lock();
        let stamps = guard.get_mut(key)?;
        Self::prune_stamps(stamps, self.window);
        if stamps.len() < self.max_failures {
            if stamps.is_empty() {
                guard.remove(key);
            }
            return None;
        }
        let oldest = *stamps.front()?;
        let remaining = self.window.saturating_sub(oldest.elapsed());
        Some(remaining.as_secs().max(1))
    }

    pub fn record_failure(&self, key: &str) {
        if key.is_empty() {
            return;
        }
        let mut guard = self.lock();
        let stamps = guard.entry(key.to_owned()).or_default();
        Self::prune_stamps(stamps, self.window);
        stamps.push_back(Instant::now());
    }

    pub fn clear(&self, key: &str) {
        if key.is_empty() {
            return;
        }
        self.lock().remove(key);
    }

    /// Drop expired keys. Called from the governor cleanup thread.
    pub fn prune(&self) {
        let mut guard = self.lock();
        let window = self.window;
        guard.retain(|_, stamps| {
            Self::prune_stamps(stamps, window);
            !stamps.is_empty()
        });
    }
}

/// Password-reset requests.
///
/// A resolved account is counted on `user:{id}`, so its login and email share
/// one window. An identifier that matches no account stays on
/// `reset:{normalized identifier}`. Separate from [`AuthAttemptLimiter`] so
/// requesting a reset link cannot fill the sign-in failure window. The HTTP
/// handler answers with the same generic 200 once the quota is full; this
/// type only tracks the window.
#[derive(Debug)]
pub struct PasswordResetAttemptLimiter {
    inner: AuthAttemptLimiter,
}

impl PasswordResetAttemptLimiter {
    #[must_use]
    pub fn new(max_requests: u32, window: Duration) -> Self {
        Self {
            inner: AuthAttemptLimiter::new(max_requests, window),
        }
    }

    fn identifier_key(identifier: &str) -> String {
        let id = normalize_identifier(identifier);
        if id.is_empty() {
            String::new()
        } else {
            format!("reset:{id}")
        }
    }

    fn user_key(user_id: i64) -> String {
        format!("user:{user_id}")
    }

    /// Seconds until another reset request for an unknown identifier is inside the quota.
    #[must_use]
    pub fn retry_after(&self, identifier: &str) -> Option<u64> {
        self.inner.retry_after(&Self::identifier_key(identifier))
    }

    pub fn record(&self, identifier: &str) {
        self.inner.record_failure(&Self::identifier_key(identifier));
    }

    pub fn clear(&self, identifier: &str) {
        self.inner.clear(&Self::identifier_key(identifier));
    }

    /// Seconds until another reset request for this account is inside the quota.
    #[must_use]
    pub fn retry_after_user(&self, user_id: i64) -> Option<u64> {
        self.inner.retry_after(&Self::user_key(user_id))
    }

    pub fn record_user(&self, user_id: i64) {
        self.inner.record_failure(&Self::user_key(user_id));
    }

    pub fn clear_user(&self, user_id: i64) {
        self.inner.clear(&Self::user_key(user_id));
    }

    pub fn prune(&self) {
        self.inner.prune();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;
    use std::net::Ipv6Addr;

    fn header_map(pairs: &[(&'static str, &str)]) -> HeaderMap {
        let mut headers = HeaderMap::new();
        for (name, value) in pairs {
            headers.append(*name, HeaderValue::from_str(value).expect("header"));
        }
        headers
    }

    fn request(peer: Option<IpAddr>, pairs: &[(&'static str, &str)]) -> Request<()> {
        let mut req = Request::builder().uri("/").body(()).expect("request");
        if let Some(peer) = peer {
            req.extensions_mut()
                .insert(ConnectInfo(SocketAddr::new(peer, 443)));
        }
        for (name, value) in pairs {
            req.headers_mut()
                .insert(*name, HeaderValue::from_str(value).expect("header"));
        }
        req
    }

    fn nets(raw: &str) -> Arc<Vec<IpNet>> {
        Arc::new(parse_trusted_proxies(Some(raw)).expect("cidrs"))
    }

    #[test]
    fn unset_trusted_proxies_ignore_forwarding_headers() {
        let peer: IpAddr = "203.0.113.8".parse().unwrap();
        let headers = header_map(&[
            ("x-forwarded-for", "198.51.100.10, 10.0.0.1"),
            ("x-real-ip", "198.51.100.11"),
            ("forwarded", "for=198.51.100.12"),
        ]);
        assert_eq!(resolve_client_ip(&headers, Some(peer), &[]), Some(peer));
        let extractor = TrustedProxyKeyExtractor {
            trusted: Arc::new(Vec::new()),
        };
        assert_eq!(
            extractor
                .extract(&request(Some(peer), &[("x-forwarded-for", "1.2.3.4"),]))
                .unwrap(),
            peer
        );
    }

    #[test]
    fn untrusted_peer_ignores_spoofed_xff() {
        let peer: IpAddr = "203.0.113.9".parse().unwrap();
        let trusted = nets("10.0.0.0/8, 127.0.0.1/32");
        let headers = header_map(&[("x-forwarded-for", "198.51.100.50")]);
        assert_eq!(
            resolve_client_ip(&headers, Some(peer), &trusted),
            Some(peer)
        );
        let extractor = TrustedProxyKeyExtractor { trusted };
        assert_eq!(
            extractor
                .extract(&request(Some(peer), &[("x-forwarded-for", "8.8.8.8")]))
                .unwrap(),
            peer
        );
    }

    #[test]
    fn trusted_peer_walks_xff_right_to_left_and_skips_spoofed_leftmost() {
        let peer: IpAddr = "10.1.2.3".parse().unwrap();
        let trusted = nets("10.0.0.0/8, 192.168.0.0/16");
        // Left-most is a spoof. The first untrusted hop from the right is the client.
        let headers = header_map(&[(
            "x-forwarded-for",
            "1.2.3.4, 203.0.113.50, 192.168.1.5, 10.0.0.4",
        )]);
        let client: IpAddr = "203.0.113.50".parse().unwrap();
        assert_eq!(
            resolve_client_ip(&headers, Some(peer), &trusted),
            Some(client)
        );
    }

    #[test]
    fn ipv6_multihop_and_bracketed_form() {
        let peer: IpAddr = "fd00::1".parse().unwrap();
        let trusted = nets("fd00::/8");
        let headers = header_map(&[("x-forwarded-for", "[2001:db8::abcd], fd00::2")]);
        let client: IpAddr = "2001:db8::abcd".parse().unwrap();
        assert_eq!(
            resolve_client_ip(&headers, Some(peer), &trusted),
            Some(client)
        );

        let spoofed_peer: IpAddr = Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 0x50).into();
        let spoofed = header_map(&[("x-forwarded-for", "198.51.100.1")]);
        assert_eq!(
            resolve_client_ip(&spoofed, Some(spoofed_peer), &trusted),
            Some(spoofed_peer)
        );
    }

    #[test]
    fn x_real_ip_and_forwarded_used_only_for_trusted_peer() {
        let peer: IpAddr = "127.0.0.1".parse().unwrap();
        let trusted = nets("127.0.0.1");
        let real: IpAddr = "198.51.100.22".parse().unwrap();
        assert_eq!(
            resolve_client_ip(
                &header_map(&[("x-real-ip", "198.51.100.22")]),
                Some(peer),
                &trusted
            ),
            Some(real)
        );
        let via_forwarded: IpAddr = "203.0.113.7".parse().unwrap();
        assert_eq!(
            resolve_client_ip(
                &header_map(&[("forwarded", "for=203.0.113.7;proto=https, for=127.0.0.1")]),
                Some(peer),
                &trusted
            ),
            Some(via_forwarded)
        );
        let outsider: IpAddr = "198.51.100.9".parse().unwrap();
        assert_eq!(
            resolve_client_ip(
                &header_map(&[
                    ("x-real-ip", "203.0.113.1"),
                    ("forwarded", "for=203.0.113.2")
                ]),
                Some(outsider),
                &trusted
            ),
            Some(outsider)
        );
    }

    #[test]
    fn xff_wins_over_x_real_ip_when_both_present() {
        let peer: IpAddr = "10.0.0.8".parse().unwrap();
        let trusted = nets("10.0.0.0/8");
        let client: IpAddr = "203.0.113.5".parse().unwrap();
        let headers = header_map(&[
            ("x-forwarded-for", "203.0.113.5, 10.0.0.2"),
            ("x-real-ip", "198.51.100.9"),
        ]);
        assert_eq!(
            resolve_client_ip(&headers, Some(peer), &trusted),
            Some(client)
        );
    }

    #[test]
    fn all_trusted_hops_fall_back_to_leftmost() {
        let peer: IpAddr = "10.0.0.3".parse().unwrap();
        let trusted = nets("10.0.0.0/8");
        let headers = header_map(&[("x-forwarded-for", "10.0.0.2, 10.0.0.1")]);
        let leftmost: IpAddr = "10.0.0.2".parse().unwrap();
        assert_eq!(
            resolve_client_ip(&headers, Some(peer), &trusted),
            Some(leftmost)
        );
    }

    #[test]
    fn missing_connect_info_cannot_extract_a_key() {
        let extractor = TrustedProxyKeyExtractor {
            trusted: nets("127.0.0.1/32"),
        };
        let err = extractor
            .extract(&request(None, &[("x-forwarded-for", "203.0.113.5")]))
            .unwrap_err();
        assert!(matches!(err, GovernorError::UnableToExtractKey));
    }

    #[test]
    fn invalid_cidr_is_rejected_and_bare_ip_is_a_host_route() {
        assert!(parse_trusted_proxies(Some("not-a-cidr")).is_err());
        assert!(parse_trusted_proxies(None).unwrap().is_empty());
        assert!(parse_trusted_proxies(Some("  ")).unwrap().is_empty());
        let nets = parse_trusted_proxies(Some("127.0.0.1, ::1")).unwrap();
        assert_eq!(nets.len(), 2);
        assert!(nets[0].contains(&"127.0.0.1".parse::<IpAddr>().unwrap()));
        assert!(!nets[0].contains(&"127.0.0.2".parse::<IpAddr>().unwrap()));
    }

    #[test]
    fn identifier_window_blocks_one_key_and_clear_resets_it() {
        let limiter = AuthAttemptLimiter::new(3, Duration::from_secs(900));
        let victim = normalize_identifier("  Alice@Example.COM ");
        assert_eq!(victim, "alice@example.com");
        let other = normalize_identifier("bob");
        for _ in 0..3 {
            assert_eq!(limiter.retry_after(&victim), None);
            limiter.record_failure(&victim);
        }
        let retry = limiter.retry_after(&victim).expect("blocked");
        assert!((1..=900).contains(&retry));
        assert_eq!(limiter.retry_after(&other), None);
        limiter.record_failure(&other);
        limiter.clear(&victim);
        assert_eq!(limiter.retry_after(&victim), None);
        limiter.clear("");
        limiter.prune();
    }

    #[test]
    fn reset_requests_use_their_own_counter() {
        let login = AuthAttemptLimiter::new(3, Duration::from_secs(900));
        let resets = PasswordResetAttemptLimiter::new(3, Duration::from_secs(3600));
        for _ in 0..3 {
            assert_eq!(resets.retry_after(" Ada@Example.com "), None);
            resets.record(" Ada@Example.com ");
        }
        assert!(resets.retry_after("ada@example.com").is_some());
        assert_eq!(login.retry_after("ada@example.com"), None);
        resets.clear("ADA@example.com");
        assert_eq!(resets.retry_after("ada@example.com"), None);
    }

    #[test]
    fn resolved_account_quota_is_separate_from_unknown_identifiers() {
        let resets = PasswordResetAttemptLimiter::new(3, Duration::from_secs(3600));
        for _ in 0..3 {
            assert_eq!(resets.retry_after_user(42), None);
            resets.record_user(42);
        }
        assert!(resets.retry_after_user(42).is_some());
        assert_eq!(resets.retry_after("ada@example.com"), None);
        assert_eq!(resets.retry_after_user(7), None);
        for _ in 0..3 {
            resets.record("ada@example.com");
        }
        assert!(resets.retry_after("ada@example.com").is_some());
        resets.clear_user(42);
        assert_eq!(resets.retry_after_user(42), None);
        assert!(resets.retry_after("ada@example.com").is_some());
    }

    #[test]
    fn replenish_period_matches_requests_per_minute() {
        assert_eq!(replenish_period(30), Duration::from_millis(2_000));
        assert_eq!(replenish_period(60_000), Duration::from_millis(1));
        assert_eq!(replenish_period(0), Duration::from_millis(60_000));
    }
}
