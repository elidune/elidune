//! Staff command `migrate-periodics`: merge legacy one-notice-per-issue periodicals into serial subscriptions.

mod apply;
mod corrections;
mod fuzzy;
mod parser;
mod plan;

use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::Path;

use serde::Serialize;
use sqlx::postgres::PgPoolOptions;
use sqlx::{FromRow, PgPool};

use crate::config::{AppConfig, DatabaseConfig};

pub use corrections::MigrationCorrectionsFile;
pub use parser::{ParseConfidence, ParsedPeriodicTitle};
pub use plan::{MigrationPlan, PlanSummary};

use apply::{apply_plan, verify, ApplySummary};
use corrections::{generate_corrections, load_corrections_file, IssueOverride};
use plan::{build_plan, ItemRow, PeriodicBiblioRow};

const USAGE: &str = "\
Migrate legacy periodical biblios into serial subscriptions

Usage:
  elidune-server migrate-periodics <generate-corrections|apply|verify|analyze> [options]

The subcommand must be the first argument (same as reclassify-deposits).

Workflow:
  1. generate-corrections  Build an editable JSON catalog (suggestions only).
  2. Edit clusterMerges / aliases / skipClusters in that file.
  3. apply --corrections <file> --include-review --apply

Options:
  --config, -c <path>       Server config file
  --corrections <path>      Corrections JSON (required for apply)
  --include-review          Apply clusters flagged for review
  --apply                   Persist changes (apply only; default is dry-run)
  --format <json|text>      Report format (default: text)
  --out <path>              Write JSON output to this path
  --help                    Show this help

Requires migration 007 (periodic_migration_sources). Does not run migrations.
";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum ReportFormat {
    #[default]
    Text,
    Json,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Subcommand {
    GenerateCorrections,
    Analyze,
    Apply { dry_run: bool },
    Verify,
}

#[derive(Debug)]
pub struct MigrateFailure {
    message: String,
    exit_code: i32,
}

impl MigrateFailure {
    fn invalid(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            exit_code: 2,
        }
    }

    fn failed(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            exit_code: 1,
        }
    }

    pub fn exit_code(&self) -> i32 {
        self.exit_code
    }
}

impl std::fmt::Display for MigrateFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for MigrateFailure {}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AnalyzeReport {
    dry_run: bool,
    plan: MigrationPlan,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ApplyReport {
    dry_run: bool,
    plan_summary: PlanSummary,
    apply_summary: Option<ApplySummary>,
}

pub async fn run(args: &[String]) -> Result<(), MigrateFailure> {
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("{USAGE}");
        return Ok(());
    }
    let parsed = parse_args(args)?;
    if let Some(path) = &parsed.config_path {
        if !Path::new(path).is_file() {
            return Err(MigrateFailure::invalid(format!(
                "config file not found: {path}"
            )));
        }
    }

    dotenvy::dotenv().ok();
    let config = AppConfig::load(parsed.config_path.as_deref())
        .map_err(|err| MigrateFailure::invalid(format!("failed to load configuration: {err}")))?;
    let pool = connect_database(&config.database)
        .await
        .map_err(|err| MigrateFailure::failed(err.to_string()))?;

    match parsed.subcommand {
        Subcommand::GenerateCorrections => {
            let (biblios, items_by_biblio) = fetch_sources(&pool).await?;
            let file = generate_corrections(biblios, items_by_biblio);
            emit_report(&file, parsed.format, parsed.out.as_deref())?;
        }
        Subcommand::Analyze => {
            let corrections = parsed
                .corrections
                .clone()
                .unwrap_or_default();
            let plan = load_plan(&pool, &corrections).await?;
            let report = AnalyzeReport {
                dry_run: true,
                plan,
            };
            emit_report(&report, parsed.format, parsed.out.as_deref())?;
        }
        Subcommand::Apply { dry_run } => {
            let corrections = parsed.corrections.as_ref().ok_or_else(|| {
                MigrateFailure::invalid("apply requires --corrections <path>")
            })?;
            let plan = load_plan(&pool, corrections).await?;
            let apply_summary = if dry_run {
                None
            } else {
                Some(
                    apply_plan(
                        &pool,
                        &plan,
                        parsed.include_review,
                        &issue_override_index(&corrections.issue_overrides),
                    )
                        .await
                        .map_err(MigrateFailure::failed)?,
                )
            };
            let report = ApplyReport {
                dry_run,
                plan_summary: plan.summary.clone(),
                apply_summary,
            };
            emit_report(&report, parsed.format, parsed.out.as_deref())?;
        }
        Subcommand::Verify => {
            let report = verify(&pool)
                .await
                .map_err(MigrateFailure::failed)?;
            emit_report(&report, parsed.format, parsed.out.as_deref())?;
        }
    }
    Ok(())
}

struct ParsedArgs {
    config_path: Option<String>,
    format: ReportFormat,
    out: Option<String>,
    corrections: Option<MigrationCorrectionsFile>,
    include_review: bool,
    subcommand: Subcommand,
}

fn parse_args(args: &[String]) -> Result<ParsedArgs, MigrateFailure> {
    let mut config_path = None;
    let mut format = ReportFormat::Text;
    let mut out = None;
    let mut corrections_path = None;
    let mut include_review = false;
    let mut apply_flag = false;

    let subcommand = args.first().ok_or_else(|| {
        MigrateFailure::invalid(
            "missing subcommand: generate-corrections, apply, verify, or analyze",
        )
    })?;
    let sub = match subcommand.as_str() {
        "generate-corrections" => Subcommand::GenerateCorrections,
        "analyze" => Subcommand::Analyze,
        "apply" => Subcommand::Apply { dry_run: true },
        "verify" => Subcommand::Verify,
        other => {
            return Err(MigrateFailure::invalid(format!(
                "unknown subcommand '{other}'"
            )));
        }
    };

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--config" | "-c" => {
                i += 1;
                config_path = Some(next_arg(args, i, "--config")?);
            }
            "--format" => {
                i += 1;
                format = match next_arg(args, i, "--format")?.as_str() {
                    "json" => ReportFormat::Json,
                    "text" => ReportFormat::Text,
                    other => {
                        return Err(MigrateFailure::invalid(format!(
                            "unsupported format '{other}'"
                        )));
                    }
                };
            }
            "--out" => {
                i += 1;
                out = Some(next_arg(args, i, "--out")?);
            }
            "--corrections" => {
                i += 1;
                corrections_path = Some(next_arg(args, i, "--corrections")?);
            }
            "--include-review" => include_review = true,
            "--apply" => apply_flag = true,
            other => {
                return Err(MigrateFailure::invalid(format!("unknown option '{other}'")));
            }
        }
        i += 1;
    }

    let corrections = if let Some(path) = corrections_path {
        Some(
            load_corrections_file(Path::new(&path))
                .map_err(MigrateFailure::invalid)?,
        )
    } else {
        None
    };

    let subcommand = match sub {
        Subcommand::Apply { .. } => Subcommand::Apply {
            dry_run: !apply_flag,
        },
        other => other,
    };

    Ok(ParsedArgs {
        config_path,
        format,
        out,
        corrections,
        include_review,
        subcommand,
    })
}

fn next_arg(args: &[String], index: usize, flag: &str) -> Result<String, MigrateFailure> {
    args.get(index)
        .cloned()
        .ok_or_else(|| MigrateFailure::invalid(format!("{flag} requires a value")))
}

async fn connect_database(db: &DatabaseConfig) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(db.max_connections)
        .min_connections(db.min_connections)
        .connect(&db.url)
        .await
}

async fn fetch_sources(
    pool: &PgPool,
) -> Result<(Vec<PeriodicBiblioRow>, std::collections::HashMap<i64, Vec<ItemRow>>), MigrateFailure>
{
    let biblios: Vec<PeriodicBiblioRow> = sqlx::query_as(
        "SELECT id, title, publication_date FROM biblios \
         WHERE media_type = 'periodic' AND archived_at IS NULL \
         ORDER BY id",
    )
    .fetch_all(pool)
    .await
    .map_err(|err| MigrateFailure::failed(err.to_string()))?;

    #[derive(FromRow)]
    struct ItemFetch {
        biblio_id: i64,
        id: i64,
        site_id: i64,
        created_at: Option<chrono::DateTime<chrono::Utc>>,
    }

    let items: Vec<ItemFetch> = sqlx::query_as(
        "SELECT i.biblio_id, i.id, i.site_id, i.created_at FROM items i \
         JOIN biblios b ON b.id = i.biblio_id \
         WHERE b.media_type = 'periodic' AND i.archived_at IS NULL AND b.archived_at IS NULL",
    )
    .fetch_all(pool)
    .await
    .map_err(|err| MigrateFailure::failed(err.to_string()))?;

    let mut items_by_biblio: std::collections::HashMap<i64, Vec<ItemRow>> =
        std::collections::HashMap::new();
    for row in items {
        items_by_biblio
            .entry(row.biblio_id)
            .or_default()
            .push(ItemRow {
                id: row.id,
                site_id: row.site_id,
                created_at: row.created_at,
            });
    }
    Ok((biblios, items_by_biblio))
}

async fn load_plan(
    pool: &PgPool,
    corrections: &MigrationCorrectionsFile,
) -> Result<MigrationPlan, MigrateFailure> {
    let (biblios, items_by_biblio) = fetch_sources(pool).await?;
    let overrides = corrections.to_overrides();

    let migrated: i64 = match sqlx::query_scalar(
        "SELECT COUNT(*)::bigint FROM periodic_migration_sources",
    )
    .fetch_one(pool)
    .await
    {
        Ok(count) => count,
        Err(_) => 0,
    };

    let mut plan = build_plan(biblios, items_by_biblio, &overrides);
    plan.skipped_already_migrated = migrated;
    plan.cluster_merges = overrides.cluster_merges.clone();
    Ok(plan)
}

fn issue_override_index(overrides: &[IssueOverride]) -> HashMap<i64, IssueOverride> {
    overrides
        .iter()
        .map(|override_| (override_.biblio_id, override_.clone()))
        .collect()
}

fn emit_report<T: Serialize>(
    report: &T,
    format: ReportFormat,
    out_path: Option<&str>,
) -> Result<(), MigrateFailure> {
    let body = match format {
        ReportFormat::Json => serde_json::to_string_pretty(report)
            .map_err(|err| MigrateFailure::failed(err.to_string()))?,
        ReportFormat::Text => serde_json::to_string_pretty(report)
            .map_err(|err| MigrateFailure::failed(err.to_string()))?,
    };
    if let Some(path) = out_path {
        fs::write(path, &body).map_err(|err| MigrateFailure::failed(err.to_string()))?;
    }
    println!("{body}");
    std::io::stdout()
        .flush()
        .map_err(|err| MigrateFailure::failed(err.to_string()))?;
    Ok(())
}
