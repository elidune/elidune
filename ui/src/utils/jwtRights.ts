/**
 * Read JWT `rights.*` claims for UI routing / action affordances.
 * No signature verification — display / routing only; the API remains authoritative.
 */

export type RightsDomain =
  | 'items'
  | 'users'
  | 'loans'
  | 'holds'
  | 'settings'
  | 'events'
  | 'acquisitions';

/** Normalized levels matching server `Rights` (lowercase JSON). */
export type RightsLevel = 'none' | 'own' | 'read' | 'write';

const DOMAIN_CLAIM_KEYS: Record<RightsDomain, readonly string[]> = {
  items: ['itemsRights'],
  users: ['usersRights'],
  loans: ['loansRights'],
  holds: ['holdsRights', 'borrowsRights'],
  settings: ['settingsRights'],
  events: ['eventsRights'],
  acquisitions: ['acquisitionsRights'],
};

function decodeAccessTokenPayload(token: string): Record<string, unknown> | null {
  const parts = token.trim().split('.');
  if (parts.length < 2) return null;
  try {
    let b64 = parts[1].replace(/-/g, '+').replace(/_/g, '/');
    const pad = (4 - (b64.length % 4)) % 4;
    if (pad) b64 += '='.repeat(pad);
    const json = atob(b64);
    const data: unknown = JSON.parse(json);
    return data && typeof data === 'object' ? (data as Record<string, unknown>) : null;
  } catch {
    return null;
  }
}

/** Rank for `>= read` / `>= write` checks (`own` is below `read`). */
export function rightsRank(level: string | null | undefined): number {
  switch ((level ?? '').trim().toLowerCase()) {
    case 'none':
    case 'n':
      return 0;
    case 'own':
    case 'o':
      return 1;
    case 'read':
    case 'r':
      return 2;
    case 'write':
    case 'w':
      return 3;
    default:
      return -1;
  }
}

export function normalizeRightsLevel(raw: unknown): RightsLevel | null {
  const s = raw != null ? String(raw).trim().toLowerCase() : '';
  if (s === 'n' || s === 'none') return 'none';
  if (s === 'o' || s === 'own') return 'own';
  if (s === 'r' || s === 'read') return 'read';
  if (s === 'w' || s === 'write') return 'write';
  return null;
}

export function hasMinRightsLevel(
  actual: string | null | undefined,
  required: RightsLevel,
): boolean {
  const a = rightsRank(actual);
  const r = rightsRank(required);
  return a >= 0 && r >= 0 && a >= r;
}

function pickDomainRaw(bag: Record<string, unknown> | null | undefined, domain: RightsDomain): unknown {
  if (!bag) return undefined;
  for (const key of DOMAIN_CLAIM_KEYS[domain]) {
    if (bag[key] != null && String(bag[key]).trim() !== '') return bag[key];
  }
  return undefined;
}

/** Nested `rights` object from a JWT payload, or null. */
export function readRightsBagFromJwt(
  accessToken: string | null | undefined,
): Record<string, unknown> | null {
  if (!accessToken?.trim()) return null;
  const payload = decodeAccessTokenPayload(accessToken);
  if (!payload) return null;
  const rights = payload.rights;
  if (rights && typeof rights === 'object') {
    return rights as Record<string, unknown>;
  }
  return null;
}

export function readDomainRightFromJwt(
  accessToken: string | null | undefined,
  domain: RightsDomain,
): RightsLevel | null {
  if (!accessToken?.trim()) return null;
  const payload = decodeAccessTokenPayload(accessToken);
  if (!payload) return null;
  const nested = readRightsBagFromJwt(accessToken);
  const fromNested = normalizeRightsLevel(pickDomainRaw(nested, domain));
  if (fromNested) return fromNested;
  // Flat legacy claims on the payload root (rare)
  return normalizeRightsLevel(pickDomainRaw(payload, domain));
}

/** @deprecated Prefer readDomainRightFromJwt(token, 'holds') — kept for call-site stability. */
export function readHoldsRightsFromJwt(accessToken: string | null | undefined): string | null {
  return readDomainRightFromJwt(accessToken, 'holds');
}

function normalizeAcquisitionsRightsValue(raw: unknown): 'n' | 'r' | 'w' | null {
  const level = normalizeRightsLevel(raw);
  if (level === 'none') return 'n';
  if (level === 'read') return 'r';
  if (level === 'write') return 'w';
  return null;
}

export function readAcquisitionsRightsFromJwt(
  accessToken: string | null | undefined,
): 'n' | 'r' | 'w' | null {
  return normalizeAcquisitionsRightsValue(readDomainRightFromJwt(accessToken, 'acquisitions'));
}
