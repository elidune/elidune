//! Apply periodical migration plan to the database.

use chrono::{NaiveDate, Utc};
use snowflaked::Generator;
use sqlx::{PgPool, Postgres, Transaction};
use std::sync::{LazyLock, Mutex};

use std::collections::HashMap;

use super::corrections::IssueOverride;
use super::plan::{
    MigrationPlan, PlannedCluster, PlannedMember, FREQUENCY_IRREGULAR, PATTERN_ISSUE_NUMBER,
};

static SNOWFLAKE: LazyLock<Mutex<Generator>> = LazyLock::new(|| Mutex::new(Generator::new(14)));

fn next_id() -> i64 {
    SNOWFLAKE
        .lock()
        .unwrap_or_else(|err| err.into_inner())
        .generate::<i64>()
}

#[derive(Debug, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplySummary {
    pub clusters_applied: u64,
    pub biblios_merged: u64,
    pub issues_created: u64,
    pub items_moved: u64,
    pub skipped: u64,
}

pub async fn apply_plan(
    pool: &PgPool,
    plan: &MigrationPlan,
    include_review: bool,
    issue_overrides: &HashMap<i64, IssueOverride>,
) -> Result<ApplySummary, String> {
    let mut summary = ApplySummary::default();
    for cluster in super::plan::clusters_for_apply(plan, include_review) {
        if cluster.site_id.is_none() && cluster.members.iter().all(|m| m.items.is_empty()) {
            summary.skipped += 1;
            continue;
        }
        let site_id = cluster.site_id.ok_or_else(|| {
            format!(
                "Cluster '{}' has copies on multiple sites or no site; resolve manually",
                cluster.cluster_key
            )
        })?;
        apply_cluster(pool, cluster, site_id, issue_overrides, &mut summary).await?;
    }
    Ok(summary)
}

/// Values taken from the corrections file when this bibliographic record is listed there.
struct ResolvedIssue {
    planned_date: NaiveDate,
    issue_number: Option<i32>,
    label: String,
    supplement: bool,
}

fn resolve_issue(member: &PlannedMember, override_: Option<&IssueOverride>) -> ResolvedIssue {
    let fallback_date = member.parsed.planned_date.unwrap_or_else(|| {
        member
            .items
            .iter()
            .filter_map(|item| item.created_at.map(|created| created.date_naive()))
            .min()
            .unwrap_or_else(|| Utc::now().date_naive())
    });
    let Some(override_) = override_ else {
        return ResolvedIssue {
            planned_date: fallback_date,
            issue_number: member.parsed.issue_number,
            label: member.parsed.display_label.clone(),
            supplement: member.parsed.supplement,
        };
    };
    // An override is the whole decision for this record, including an explicit empty issue number.
    ResolvedIssue {
        planned_date: override_.planned_date.unwrap_or(fallback_date),
        issue_number: override_.issue_number,
        label: override_.display_label.clone(),
        supplement: override_.supplement.unwrap_or(member.parsed.supplement),
    }
}

async fn apply_cluster(
    pool: &PgPool,
    cluster: &PlannedCluster,
    site_id: i64,
    issue_overrides: &HashMap<i64, IssueOverride>,
    summary: &mut ApplySummary,
) -> Result<(), String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;

    let subscription_id =
        get_or_create_subscription(&mut tx, cluster, site_id).await?;

    let mut sequence: i32 = existing_max_sequence(&mut tx, subscription_id).await?;
    let mut slot: i32 = existing_max_slot(&mut tx, subscription_id).await?;

    let mut members: Vec<&PlannedMember> = cluster.members.iter().collect();
    members.sort_by(|a, b| {
        let left = resolve_issue(a, issue_overrides.get(&a.biblio_id));
        let right = resolve_issue(b, issue_overrides.get(&b.biblio_id));
        (left.planned_date, left.issue_number, a.biblio_id).cmp(&(
            right.planned_date,
            right.issue_number,
            b.biblio_id,
        ))
    });

    for member in members {
        if already_migrated(&mut tx, member.biblio_id).await? {
            summary.skipped += 1;
            continue;
        }

        let resolved = resolve_issue(member, issue_overrides.get(&member.biblio_id));

        if member.items.is_empty() {
            let issue_id = insert_arrived_issue(
                &mut tx,
                subscription_id,
                resolved.planned_date,
                &resolved.label,
                resolved.issue_number,
                resolved.supplement,
                &mut sequence,
                &mut slot,
                None,
            )
            .await?;
            record_source(
                &mut tx,
                member.biblio_id,
                cluster.canonical_biblio_id,
                subscription_id,
                Some(issue_id),
            )
            .await?;
            summary.issues_created += 1;
            maybe_archive_biblio(&mut tx, member.biblio_id, cluster.canonical_biblio_id).await?;
            if member.biblio_id != cluster.canonical_biblio_id {
                summary.biblios_merged += 1;
            }
            continue;
        }

        let mut items = member.items.clone();
        items.sort_by_key(|i| i.id);

        for (index, item) in items.iter().enumerate() {
            let supplement = resolved.supplement || index > 0;
            let label = if index == 0 {
                resolved.label.clone()
            } else {
                format!("{} (extra copy)", resolved.label)
            };

            sqlx::query("UPDATE items SET biblio_id = $1, updated_at = now() WHERE id = $2")
                .bind(cluster.canonical_biblio_id)
                .bind(item.id)
                .execute(&mut *tx)
                .await
                .map_err(|e| e.to_string())?;
            summary.items_moved += 1;

            reassign_holds(&mut tx, member.biblio_id, cluster.canonical_biblio_id).await?;
            copy_authors(&mut tx, member.biblio_id, cluster.canonical_biblio_id).await?;

            let issue_id = insert_arrived_issue(
                &mut tx,
                subscription_id,
                resolved.planned_date,
                &label,
                resolved.issue_number,
                supplement,
                &mut sequence,
                &mut slot,
                Some(item.id),
            )
            .await?;
            summary.issues_created += 1;

            if index == 0 {
                record_source(
                    &mut tx,
                    member.biblio_id,
                    cluster.canonical_biblio_id,
                    subscription_id,
                    Some(issue_id),
                )
                .await?;
            }
        }

        maybe_archive_biblio(&mut tx, member.biblio_id, cluster.canonical_biblio_id).await?;
        if member.biblio_id != cluster.canonical_biblio_id {
            summary.biblios_merged += 1;
        }
    }

    update_canonical_title(&mut tx, cluster.canonical_biblio_id, &cluster.canonical_title).await?;
    refresh_subscription_counters(&mut tx, subscription_id).await?;

    tx.commit().await.map_err(|e| e.to_string())?;
    summary.clusters_applied += 1;
    Ok(())
}

async fn get_or_create_subscription(
    tx: &mut Transaction<'_, Postgres>,
    cluster: &PlannedCluster,
    site_id: i64,
) -> Result<i64, String> {
    let existing: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM serial_subscriptions \
         WHERE biblio_id = $1 AND site_id = $2 AND status = 'active' \
         LIMIT 1",
    )
    .bind(cluster.canonical_biblio_id)
    .bind(site_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|e| e.to_string())?;
    if let Some(id) = existing {
        return Ok(id);
    }

    let first_date = cluster
        .first_issue_date
        .unwrap_or_else(|| Utc::now().date_naive());
    let id = next_id();
    sqlx::query(
        "INSERT INTO serial_subscriptions ( \
            id, biblio_id, site_id, frequency_id, numbering_pattern_id, \
            first_issue_date, value_x, value_y, value_z, inner_x, inner_y, inner_z, next_slot, \
            length_kind, length_count, irregularities, recurring_skips, combined_issues, \
            status, create_items, latest_issue_library_use_only, created_at, updated_at \
         ) VALUES ( \
            $1, $2, $3, $4, $5, \
            $6, 1, 0, 0, 0, 0, 0, 1, \
            'issues', 12, '{}', '[]', '[]', \
            'active', false, true, now(), now())",
    )
    .bind(id)
    .bind(cluster.canonical_biblio_id)
    .bind(site_id)
    .bind(FREQUENCY_IRREGULAR)
    .bind(PATTERN_ISSUE_NUMBER)
    .bind(first_date)
    .execute(&mut **tx)
    .await
    .map_err(|e| e.to_string())?;
    Ok(id)
}

async fn insert_arrived_issue(
    tx: &mut Transaction<'_, Postgres>,
    subscription_id: i64,
    planned_date: NaiveDate,
    display_label: &str,
    issue_number: Option<i32>,
    supplement: bool,
    sequence: &mut i32,
    slot: &mut i32,
    item_id: Option<i64>,
) -> Result<i64, String> {
    let (seq_val, slot_val) = if supplement {
        (None, None)
    } else {
        *sequence += 1;
        *slot += 1;
        (Some(*sequence), Some(*slot))
    };
    let value_x = issue_number.unwrap_or(seq_val.unwrap_or(0));
    let id = next_id();
    sqlx::query(
        "INSERT INTO serial_issues ( \
            id, serial_subscription_id, planned_date, display_label, sequence, slot, \
            value_x, value_y, value_z, status, supplement, item_id, received_at, \
            created_at, updated_at \
         ) VALUES ( \
            $1, $2, $3, $4, $5, $6, \
            $7, 0, 0, 'arrived', $8, $9, now(), now(), now())",
    )
    .bind(id)
    .bind(subscription_id)
    .bind(planned_date)
    .bind(display_label)
    .bind(seq_val)
    .bind(slot_val)
    .bind(value_x)
    .bind(supplement)
    .bind(item_id)
    .execute(&mut **tx)
    .await
    .map_err(|e| e.to_string())?;
    Ok(id)
}

async fn record_source(
    tx: &mut Transaction<'_, Postgres>,
    source_biblio_id: i64,
    canonical_biblio_id: i64,
    subscription_id: i64,
    issue_id: Option<i64>,
) -> Result<(), String> {
    sqlx::query(
        "INSERT INTO periodic_migration_sources \
            (source_biblio_id, canonical_biblio_id, serial_subscription_id, serial_issue_id) \
         VALUES ($1, $2, $3, $4) \
         ON CONFLICT (source_biblio_id) DO NOTHING",
    )
    .bind(source_biblio_id)
    .bind(canonical_biblio_id)
    .bind(subscription_id)
    .bind(issue_id)
    .execute(&mut **tx)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

async fn already_migrated(tx: &mut Transaction<'_, Postgres>, biblio_id: i64) -> Result<bool, String> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM periodic_migration_sources WHERE source_biblio_id = $1)",
    )
    .bind(biblio_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(|e| e.to_string())?;
    Ok(exists)
}

async fn existing_max_sequence(tx: &mut Transaction<'_, Postgres>, subscription_id: i64) -> Result<i32, String> {
    let max: Option<i32> = sqlx::query_scalar(
        "SELECT MAX(sequence) FROM serial_issues WHERE serial_subscription_id = $1",
    )
    .bind(subscription_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(|e| e.to_string())?;
    Ok(max.unwrap_or(0))
}

async fn existing_max_slot(tx: &mut Transaction<'_, Postgres>, subscription_id: i64) -> Result<i32, String> {
    let max: Option<i32> = sqlx::query_scalar(
        "SELECT MAX(slot) FROM serial_issues WHERE serial_subscription_id = $1",
    )
    .bind(subscription_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(|e| e.to_string())?;
    Ok(max.unwrap_or(0))
}

async fn reassign_holds(
    tx: &mut Transaction<'_, Postgres>,
    from_biblio: i64,
    to_biblio: i64,
) -> Result<(), String> {
    sqlx::query("UPDATE holds SET biblio_id = $1 WHERE biblio_id = $2")
        .bind(to_biblio)
        .bind(from_biblio)
        .execute(&mut **tx)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

async fn copy_authors(
    tx: &mut Transaction<'_, Postgres>,
    from_biblio: i64,
    to_biblio: i64,
) -> Result<(), String> {
    sqlx::query(
        "INSERT INTO biblio_authors (biblio_id, author_id, function) \
         SELECT $1, author_id, function FROM biblio_authors WHERE biblio_id = $2 \
         ON CONFLICT (biblio_id, author_id, function) DO NOTHING",
    )
    .bind(to_biblio)
    .bind(from_biblio)
    .execute(&mut **tx)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

async fn maybe_archive_biblio(
    tx: &mut Transaction<'_, Postgres>,
    biblio_id: i64,
    canonical_biblio_id: i64,
) -> Result<(), String> {
    if biblio_id == canonical_biblio_id {
        return Ok(());
    }
    sqlx::query(
        "UPDATE biblios SET archived_at = now(), updated_at = now() \
         WHERE id = $1 AND archived_at IS NULL \
           AND NOT EXISTS (SELECT 1 FROM items WHERE biblio_id = $1 AND archived_at IS NULL)",
    )
    .bind(biblio_id)
    .execute(&mut **tx)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

async fn update_canonical_title(
    tx: &mut Transaction<'_, Postgres>,
    biblio_id: i64,
    title: &str,
) -> Result<(), String> {
    sqlx::query("UPDATE biblios SET title = $1, updated_at = now() WHERE id = $2")
        .bind(title)
        .bind(biblio_id)
        .execute(&mut **tx)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

async fn refresh_subscription_counters(
    tx: &mut Transaction<'_, Postgres>,
    subscription_id: i64,
) -> Result<(), String> {
    let max_x: Option<i32> = sqlx::query_scalar(
        "SELECT MAX(value_x) FROM serial_issues \
         WHERE serial_subscription_id = $1 AND supplement = false",
    )
    .bind(subscription_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(|e| e.to_string())?;
    let next_slot: Option<i32> = sqlx::query_scalar(
        "SELECT MAX(slot) + 1 FROM serial_issues WHERE serial_subscription_id = $1",
    )
    .bind(subscription_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(|e| e.to_string())?;
    sqlx::query(
        "UPDATE serial_subscriptions SET \
            value_x = $2, next_slot = COALESCE($3, 1), updated_at = now() \
         WHERE id = $1",
    )
    .bind(subscription_id)
    .bind(max_x.unwrap_or(0) + 1)
    .bind(next_slot)
    .execute(&mut **tx)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyReport {
    pub legacy_periodic_biblios: i64,
    pub unmigrated_sources: i64,
    pub subscriptions: i64,
    pub arrived_issues_without_item: i64,
    pub items_on_periodic_without_migration: i64,
}

pub async fn verify(pool: &PgPool) -> Result<VerifyReport, String> {
    let legacy: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)::bigint FROM biblios \
         WHERE media_type = 'periodic' AND archived_at IS NULL \
           AND id NOT IN (SELECT canonical_biblio_id FROM periodic_migration_sources)",
    )
    .fetch_one(pool)
    .await
    .map_err(|e| e.to_string())?;

    let unmigrated: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)::bigint FROM biblios b \
         WHERE b.media_type = 'periodic' AND b.archived_at IS NULL \
           AND NOT EXISTS (SELECT 1 FROM periodic_migration_sources p WHERE p.source_biblio_id = b.id)",
    )
    .fetch_one(pool)
    .await
    .map_err(|e| e.to_string())?;

    let subscriptions: i64 =
        sqlx::query_scalar("SELECT COUNT(*)::bigint FROM serial_subscriptions")
            .fetch_one(pool)
            .await
            .map_err(|e| e.to_string())?;

    let without_item: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)::bigint FROM serial_issues \
         WHERE status = 'arrived' AND item_id IS NULL AND supplement = false",
    )
    .fetch_one(pool)
    .await
    .map_err(|e| e.to_string())?;

    let orphan_items: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)::bigint FROM items i \
         JOIN biblios b ON b.id = i.biblio_id \
         WHERE b.media_type = 'periodic' AND i.archived_at IS NULL AND b.archived_at IS NULL \
           AND NOT EXISTS ( \
               SELECT 1 FROM serial_issues si \
               WHERE si.item_id = i.id \
           )",
    )
    .fetch_one(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(VerifyReport {
        legacy_periodic_biblios: legacy,
        unmigrated_sources: unmigrated,
        subscriptions,
        arrived_issues_without_item: without_item,
        items_on_periodic_without_migration: orphan_items,
    })
}
