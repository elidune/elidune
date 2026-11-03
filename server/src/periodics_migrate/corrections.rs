//! Human-editable migration corrections file (no catalog rules hardcoded in Rust).

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use super::fuzzy::suggest_cluster_merges;
use super::plan::{build_plan, MigrationOverrides, MigrationPlan, PeriodicBiblioRow, ItemRow};

pub const CORRECTIONS_VERSION: u32 = 1;

fn default_version() -> u32 {
    CORRECTIONS_VERSION
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrationCorrectionsFile {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub aliases: HashMap<String, String>,
    #[serde(default)]
    pub cluster_merges: HashMap<String, String>,
    #[serde(default)]
    pub skip_clusters: HashSet<String>,
    #[serde(default)]
    pub force_canonical_biblio_id: HashMap<String, i64>,
    /// Per bibliographic record. Apply copies these fields onto the serial issue.
    #[serde(default)]
    pub issue_overrides: Vec<IssueOverride>,
    /// Catalog snapshot to review before apply. Not re-read for logic beyond suggestions display.
    #[serde(default)]
    pub clusters: Vec<ClusterCatalogEntry>,
}

/// Ready-to-store issue text, date and number for one legacy bibliographic record.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueOverride {
    pub biblio_id: i64,
    pub display_label: String,
    #[serde(default)]
    pub planned_date: Option<chrono::NaiveDate>,
    #[serde(default)]
    pub issue_number: Option<i32>,
    #[serde(default)]
    pub supplement: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClusterCatalogEntry {
    pub cluster_key: String,
    pub member_count: usize,
    pub item_count: usize,
    pub canonical_biblio_id: i64,
    pub canonical_title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested_merge_into: Option<String>,
}

impl MigrationCorrectionsFile {
    pub fn to_overrides(&self) -> MigrationOverrides {
        MigrationOverrides {
            skip_clusters: self.skip_clusters.clone(),
            aliases: self.aliases.clone(),
            cluster_merges: self.cluster_merges.clone(),
            auto_merge_typo_clusters: false,
            auto_merge_min_similarity: 0.0,
            force_canonical_biblio_id: self.force_canonical_biblio_id.clone(),
        }
    }
}

pub fn generate_corrections(
    biblios: Vec<PeriodicBiblioRow>,
    items_by_biblio: HashMap<i64, Vec<ItemRow>>,
) -> MigrationCorrectionsFile {
    let empty = MigrationOverrides::default();
    let raw_plan = build_plan(biblios, items_by_biblio, &empty);

    let sizes: HashMap<String, usize> = raw_plan
        .clusters
        .iter()
        .map(|c| (c.cluster_key.clone(), c.members.len()))
        .collect();
    let suggestions = suggest_cluster_merges(&sizes, 0.90);

    let clusters = raw_plan
        .clusters
        .iter()
        .map(|c| {
            let item_count: usize = c.members.iter().map(|m| m.items.len()).sum();
            ClusterCatalogEntry {
                cluster_key: c.cluster_key.clone(),
                member_count: c.members.len(),
                item_count,
                canonical_biblio_id: c.canonical_biblio_id,
                canonical_title: c.canonical_title.clone(),
                suggested_merge_into: suggestions.get(&c.cluster_key).cloned(),
            }
        })
        .collect();

    MigrationCorrectionsFile {
        version: CORRECTIONS_VERSION,
        aliases: HashMap::new(),
        cluster_merges: HashMap::new(),
        skip_clusters: HashSet::new(),
        force_canonical_biblio_id: HashMap::new(),
        issue_overrides: Vec::new(),
        clusters,
    }
}

pub fn load_corrections_file(path: &std::path::Path) -> Result<MigrationCorrectionsFile, String> {
    let raw = std::fs::read_to_string(path).map_err(|err| err.to_string())?;
    let file: MigrationCorrectionsFile =
        serde_json::from_str(&raw).map_err(|err| format!("invalid corrections JSON: {err}"))?;
    if file.version != CORRECTIONS_VERSION {
        return Err(format!(
            "unsupported corrections version {} (expected {CORRECTIONS_VERSION})",
            file.version
        ));
    }
    Ok(file)
}

/// Keys that would receive issues on apply (after merges), for sanity checks.
pub fn effective_subscription_keys(plan: &MigrationPlan) -> HashSet<String> {
    plan.clusters
        .iter()
        .map(|c| c.cluster_key.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corrections_roundtrip() {
        let file = MigrationCorrectionsFile {
            version: CORRECTIONS_VERSION,
            aliases: HashMap::from([("4 saisonns".into(), "4 saisons".into())]),
            cluster_merges: HashMap::from([("4 saisonns".into(), "4 saisons".into())]),
            skip_clusters: HashSet::new(),
            force_canonical_biblio_id: HashMap::new(),
            issue_overrides: vec![],
            clusters: vec![],
        };
        let json = serde_json::to_string(&file).unwrap();
        let back: MigrationCorrectionsFile = serde_json::from_str(&json).unwrap();
        assert_eq!(back.cluster_merges.get("4 saisonns").unwrap(), "4 saisons");
    }

}
