//! Build migration clusters from legacy periodical biblios.

use std::collections::{HashMap, HashSet};

use chrono::NaiveDate;
use serde::Serialize;
use sqlx::FromRow;

use super::fuzzy::{resolve_cluster_key, suggest_cluster_merges};
use super::parser::{parse_periodic_title, ParseConfidence, ParsedPeriodicTitle};

pub const FREQUENCY_IRREGULAR: i64 = 14;
pub const PATTERN_ISSUE_NUMBER: i64 = 1;

#[derive(Debug, Clone, FromRow, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemRow {
    pub id: i64,
    pub site_id: i64,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, FromRow)]
pub struct PeriodicBiblioRow {
    pub id: i64,
    pub title: String,
    pub publication_date: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlannedMember {
    pub biblio_id: i64,
    pub title: String,
    pub parsed: ParsedPeriodicTitle,
    pub items: Vec<ItemRow>,
    pub confidence: ParseConfidence,
    pub issue_collision: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlannedCluster {
    pub cluster_key: String,
    pub canonical_biblio_id: i64,
    pub canonical_title: String,
    pub confidence: ParseConfidence,
    pub members: Vec<PlannedMember>,
    pub site_id: Option<i64>,
    pub first_issue_date: Option<NaiveDate>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrationPlan {
    pub clusters: Vec<PlannedCluster>,
    pub skipped_already_migrated: i64,
    /// Typo merges applied when building this plan (for editing / `--analyze-merges`).
    #[serde(default)]
    pub cluster_merges: HashMap<String, String>,
    pub summary: PlanSummary,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanSummary {
    pub cluster_count: u64,
    pub biblio_count: u64,
    pub item_count: u64,
    pub high: u64,
    pub review: u64,
    pub blocked: u64,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrationOverrides {
    #[serde(default)]
    pub skip_clusters: HashSet<String>,
    #[serde(default)]
    pub aliases: HashMap<String, String>,
    /// Map typo cluster keys to the subscription cluster they should join.
    #[serde(default)]
    pub cluster_merges: HashMap<String, String>,
    #[serde(default)]
    pub auto_merge_typo_clusters: bool,
    #[serde(default)]
    pub auto_merge_min_similarity: f64,
    #[serde(default)]
    pub force_canonical_biblio_id: HashMap<String, i64>,
}

pub fn build_plan(
    biblios: Vec<PeriodicBiblioRow>,
    items_by_biblio: HashMap<i64, Vec<ItemRow>>,
    overrides: &MigrationOverrides,
) -> MigrationPlan {
    let mut grouped: HashMap<String, Vec<(PeriodicBiblioRow, ParsedPeriodicTitle, Vec<ItemRow>)>> =
        HashMap::new();

    for biblio in biblios {
        let parsed = parse_periodic_title(
            &biblio.title,
            biblio.publication_date.as_deref(),
            &overrides.aliases,
        );
        let items = items_by_biblio.get(&biblio.id).cloned().unwrap_or_default();
        let key = resolve_cluster_key(&parsed.cluster_key, &overrides.cluster_merges);
        grouped.entry(key).or_default().push((biblio, parsed, items));
    }

    let mut applied_merges = overrides.cluster_merges.clone();
    if overrides.auto_merge_typo_clusters {
        let (merged, fuzzy) = merge_grouped_by_fuzzy_similarity(
            grouped,
            overrides.auto_merge_min_similarity,
        );
        grouped = merged;
        applied_merges.extend(fuzzy);
    }

    let mut clusters = Vec::new();
    let mut summary = PlanSummary::default();

    for (cluster_key, mut members_raw) in grouped {
        if overrides.skip_clusters.contains(&cluster_key) {
            continue;
        }

        members_raw.sort_by(|a, b| a.0.id.cmp(&b.0.id));

        let canonical = pick_canonical(&members_raw, overrides, &cluster_key);
        let site_id = infer_site_id(&members_raw);

        let duplicate_issues = issue_collisions(&members_raw);
        let cluster_confidence = cluster_confidence(&members_raw, false);

        let mut members = Vec::new();
        for (biblio, parsed, items) in &members_raw {
            let member_confidence = member_confidence(parsed, false, !items.is_empty());
            summary.item_count += items.len() as u64;
            members.push(PlannedMember {
                biblio_id: biblio.id,
                title: biblio.title.clone(),
                parsed: parsed.clone(),
                items: items.clone(),
                confidence: member_confidence,
                issue_collision: duplicate_issues,
            });
        }
        summary.biblio_count += members.len() as u64;

        match cluster_confidence {
            ParseConfidence::High => summary.high += 1,
            ParseConfidence::Review => summary.review += 1,
            ParseConfidence::Blocked => summary.blocked += 1,
        }

        let first_issue_date = infer_first_issue_date(&members);

        clusters.push(PlannedCluster {
            cluster_key,
            canonical_biblio_id: canonical.0,
            canonical_title: canonical.1,
            confidence: cluster_confidence,
            members,
            site_id,
            first_issue_date,
        });
    }

    clusters.sort_by(|a, b| a.cluster_key.cmp(&b.cluster_key));
    summary.cluster_count = clusters.len() as u64;

    MigrationPlan {
        clusters,
        skipped_already_migrated: 0,
        cluster_merges: applied_merges,
        summary,
    }
}

fn pick_canonical(
    members: &[(PeriodicBiblioRow, ParsedPeriodicTitle, Vec<ItemRow>)],
    overrides: &MigrationOverrides,
    cluster_key: &str,
) -> (i64, String) {
    if let Some(id) = overrides.force_canonical_biblio_id.get(cluster_key) {
        if let Some((biblio, _, _)) = members.iter().find(|(b, _, _)| b.id == *id) {
            return (biblio.id, biblio.title.clone());
        }
    }
    let (biblio, _, _) = members
        .iter()
        .min_by_key(|(b, _, _)| (title_len(&b.title), b.id))
        .expect("non-empty cluster");
    (biblio.id, biblio.title.clone())
}

fn title_len(title: &str) -> usize {
    title.chars().filter(|c| !c.is_whitespace()).count()
}

fn infer_site_id(members: &[(PeriodicBiblioRow, ParsedPeriodicTitle, Vec<ItemRow>)]) -> Option<i64> {
    let mut counts: HashMap<i64, u32> = HashMap::new();
    for (_, _, items) in members {
        for item in items {
            *counts.entry(item.site_id).or_default() += 1;
        }
    }
    counts
        .into_iter()
        .max_by_key(|(_, count)| *count)
        .map(|(site, _)| site)
}

fn issue_collisions(
    members: &[(PeriodicBiblioRow, ParsedPeriodicTitle, Vec<ItemRow>)],
) -> bool {
    let mut seen = HashMap::new();
    for (_, parsed, _) in members {
        if parsed.supplement {
            continue;
        }
        if let Some(num) = parsed.issue_number {
            if seen.insert(num, ()).is_some() {
                return true;
            }
        }
    }
    false
}

fn cluster_confidence(
    members: &[(PeriodicBiblioRow, ParsedPeriodicTitle, Vec<ItemRow>)],
    _hard_collisions: bool,
) -> ParseConfidence {
    let needs_review = members.iter().any(|(_, parsed, items)| {
        !items.is_empty() && parsed.issue_number.is_none() && parsed.planned_date.is_none()
    });
    if needs_review {
        ParseConfidence::Review
    } else {
        ParseConfidence::High
    }
}

fn member_confidence(
    parsed: &ParsedPeriodicTitle,
    _hard_collisions: bool,
    has_items: bool,
) -> ParseConfidence {
    if !has_items {
        return ParseConfidence::Review;
    }
    if parsed.issue_number.is_some() || parsed.planned_date.is_some() {
        ParseConfidence::High
    } else {
        ParseConfidence::Review
    }
}

fn infer_first_issue_date(members: &[PlannedMember]) -> Option<NaiveDate> {
    let mut dates: Vec<NaiveDate> = members
        .iter()
        .filter_map(|m| m.parsed.planned_date)
        .collect();
    for member in members {
        for item in &member.items {
            if let Some(created) = item.created_at {
                dates.push(created.date_naive());
            }
        }
    }
    dates.into_iter().min()
}

fn merge_grouped_by_fuzzy_similarity(
    grouped: HashMap<String, Vec<(PeriodicBiblioRow, ParsedPeriodicTitle, Vec<ItemRow>)>>,
    min_similarity: f64,
) -> (
    HashMap<String, Vec<(PeriodicBiblioRow, ParsedPeriodicTitle, Vec<ItemRow>)>>,
    HashMap<String, String>,
) {
    let sizes: HashMap<String, usize> = grouped
        .iter()
        .map(|(k, v)| (k.clone(), v.len()))
        .collect();
    let merges = suggest_cluster_merges(&sizes, min_similarity);
    if merges.is_empty() {
        return (grouped, HashMap::new());
    }
    let mut out: HashMap<String, Vec<(PeriodicBiblioRow, ParsedPeriodicTitle, Vec<ItemRow>)>> =
        HashMap::new();
    for (key, members) in grouped {
        let canon = resolve_cluster_key(&key, &merges);
        out.entry(canon).or_default().extend(members);
    }
    (out, merges)
}

pub fn clusters_for_apply<'a>(
    plan: &'a MigrationPlan,
    include_review: bool,
) -> impl Iterator<Item = &'a PlannedCluster> {
    plan.clusters.iter().filter(move |cluster| {
        match cluster.confidence {
            ParseConfidence::High => true,
            ParseConfidence::Review => include_review,
            ParseConfidence::Blocked => false,
        }
    })
}
