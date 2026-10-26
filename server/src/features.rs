//! Instance-level feature flags (optional modules for small libraries).
//!
//! Defaults are all enabled. Operators can disable modules in TOML (`[features]`),
//! via `ELIDUNE_FEATURES__*` env vars, or at runtime through `PUT /admin/config/features`
//! when `overridable = true`. The effective set is exposed on `GET /library-info`
//! and enforced on the matching API routers.

use axum::{
    extract::State,
    http::Request,
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    error::{AppError, AppResult},
    AppState,
};

fn default_true() -> bool {
    true
}

fn default_overridable() -> bool {
    true
}

/// Runtime feature toggles for optional LMS modules.
///
/// TOML and `PUT /admin/config/features` use **snake_case** (same as reminders/holds).
/// Public `library-info` exposes [`EnabledFeatures`] in camelCase.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
pub struct FeaturesConfig {
    /// When true, admins may override this section via the DB `settings` table.
    #[serde(default = "default_overridable")]
    pub overridable: bool,
    /// Optional preset name from TOML (`essential`). Not exposed on `library-info`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<String>)]
    pub profile: Option<String>,
    #[serde(default = "default_true")]
    pub acquisitions: bool,
    #[serde(default = "default_true")]
    pub purchase_suggestions: bool,
    #[serde(default = "default_true")]
    pub weeding: bool,
    #[serde(default = "default_true")]
    pub transits: bool,
    #[serde(default = "default_true")]
    pub deposits: bool,
    #[serde(default = "default_true")]
    pub events: bool,
    #[serde(default = "default_true")]
    pub inventory: bool,
    #[serde(default = "default_true")]
    pub z3950: bool,
    #[serde(default = "default_true")]
    pub import_iso: bool,
    #[serde(default = "default_true")]
    pub stats: bool,
    #[serde(default = "default_true")]
    pub visitor_counts: bool,
}

impl Default for FeaturesConfig {
    fn default() -> Self {
        Self {
            overridable: true,
            profile: None,
            acquisitions: true,
            purchase_suggestions: true,
            weeding: true,
            transits: true,
            deposits: true,
            events: true,
            inventory: true,
            z3950: true,
            import_iso: true,
            stats: true,
            visitor_counts: true,
        }
    }
}

impl FeaturesConfig {
    /// Public payload for `GET /library-info` (bool flags only).
    #[must_use]
    pub fn enabled_flags(&self) -> EnabledFeatures {
        EnabledFeatures {
            acquisitions: self.acquisitions,
            purchase_suggestions: self.purchase_suggestions,
            weeding: self.weeding,
            transits: self.transits,
            deposits: self.deposits,
            events: self.events,
            inventory: self.inventory,
            z3950: self.z3950,
            import_iso: self.import_iso,
            stats: self.stats,
            visitor_counts: self.visitor_counts,
        }
    }

    /// Apply deploy preset after loading the file config.
    ///
    /// `profile = "essential"` disables advanced modules suited to larger libraries.
    /// Explicit bools in the same file are overwritten by the preset for those keys.
    pub fn apply_profile(&mut self) {
        if self.profile.as_deref().map(str::trim) != Some("essential") {
            return;
        }
        self.acquisitions = false;
        self.purchase_suggestions = false;
        self.weeding = false;
        self.transits = false;
        self.deposits = false;
        self.visitor_counts = false;
    }

    #[must_use]
    pub fn is_enabled(&self, feature: Feature) -> bool {
        match feature {
            Feature::Acquisitions => self.acquisitions,
            Feature::PurchaseSuggestions => self.purchase_suggestions,
            Feature::Weeding => self.weeding,
            Feature::Transits => self.transits,
            Feature::Deposits => self.deposits,
            Feature::Events => self.events,
            Feature::Inventory => self.inventory,
            Feature::Z3950 => self.z3950,
            Feature::ImportIso => self.import_iso,
            Feature::Stats => self.stats,
            Feature::VisitorCounts => self.visitor_counts,
        }
    }
}

/// Bool flags returned on `library-info` (same for every user of the instance).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EnabledFeatures {
    pub acquisitions: bool,
    pub purchase_suggestions: bool,
    pub weeding: bool,
    pub transits: bool,
    pub deposits: bool,
    pub events: bool,
    pub inventory: bool,
    pub z3950: bool,
    pub import_iso: bool,
    pub stats: bool,
    pub visitor_counts: bool,
}

impl Default for EnabledFeatures {
    fn default() -> Self {
        FeaturesConfig::default().enabled_flags()
    }
}

/// Identifiers for gated modules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Feature {
    Acquisitions,
    PurchaseSuggestions,
    Weeding,
    Transits,
    Deposits,
    Events,
    Inventory,
    Z3950,
    ImportIso,
    Stats,
    VisitorCounts,
}

impl Feature {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Acquisitions => "acquisitions",
            Self::PurchaseSuggestions => "purchase_suggestions",
            Self::Weeding => "weeding",
            Self::Transits => "transits",
            Self::Deposits => "deposits",
            Self::Events => "events",
            Self::Inventory => "inventory",
            Self::Z3950 => "z3950",
            Self::ImportIso => "import_iso",
            Self::Stats => "stats",
            Self::VisitorCounts => "visitor_counts",
        }
    }
}

/// Refuse the request when the instance feature is off (HTTP 403, `feature_disabled`).
pub fn require_feature(state: &AppState, feature: Feature) -> AppResult<()> {
    if state.dynamic_config.read_features().is_enabled(feature) {
        Ok(())
    } else {
        Err(AppError::FeatureDisabled(feature.as_str().to_string()))
    }
}

macro_rules! define_feature_gate {
    ($fn_name:ident, $feature:expr) => {
        pub async fn $fn_name(
            State(state): State<AppState>,
            request: Request<axum::body::Body>,
            next: Next,
        ) -> Response {
            match require_feature(&state, $feature) {
                Ok(()) => next.run(request).await,
                Err(err) => err.into_response(),
            }
        }
    };
}

define_feature_gate!(gate_acquisitions, Feature::Acquisitions);
define_feature_gate!(gate_purchase_suggestions, Feature::PurchaseSuggestions);
define_feature_gate!(gate_weeding, Feature::Weeding);
define_feature_gate!(gate_transits, Feature::Transits);
define_feature_gate!(gate_deposits, Feature::Deposits);
define_feature_gate!(gate_events, Feature::Events);
define_feature_gate!(gate_inventory, Feature::Inventory);
define_feature_gate!(gate_z3950, Feature::Z3950);
define_feature_gate!(gate_stats, Feature::Stats);
define_feature_gate!(gate_visitor_counts, Feature::VisitorCounts);

/// Attach a feature gate middleware (use from `app.rs` with cloned `AppState`).
pub fn gated(
    router: axum::Router<AppState>,
    state: AppState,
    feature: Feature,
) -> axum::Router<AppState> {
    match feature {
        Feature::Acquisitions => router.route_layer(axum::middleware::from_fn_with_state(
            state,
            gate_acquisitions,
        )),
        Feature::PurchaseSuggestions => router.route_layer(axum::middleware::from_fn_with_state(
            state,
            gate_purchase_suggestions,
        )),
        Feature::Weeding => {
            router.route_layer(axum::middleware::from_fn_with_state(state, gate_weeding))
        }
        Feature::Transits => {
            router.route_layer(axum::middleware::from_fn_with_state(state, gate_transits))
        }
        Feature::Deposits => {
            router.route_layer(axum::middleware::from_fn_with_state(state, gate_deposits))
        }
        Feature::Events => {
            router.route_layer(axum::middleware::from_fn_with_state(state, gate_events))
        }
        Feature::Inventory => {
            router.route_layer(axum::middleware::from_fn_with_state(state, gate_inventory))
        }
        Feature::Z3950 => {
            router.route_layer(axum::middleware::from_fn_with_state(state, gate_z3950))
        }
        Feature::ImportIso => router, // gated per-handler on biblios routes
        Feature::Stats => {
            router.route_layer(axum::middleware::from_fn_with_state(state, gate_stats))
        }
        Feature::VisitorCounts => router.route_layer(axum::middleware::from_fn_with_state(
            state,
            gate_visitor_counts,
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn essential_profile_disables_advanced_modules() {
        let mut cfg = FeaturesConfig {
            profile: Some("essential".into()),
            ..FeaturesConfig::default()
        };
        cfg.apply_profile();
        assert!(!cfg.acquisitions);
        assert!(!cfg.purchase_suggestions);
        assert!(!cfg.weeding);
        assert!(!cfg.transits);
        assert!(!cfg.deposits);
        assert!(!cfg.visitor_counts);
        assert!(cfg.events);
        assert!(cfg.inventory);
        assert!(cfg.stats);
    }

    #[test]
    fn enabled_flags_omit_admin_fields() {
        let flags = FeaturesConfig::default().enabled_flags();
        assert!(flags.acquisitions);
        let json = serde_json::to_value(&flags).unwrap();
        assert!(json.get("overridable").is_none());
        assert!(json.get("profile").is_none());
    }
}
