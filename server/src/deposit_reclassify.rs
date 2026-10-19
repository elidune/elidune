//! Staff command `reclassify-deposits`.
//!
//! Historical MARC imports stored the lender as `items.source_id`. This command
//! moves those copies onto a real site and an incoming deposit batch. It changes
//! only `items.deposit_batch_id`, `items.source_id`, and `items.updated_at`.
//! Loans, holds, transits, due dates, fines, and bibliographic records stay as they are.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;
use serde_with::{serde_as, DisplayFromStr};
use sqlx::postgres::PgPoolOptions;
use sqlx::{FromRow, PgPool, Postgres, Transaction};

use crate::config::{AppConfig, DatabaseConfig};
use crate::repository::Repository;
use crate::services::audit;

const REFERENCE_MAX_CHARS: usize = 200;
const NOTES_MAX_CHARS: usize = 4000;
const ACTOR_TYPE: &str = "script";
const ACTOR_NAME: &str = "reclassify-deposits";

const USAGE: &str = "\
reclassify historical deposit copies into a deposit batch

Usage:
  elidune-server reclassify-deposits [options]

Options:
  --config, -c <path>          Server config file (same keys as the server)
  --lender <id>                Existing lender id; refused when missing or archived
  --batch-reference <text>     Agreement reference. Reused when that lender already has it
  --return-due-on <YYYY-MM-DD> Required only when the batch does not already exist
  --received-on <YYYY-MM-DD>   arrived_on used when creating the batch (default: today, UTC)
  --notes <text>               Notes stored when creating the batch
  --site <source_id>           Physical site the copies are assigned to (items.source_id)
  --from-source <source_id>    Every non-archived copy currently at this source
  --barcodes <file>            One barcode per line, or a CSV with a barcode column
  --apply                      Write changes. Omit for a dry run (the default)
  --format <json|text>         Report format (default: text)
  --help                       Show this help

Pass exactly one of --from-source or --barcodes.
Dry run writes nothing. Run it against a copy of the production database before --apply.
The database must already include deposit batches (migration 065). This command does not run migrations.
";

const ITEM_SELECT: &str = r#"
    SELECT
        i.id,
        i.barcode,
        i.biblio_id,
        b.title,
        i.source_id,
        i.deposit_batch_id,
        i.archived_at,
        i.weeding_status,
        EXISTS (
            SELECT 1 FROM loans l
            WHERE l.item_id = i.id AND l.returned_at IS NULL
        ) AS on_loan,
        EXISTS (
            SELECT 1 FROM holds h
            WHERE h.item_id = i.id
              AND h.status IN ('pending', 'to_pick', 'ready')
        ) AS has_holds,
        EXISTS (
            SELECT 1 FROM item_transits t
            WHERE t.item_id = i.id
              AND t.status IN ('requested', 'in_transit')
        ) AS in_transit
    FROM items i
    LEFT JOIN biblios b ON b.id = i.biblio_id
"#;

/// Why a copy is not updated. `already_reclassified` is a skip, not a refusal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LineReason {
    BarcodeUnknown,
    ItemArchived,
    ItemInOtherBatch,
    ItemInTransit,
    AlreadyReclassified,
}

/// What the command would do, or did, with one selected copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LineDisposition {
    Reclassify,
    Refused,
    AlreadyReclassified,
}

/// Whether the named batch already existed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BatchAction {
    Create,
    Reuse,
}

impl BatchAction {
    fn as_str(self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Reuse => "reuse",
        }
    }
}

/// Report format for stdout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReportFormat {
    #[default]
    Text,
    Json,
}

/// How copies are chosen. Exactly one variant is set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selection {
    /// Non-archived copies (`archived_at` is null) whose `source_id` is this source.
    FromSource(i64),
    /// Barcodes in file order, duplicates removed.
    Barcodes(Vec<String>),
}

/// Explicit inputs. Nothing is inferred from the catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReclassifyCommand {
    pub lender_id: i64,
    pub batch_reference: String,
    pub return_due_on: Option<NaiveDate>,
    pub received_on: Option<NaiveDate>,
    pub notes: Option<String>,
    pub site_id: i64,
    pub selection: Selection,
    pub apply: bool,
}

/// Parsed CLI invocation, including config path and report format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invocation {
    pub config_path: Option<String>,
    pub format: ReportFormat,
    pub command: ReclassifyCommand,
}

/// Invalid arguments (exit 2) or a database failure (exit 1).
#[derive(Debug)]
pub struct InvalidArgument {
    code: Option<&'static str>,
    message: String,
}

/// Invalid arguments (exit 2) or a database failure (exit 1).
#[derive(Debug)]
pub enum ReclassifyFailure {
    Invalid(InvalidArgument),
    Failed(String),
}

impl ReclassifyFailure {
    fn invalid(message: impl Into<String>) -> Self {
        Self::Invalid(InvalidArgument {
            code: None,
            message: message.into(),
        })
    }

    fn invalid_coded(code: &'static str, message: impl Into<String>) -> Self {
        Self::Invalid(InvalidArgument {
            code: Some(code),
            message: message.into(),
        })
    }

    fn failed(message: impl Into<String>) -> Self {
        Self::Failed(message.into())
    }

    /// Stable machine code when the failure has one (`deposit_site_mismatch`).
    #[must_use]
    pub fn code(&self) -> Option<&'static str> {
        match self {
            Self::Invalid(argument) => argument.code,
            Self::Failed(_) => None,
        }
    }

    /// Process exit code. Refusals of individual copies are not failures.
    #[must_use]
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Invalid(_) => 2,
            Self::Failed(_) => 1,
        }
    }
}

impl std::fmt::Display for ReclassifyFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid(argument) => f.write_str(&argument.message),
            Self::Failed(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for ReclassifyFailure {}

fn failed(err: sqlx::Error) -> ReclassifyFailure {
    ReclassifyFailure::failed(err.to_string())
}

#[serde_as]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportLine {
    pub barcode: Option<String>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub item_id: Option<i64>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub biblio_id: Option<i64>,
    pub title: Option<String>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub old_source_id: Option<i64>,
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub new_source_id: Option<i64>,
    pub disposition: LineDisposition,
    pub reason: Option<LineReason>,
    pub on_loan: bool,
    pub has_holds: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DuplicateReason {
    PossibleDuplicateRecord,
}

#[serde_as]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PossibleDuplicateRecord {
    #[serde_as(as = "DisplayFromStr")]
    pub record_id: i64,
    #[serde_as(as = "DisplayFromStr")]
    pub other_record_id: i64,
    pub isbn: String,
    pub reason: DuplicateReason,
}

/// Counts. `on_loan` and `with_holds` count copies that will be reclassified
/// and currently have an open loan or an active hold. Refused copies are excluded.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub to_reclassify: u64,
    pub on_loan: u64,
    pub with_holds: u64,
    pub refused: u64,
    pub already_reclassified: u64,
}

/// A condition that does not refuse the run. `count` is set when the warning
/// is about a number of copies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportWarning {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<i64>,
}

#[serde_as]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub dry_run: bool,
    pub applied: bool,
    #[serde_as(as = "DisplayFromStr")]
    pub lender_id: i64,
    pub batch_reference: String,
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub batch_id: Option<i64>,
    pub batch_action: BatchAction,
    #[serde_as(as = "DisplayFromStr")]
    pub site_id: i64,
    pub arrived_on: NaiveDate,
    pub return_due_on: NaiveDate,
    pub notes: Option<String>,
    pub warnings: Vec<ReportWarning>,
    /// Copies whose `source_id` is still `--from-source` after this run.
    /// Absent when the selection is a barcode list.
    pub remaining_on_old_source: Option<i64>,
    pub items: Vec<ReportLine>,
    pub possible_duplicate_records: Vec<PossibleDuplicateRecord>,
    pub summary: Summary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ItemFacts {
    archived: bool,
    withdrawn: bool,
    deposit_batch_id: Option<i64>,
    source_id: Option<i64>,
    in_transit: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Decision {
    Reclassify,
    Already,
    Refused(LineReason),
}

/// Archived (`archived_at`) and withdrawn weeding are refused.
/// `candidate` still circulates, so it can be reclassified.
/// An open transit is `requested` or `in_transit`.
fn decide(facts: &ItemFacts, target_batch_id: Option<i64>, site_id: i64) -> Decision {
    if facts.archived || facts.withdrawn {
        return Decision::Refused(LineReason::ItemArchived);
    }
    if target_batch_id.is_some()
        && facts.deposit_batch_id == target_batch_id
        && facts.source_id == Some(site_id)
    {
        return Decision::Already;
    }
    if facts.deposit_batch_id.is_some() && facts.deposit_batch_id != target_batch_id {
        return Decision::Refused(LineReason::ItemInOtherBatch);
    }
    if facts.in_transit {
        return Decision::Refused(LineReason::ItemInTransit);
    }
    Decision::Reclassify
}

#[derive(Debug, FromRow)]
struct Candidate {
    id: i64,
    barcode: Option<String>,
    biblio_id: Option<i64>,
    title: Option<String>,
    source_id: Option<i64>,
    deposit_batch_id: Option<i64>,
    archived_at: Option<DateTime<Utc>>,
    weeding_status: String,
    on_loan: bool,
    has_holds: bool,
    in_transit: bool,
}

impl Candidate {
    fn facts(&self) -> ItemFacts {
        ItemFacts {
            archived: self.archived_at.is_some(),
            withdrawn: self.weeding_status == "withdrawn",
            deposit_batch_id: self.deposit_batch_id,
            source_id: self.source_id,
            in_transit: self.in_transit,
        }
    }
}

struct PlannedItem {
    line: ReportLine,
    update: Option<ItemUpdate>,
    biblio_id: Option<i64>,
}

struct ItemUpdate {
    item_id: i64,
    old_source_id: Option<i64>,
    barcode: Option<String>,
    biblio_id: Option<i64>,
}

#[derive(Debug, FromRow)]
struct BatchRow {
    id: i64,
    status: String,
    site_id: i64,
    arrived_on: NaiveDate,
    return_due_on: NaiveDate,
    notes: Option<String>,
}

struct ResolvedBatch {
    id: Option<i64>,
    action: BatchAction,
    arrived_on: NaiveDate,
    return_due_on: NaiveDate,
    notes: Option<String>,
}

struct NewBatch {
    arrived_on: NaiveDate,
    return_due_on: NaiveDate,
    notes: Option<String>,
}

#[derive(Debug, FromRow)]
struct LenderRow {
    archived_at: Option<DateTime<Utc>>,
}

#[derive(Debug, FromRow)]
struct SiteRow {
    archived_at: Option<DateTime<Utc>>,
    is_archive: i32,
}

#[derive(Debug, FromRow)]
struct DuplicateRow {
    record_id: i64,
    other_record_id: i64,
    isbn: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ReclassifyAuditPayload {
    actor_type: &'static str,
    actor_name: &'static str,
    lender_id: i64,
    batch_id: i64,
    old_source_id: Option<i64>,
    new_source_id: i64,
    barcode: Option<String>,
    item_id: i64,
    biblio_id: Option<i64>,
}

type Tx<'a> = Transaction<'a, Postgres>;

/// Run the subcommand. Prints the report on success.
///
/// Loads the same config as the server (`--config` / `-c`, then `ELIDUNE_*`)
/// and connects to that database. Pending migrations are not applied.
pub async fn run(args: &[String]) -> Result<(), ReclassifyFailure> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{USAGE}");
        return Ok(());
    }
    let invocation = parse_invocation(args)?;
    if let Some(path) = &invocation.config_path {
        if !Path::new(path).is_file() {
            return Err(ReclassifyFailure::invalid(format!(
                "config file not found: {path}"
            )));
        }
    }
    dotenvy::dotenv().ok();
    let config = AppConfig::load(invocation.config_path.as_deref()).map_err(|err| {
        ReclassifyFailure::invalid(format!("failed to load configuration: {err}"))
    })?;
    let pool = connect_database(&config.database).await.map_err(failed)?;
    let report = execute(&pool, &invocation.command).await?;
    print!("{}", render_report(&report, invocation.format));
    let _ = std::io::Write::flush(&mut std::io::stdout());
    Ok(())
}

async fn connect_database(db: &DatabaseConfig) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(db.max_connections)
        .min_connections(db.min_connections)
        .connect(&db.url)
        .await
}

/// Parse CLI arguments. Does not open the database or the barcode file's parent checks
/// beyond reading `--barcodes`.
#[must_use = "parse errors must be reported"]
pub fn parse_invocation(args: &[String]) -> Result<Invocation, ReclassifyFailure> {
    let mut config_path = None;
    let mut format = None;
    let mut lender_id = None;
    let mut batch_reference = None;
    let mut return_due_on = None;
    let mut received_on = None;
    let mut notes: Option<Option<String>> = None;
    let mut site_id = None;
    let mut from_source = None;
    let mut barcodes_path = None;
    let mut apply = false;

    let mut index = 0;
    while index < args.len() {
        let flag = args[index].as_str();
        index += 1;
        match flag {
            "--apply" => {
                if apply {
                    return Err(ReclassifyFailure::invalid(
                        "--apply was given more than once",
                    ));
                }
                apply = true;
            }
            "--config" | "-c" => {
                set_once(
                    &mut config_path,
                    flag,
                    take_value(args, &mut index, flag)?.to_string(),
                )?;
            }
            "--format" => {
                set_once(
                    &mut format,
                    flag,
                    parse_format(take_value(args, &mut index, flag)?)?,
                )?;
            }
            "--lender" => {
                set_once(
                    &mut lender_id,
                    flag,
                    parse_id(flag, take_value(args, &mut index, flag)?)?,
                )?;
            }
            "--batch-reference" => {
                set_once(
                    &mut batch_reference,
                    flag,
                    parse_reference(take_value(args, &mut index, flag)?)?,
                )?;
            }
            "--return-due-on" => {
                set_once(
                    &mut return_due_on,
                    flag,
                    parse_date(flag, take_value(args, &mut index, flag)?)?,
                )?;
            }
            "--received-on" => {
                set_once(
                    &mut received_on,
                    flag,
                    parse_date(flag, take_value(args, &mut index, flag)?)?,
                )?;
            }
            "--notes" => {
                set_once(
                    &mut notes,
                    flag,
                    parse_notes(take_value(args, &mut index, flag)?)?,
                )?;
            }
            "--site" => {
                set_once(
                    &mut site_id,
                    flag,
                    parse_id(flag, take_value(args, &mut index, flag)?)?,
                )?;
            }
            "--from-source" => {
                set_once(
                    &mut from_source,
                    flag,
                    parse_id(flag, take_value(args, &mut index, flag)?)?,
                )?;
            }
            "--barcodes" => {
                set_once(
                    &mut barcodes_path,
                    flag,
                    take_value(args, &mut index, flag)?.to_string(),
                )?;
            }
            other if other.starts_with('-') => {
                return Err(ReclassifyFailure::invalid(format!(
                    "unknown option {other}"
                )));
            }
            other => {
                return Err(ReclassifyFailure::invalid(format!(
                    "unexpected argument {other}"
                )));
            }
        }
    }

    let lender_id = lender_id.ok_or_else(|| ReclassifyFailure::invalid("--lender is required"))?;
    let batch_reference = batch_reference
        .ok_or_else(|| ReclassifyFailure::invalid("--batch-reference is required"))?;
    let site_id = site_id.ok_or_else(|| ReclassifyFailure::invalid("--site is required"))?;
    let selection = match (from_source, barcodes_path) {
        (Some(_), Some(_)) => {
            return Err(ReclassifyFailure::invalid(
                "pass exactly one of --from-source or --barcodes",
            ));
        }
        (None, None) => {
            return Err(ReclassifyFailure::invalid(
                "pass exactly one of --from-source or --barcodes",
            ));
        }
        (Some(source_id), None) => Selection::FromSource(source_id),
        (None, Some(path)) => Selection::Barcodes(read_barcodes(&path)?),
    };

    Ok(Invocation {
        config_path,
        format: format.unwrap_or_default(),
        command: ReclassifyCommand {
            lender_id,
            batch_reference,
            return_due_on,
            received_on,
            notes: notes.flatten(),
            site_id,
            selection,
            apply,
        },
    })
}

fn take_value<'a>(
    args: &'a [String],
    index: &mut usize,
    flag: &str,
) -> Result<&'a str, ReclassifyFailure> {
    let value = args
        .get(*index)
        .ok_or_else(|| ReclassifyFailure::invalid(format!("{flag} requires a value")))?;
    *index += 1;
    Ok(value.as_str())
}

fn set_once<T>(slot: &mut Option<T>, flag: &str, value: T) -> Result<(), ReclassifyFailure> {
    if slot.is_some() {
        return Err(ReclassifyFailure::invalid(format!(
            "{flag} was given more than once"
        )));
    }
    *slot = Some(value);
    Ok(())
}

fn parse_id(flag: &str, raw: &str) -> Result<i64, ReclassifyFailure> {
    let id = raw
        .parse::<i64>()
        .map_err(|_| ReclassifyFailure::invalid(format!("{flag} must be an integer id")))?;
    if id <= 0 {
        return Err(ReclassifyFailure::invalid(format!(
            "{flag} must be a positive id"
        )));
    }
    Ok(id)
}

fn parse_date(flag: &str, raw: &str) -> Result<NaiveDate, ReclassifyFailure> {
    NaiveDate::parse_from_str(raw, "%Y-%m-%d")
        .map_err(|_| ReclassifyFailure::invalid(format!("{flag} must be YYYY-MM-DD")))
}

fn parse_format(raw: &str) -> Result<ReportFormat, ReclassifyFailure> {
    match raw {
        "json" => Ok(ReportFormat::Json),
        "text" => Ok(ReportFormat::Text),
        _ => Err(ReclassifyFailure::invalid("--format must be json or text")),
    }
}

fn parse_reference(raw: &str) -> Result<String, ReclassifyFailure> {
    let reference = raw.trim();
    if reference.is_empty() {
        return Err(ReclassifyFailure::invalid(
            "--batch-reference must not be empty",
        ));
    }
    if reference.chars().count() > REFERENCE_MAX_CHARS {
        return Err(ReclassifyFailure::invalid(format!(
            "--batch-reference must be at most {REFERENCE_MAX_CHARS} characters"
        )));
    }
    Ok(reference.to_string())
}

fn parse_notes(raw: &str) -> Result<Option<String>, ReclassifyFailure> {
    let notes = raw.trim();
    if notes.is_empty() {
        return Ok(None);
    }
    if notes.chars().count() > NOTES_MAX_CHARS {
        return Err(ReclassifyFailure::invalid(format!(
            "--notes must be at most {NOTES_MAX_CHARS} characters"
        )));
    }
    Ok(Some(notes.to_string()))
}

fn read_barcodes(path: &str) -> Result<Vec<String>, ReclassifyFailure> {
    let text = std::fs::read_to_string(path)
        .map_err(|err| ReclassifyFailure::invalid(format!("cannot read {path}: {err}")))?;
    let codes = parse_barcode_list(&text);
    if codes.is_empty() {
        return Err(ReclassifyFailure::invalid(format!(
            "{path} contains no barcodes"
        )));
    }
    Ok(codes)
}

/// One barcode per line, or a CSV whose header names a `barcode` column.
/// When the first line has a comma and no barcode header, the first column is used.
fn parse_barcode_list(text: &str) -> Vec<String> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    if lines.is_empty() {
        return Vec::new();
    }
    let csv = lines[0].contains(',');
    let records: Vec<Vec<String>> = if csv {
        lines.iter().copied().map(split_csv_line).collect()
    } else {
        lines
            .iter()
            .copied()
            .map(|line| vec![line.to_string()])
            .collect()
    };
    let (start, column) = if csv {
        barcode_column(records.first().map(Vec::as_slice).unwrap_or(&[]))
            .map_or((0, 0), |column| (1, column))
    } else {
        (0, 0)
    };
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for record in records.into_iter().skip(start) {
        let Some(raw) = record.get(column) else {
            continue;
        };
        let code = raw.trim();
        if code.is_empty() || !seen.insert(code.to_string()) {
            continue;
        }
        out.push(code.to_string());
    }
    out
}

fn barcode_column(fields: &[String]) -> Option<usize> {
    fields
        .iter()
        .position(|field| field.trim().eq_ignore_ascii_case("barcode"))
}

fn split_csv_line(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();
    while let Some(ch) = chars.next() {
        if in_quotes {
            if ch == '"' {
                if chars.peek() == Some(&'"') {
                    current.push('"');
                    chars.next();
                } else {
                    in_quotes = false;
                }
            } else {
                current.push(ch);
            }
        } else if ch == '"' && current.is_empty() {
            in_quotes = true;
        } else if ch == ',' {
            fields.push(current.trim().to_string());
            current.clear();
        } else {
            current.push(ch);
        }
    }
    fields.push(current.trim().to_string());
    fields
}

/// Dry-run or apply the command. Apply commits one transaction; a dry run rolls back.
pub async fn execute(
    pool: &PgPool,
    command: &ReclassifyCommand,
) -> Result<Report, ReclassifyFailure> {
    validate_command(command)?;
    let mut tx = pool.begin().await.map_err(failed)?;
    let result = execute_in_tx(&mut tx, command).await;
    match result {
        Ok(report) if command.apply => {
            tx.commit().await.map_err(failed)?;
            Ok(report)
        }
        Ok(report) => {
            tx.rollback().await.map_err(failed)?;
            Ok(report)
        }
        Err(err) => {
            let _ = tx.rollback().await;
            Err(err)
        }
    }
}

fn validate_command(command: &ReclassifyCommand) -> Result<(), ReclassifyFailure> {
    if command.lender_id <= 0 {
        return Err(ReclassifyFailure::invalid("--lender must be a positive id"));
    }
    if command.site_id <= 0 {
        return Err(ReclassifyFailure::invalid("--site must be a positive id"));
    }
    if command.batch_reference.trim().is_empty() {
        return Err(ReclassifyFailure::invalid(
            "--batch-reference must not be empty",
        ));
    }
    if let Selection::FromSource(source_id) = command.selection {
        if source_id <= 0 {
            return Err(ReclassifyFailure::invalid(
                "--from-source must be a positive id",
            ));
        }
        if source_id == command.site_id {
            return Err(ReclassifyFailure::invalid(
                "--from-source must not equal --site",
            ));
        }
    }
    if let Selection::Barcodes(codes) = &command.selection {
        if codes.is_empty() {
            return Err(ReclassifyFailure::invalid(
                "--barcodes did not contain any barcodes",
            ));
        }
    }
    Ok(())
}

async fn execute_in_tx(
    tx: &mut Tx<'_>,
    command: &ReclassifyCommand,
) -> Result<Report, ReclassifyFailure> {
    ensure_lender(tx, command.lender_id).await?;
    ensure_site(tx, command.site_id).await?;
    if let Selection::FromSource(source_id) = command.selection {
        ensure_source_exists(tx, source_id).await?;
    }
    let batch = resolve_batch(tx, command).await?;
    let planned = plan_items(tx, command, batch.id).await?;
    let biblio_ids = distinct_biblio_ids(&planned);
    let duplicates = find_duplicates(tx, &biblio_ids).await?;
    if command.apply {
        let batch_id = batch
            .id
            .ok_or_else(|| ReclassifyFailure::failed("batch id missing during apply"))?;
        apply_updates(tx, command, batch_id, &planned).await?;
    }
    let mut warnings = report_warnings(batch.return_due_on);
    if let Some(warning) = from_source_warning(tx, command).await? {
        warnings.push(warning);
    }
    let remaining_on_old_source = remaining_on_old_source(tx, command, &planned).await?;
    Ok(Report {
        dry_run: !command.apply,
        applied: command.apply,
        lender_id: command.lender_id,
        batch_reference: command.batch_reference.clone(),
        batch_id: batch.id,
        batch_action: batch.action,
        site_id: command.site_id,
        arrived_on: batch.arrived_on,
        return_due_on: batch.return_due_on,
        notes: batch.notes,
        warnings,
        remaining_on_old_source,
        summary: summarize(&planned),
        items: planned.into_iter().map(|item| item.line).collect(),
        possible_duplicate_records: duplicates,
    })
}

async fn ensure_lender(tx: &mut Tx<'_>, lender_id: i64) -> Result<(), ReclassifyFailure> {
    let row: Option<LenderRow> = sqlx::query_as("SELECT archived_at FROM lenders WHERE id = $1")
        .bind(lender_id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(failed)?;
    match row {
        None => Err(ReclassifyFailure::invalid(format!(
            "lender {lender_id} does not exist"
        ))),
        Some(row) if row.archived_at.is_some() => Err(ReclassifyFailure::invalid(format!(
            "lender {lender_id} is archived"
        ))),
        Some(_) => Ok(()),
    }
}

async fn ensure_site(tx: &mut Tx<'_>, site_id: i64) -> Result<(), ReclassifyFailure> {
    let row: Option<SiteRow> = sqlx::query_as(
        "SELECT archived_at, COALESCE(is_archive, 0)::int AS is_archive FROM sources WHERE id = $1",
    )
    .bind(site_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(failed)?;
    match row {
        None => Err(ReclassifyFailure::invalid(format!(
            "site {site_id} does not exist"
        ))),
        Some(row) if row.archived_at.is_some() || row.is_archive != 0 => Err(
            ReclassifyFailure::invalid(format!("site {site_id} is archived")),
        ),
        Some(_) => Ok(()),
    }
}

async fn ensure_source_exists(tx: &mut Tx<'_>, source_id: i64) -> Result<(), ReclassifyFailure> {
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM sources WHERE id = $1)")
        .bind(source_id)
        .fetch_one(&mut **tx)
        .await
        .map_err(failed)?;
    if exists {
        Ok(())
    } else {
        Err(ReclassifyFailure::invalid(format!(
            "--from-source {source_id} does not exist"
        )))
    }
}

async fn resolve_batch(
    tx: &mut Tx<'_>,
    command: &ReclassifyCommand,
) -> Result<ResolvedBatch, ReclassifyFailure> {
    let sql = if command.apply {
        "SELECT id, status, site_id, arrived_on, return_due_on, notes
         FROM deposit_batches
         WHERE lender_id = $1 AND reference = $2
         FOR UPDATE"
    } else {
        "SELECT id, status, site_id, arrived_on, return_due_on, notes
         FROM deposit_batches
         WHERE lender_id = $1 AND reference = $2"
    };
    let existing: Option<BatchRow> = sqlx::query_as(sql)
        .bind(command.lender_id)
        .bind(&command.batch_reference)
        .fetch_optional(&mut **tx)
        .await
        .map_err(failed)?;
    if let Some(batch) = existing {
        if batch.site_id != command.site_id {
            let code = crate::error::error_code::DEPOSIT_SITE_MISMATCH;
            return Err(ReclassifyFailure::invalid_coded(
                code,
                format!(
                    "{code}: deposit batch {} for lender {} belongs to site {}, not --site {}",
                    command.batch_reference, command.lender_id, batch.site_id, command.site_id
                ),
            ));
        }
        if batch.status != "active" {
            return Err(ReclassifyFailure::invalid(format!(
                "deposit batch {} for lender {} is {}; only an active batch can be reused",
                command.batch_reference, command.lender_id, batch.status
            )));
        }
        return Ok(ResolvedBatch {
            id: Some(batch.id),
            action: BatchAction::Reuse,
            arrived_on: batch.arrived_on,
            return_due_on: batch.return_due_on,
            notes: batch.notes,
        });
    }

    let planned = plan_new_batch(command)?;
    let id = if command.apply {
        Some(insert_batch(tx, command, &planned).await?)
    } else {
        None
    };
    Ok(ResolvedBatch {
        id,
        action: BatchAction::Create,
        arrived_on: planned.arrived_on,
        return_due_on: planned.return_due_on,
        notes: planned.notes,
    })
}

const RETURN_DUE_ON_PAST: &str = "return_due_on_past";
const FROM_SOURCE_MIXED: &str = "from_source_mixed";

/// Copies at `--from-source` that sit in more than one deposit batch, or that
/// have an open loan checked out at a site other than `--site`. A null loan
/// site is unknown and is not a conflict. Archived copies are outside the
/// selection, so they are not counted.
async fn from_source_warning(
    tx: &mut Tx<'_>,
    command: &ReclassifyCommand,
) -> Result<Option<ReportWarning>, ReclassifyFailure> {
    let Selection::FromSource(source_id) = command.selection else {
        return Ok(None);
    };
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)::bigint
         FROM items i
         WHERE i.source_id = $1
           AND i.archived_at IS NULL
           AND (
             (
               (
                 SELECT COUNT(DISTINCT deposit_batch_id)
                 FROM items
                 WHERE source_id = $1
                   AND archived_at IS NULL
                   AND deposit_batch_id IS NOT NULL
               ) > 1
               AND i.deposit_batch_id IS NOT NULL
             )
             OR EXISTS (
               SELECT 1
               FROM loans l
               WHERE l.item_id = i.id
                 AND l.returned_at IS NULL
                 AND l.site_id IS NOT NULL
                 AND l.site_id <> $2
             )
           )",
    )
    .bind(source_id)
    .bind(command.site_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(failed)?;
    if count == 0 {
        return Ok(None);
    }
    Ok(Some(ReportWarning {
        code: FROM_SOURCE_MIXED.to_string(),
        message: format!(
            "{count} copies at --from-source are in more than one deposit batch or have an open loan whose site differs from --site"
        ),
        count: Some(count),
    }))
}

/// The date that will apply is the new batch's `--return-due-on`, or the stored
/// date when an existing batch is reused. Today is the UTC date, matching
/// `cap_loan_due_date`: a due date of today already blocks checkout.
fn report_warnings(return_due_on: NaiveDate) -> Vec<ReportWarning> {
    let mut warnings = Vec::new();
    if return_due_on <= Utc::now().date_naive() {
        warnings.push(ReportWarning {
            code: RETURN_DUE_ON_PAST.to_string(),
            message: format!(
                "return due date {return_due_on} is today or in the past; copies will immediately be refused for loans (deposit_return_due) and holds"
            ),
            count: None,
        });
    }
    warnings
}

/// Every item row still pointing at the old source, including archived rows.
/// A dry run subtracts the copies this run would move. Apply counts after the
/// updates, still inside the same transaction.
async fn remaining_on_old_source(
    tx: &mut Tx<'_>,
    command: &ReclassifyCommand,
    planned: &[PlannedItem],
) -> Result<Option<i64>, ReclassifyFailure> {
    let Selection::FromSource(source_id) = command.selection else {
        return Ok(None);
    };
    let current: i64 =
        sqlx::query_scalar("SELECT COUNT(*)::bigint FROM items WHERE source_id = $1")
            .bind(source_id)
            .fetch_one(&mut **tx)
            .await
            .map_err(failed)?;
    if command.apply {
        return Ok(Some(current));
    }
    let moving = planned.iter().filter(|item| item.update.is_some()).count() as i64;
    Ok(Some(current - moving))
}

fn plan_new_batch(command: &ReclassifyCommand) -> Result<NewBatch, ReclassifyFailure> {
    let return_due_on = command.return_due_on.ok_or_else(|| {
        ReclassifyFailure::invalid(
            "--return-due-on is required when the deposit batch does not already exist",
        )
    })?;
    let arrived_on = command
        .received_on
        .unwrap_or_else(|| Utc::now().date_naive());
    if return_due_on < arrived_on {
        return Err(ReclassifyFailure::invalid(
            "return-due-on must be on or after received-on (today when --received-on is omitted)",
        ));
    }
    Ok(NewBatch {
        arrived_on,
        return_due_on,
        notes: command.notes.clone(),
    })
}

async fn insert_batch(
    tx: &mut Tx<'_>,
    command: &ReclassifyCommand,
    planned: &NewBatch,
) -> Result<i64, ReclassifyFailure> {
    let id = sqlx::query_scalar(
        "INSERT INTO deposit_batches (lender_id, reference, site_id, arrived_on, return_due_on, notes)
         VALUES ($1, $2, $3, $4, $5, $6)
         RETURNING id",
    )
    .bind(command.lender_id)
    .bind(&command.batch_reference)
    .bind(command.site_id)
    .bind(planned.arrived_on)
    .bind(planned.return_due_on)
    .bind(&planned.notes)
    .fetch_one(&mut **tx)
    .await
    .map_err(map_batch_insert)?;
    Ok(id)
}

fn map_batch_insert(err: sqlx::Error) -> ReclassifyFailure {
    let code = match &err {
        sqlx::Error::Database(db) => db.code().map(std::borrow::Cow::into_owned),
        _ => None,
    };
    match code.as_deref() {
        Some("23505") => ReclassifyFailure::failed(
            "A deposit batch with this reference already exists for that lender",
        ),
        Some("23514") => ReclassifyFailure::invalid(
            "return-due-on must be on or after received-on (today when --received-on is omitted)",
        ),
        _ => failed(err),
    }
}

async fn plan_items(
    tx: &mut Tx<'_>,
    command: &ReclassifyCommand,
    target_batch_id: Option<i64>,
) -> Result<Vec<PlannedItem>, ReclassifyFailure> {
    match &command.selection {
        Selection::FromSource(source_id) => {
            let rows = load_from_source(tx, *source_id, command.apply).await?;
            let mut planned: Vec<PlannedItem> = rows
                .iter()
                .map(|row| plan_row(row, command.site_id, target_batch_id))
                .collect();
            planned.sort_by(|left, right| {
                left.line
                    .barcode
                    .cmp(&right.line.barcode)
                    .then(left.line.item_id.cmp(&right.line.item_id))
            });
            Ok(planned)
        }
        Selection::Barcodes(codes) => {
            let rows = load_barcodes(tx, codes, command.apply).await?;
            let by_barcode = rows_by_barcode(rows);
            let mut planned = Vec::with_capacity(codes.len());
            for code in codes {
                planned.push(match by_barcode.get(code) {
                    Some(row) => plan_row(row, command.site_id, target_batch_id),
                    None => PlannedItem {
                        line: unknown_line(code, command.site_id),
                        update: None,
                        biblio_id: None,
                    },
                });
            }
            Ok(planned)
        }
    }
}

fn rows_by_barcode(rows: Vec<Candidate>) -> HashMap<String, Candidate> {
    let mut by_barcode = HashMap::with_capacity(rows.len());
    for row in rows {
        if let Some(code) = row.barcode.clone() {
            by_barcode.insert(code, row);
        }
    }
    by_barcode
}

fn plan_row(row: &Candidate, site_id: i64, target_batch_id: Option<i64>) -> PlannedItem {
    let decision = decide(&row.facts(), target_batch_id, site_id);
    let (disposition, reason, update) = match decision {
        Decision::Reclassify => (
            LineDisposition::Reclassify,
            None,
            Some(ItemUpdate {
                item_id: row.id,
                old_source_id: row.source_id,
                barcode: row.barcode.clone(),
                biblio_id: row.biblio_id,
            }),
        ),
        Decision::Already => (
            LineDisposition::AlreadyReclassified,
            Some(LineReason::AlreadyReclassified),
            None,
        ),
        Decision::Refused(reason) => (LineDisposition::Refused, Some(reason), None),
    };
    PlannedItem {
        line: ReportLine {
            barcode: row.barcode.clone(),
            item_id: Some(row.id),
            biblio_id: row.biblio_id,
            title: row.title.clone(),
            old_source_id: row.source_id,
            new_source_id: Some(site_id),
            disposition,
            reason,
            on_loan: row.on_loan,
            has_holds: row.has_holds,
        },
        update,
        biblio_id: row.biblio_id,
    }
}

fn unknown_line(barcode: &str, site_id: i64) -> ReportLine {
    ReportLine {
        barcode: Some(barcode.to_string()),
        item_id: None,
        biblio_id: None,
        title: None,
        old_source_id: None,
        new_source_id: Some(site_id),
        disposition: LineDisposition::Refused,
        reason: Some(LineReason::BarcodeUnknown),
        on_loan: false,
        has_holds: false,
    }
}

async fn load_from_source(
    tx: &mut Tx<'_>,
    source_id: i64,
    lock: bool,
) -> Result<Vec<Candidate>, ReclassifyFailure> {
    let sql = if lock {
        format!(
            "{ITEM_SELECT} WHERE i.source_id = $1 AND i.archived_at IS NULL ORDER BY i.id FOR UPDATE OF i"
        )
    } else {
        format!("{ITEM_SELECT} WHERE i.source_id = $1 AND i.archived_at IS NULL ORDER BY i.id")
    };
    sqlx::query_as(&sql)
        .bind(source_id)
        .fetch_all(&mut **tx)
        .await
        .map_err(failed)
}

async fn load_barcodes(
    tx: &mut Tx<'_>,
    codes: &[String],
    lock: bool,
) -> Result<Vec<Candidate>, ReclassifyFailure> {
    let sql = if lock {
        format!("{ITEM_SELECT} WHERE i.barcode = ANY($1) ORDER BY i.id FOR UPDATE OF i")
    } else {
        format!("{ITEM_SELECT} WHERE i.barcode = ANY($1) ORDER BY i.id")
    };
    sqlx::query_as(&sql)
        .bind(codes)
        .fetch_all(&mut **tx)
        .await
        .map_err(failed)
}

fn distinct_biblio_ids(planned: &[PlannedItem]) -> Vec<i64> {
    let mut ids: Vec<i64> = planned.iter().filter_map(|item| item.biblio_id).collect();
    ids.sort_unstable();
    ids.dedup();
    ids
}

async fn find_duplicates(
    tx: &mut Tx<'_>,
    biblio_ids: &[i64],
) -> Result<Vec<PossibleDuplicateRecord>, ReclassifyFailure> {
    if biblio_ids.is_empty() {
        return Ok(Vec::new());
    }
    let rows: Vec<DuplicateRow> = sqlx::query_as(
        r#"
        WITH selected AS (
            SELECT DISTINCT unnest($1::bigint[]) AS biblio_id
        ),
        norms AS (
            SELECT b.id,
                   upper(regexp_replace(b.isbn, '[^A-Za-z0-9]', '', 'g')) AS isbn_norm
            FROM biblios b
            WHERE b.isbn IS NOT NULL
        )
        SELECT s.id AS record_id,
               o.id AS other_record_id,
               s.isbn_norm AS isbn
        FROM norms s
        JOIN selected sel ON sel.biblio_id = s.id
        JOIN norms o ON o.isbn_norm = s.isbn_norm AND o.id <> s.id
        WHERE s.isbn_norm <> ''
          AND NOT EXISTS (SELECT 1 FROM selected x WHERE x.biblio_id = o.id)
        ORDER BY s.id, o.id
        "#,
    )
    .bind(biblio_ids)
    .fetch_all(&mut **tx)
    .await
    .map_err(failed)?;
    Ok(rows
        .into_iter()
        .map(|row| PossibleDuplicateRecord {
            record_id: row.record_id,
            other_record_id: row.other_record_id,
            isbn: row.isbn,
            reason: DuplicateReason::PossibleDuplicateRecord,
        })
        .collect())
}

fn summarize(planned: &[PlannedItem]) -> Summary {
    let mut summary = Summary::default();
    for item in planned {
        match item.line.disposition {
            LineDisposition::Reclassify => {
                summary.to_reclassify += 1;
                if item.line.on_loan {
                    summary.on_loan += 1;
                }
                if item.line.has_holds {
                    summary.with_holds += 1;
                }
            }
            LineDisposition::Refused => summary.refused += 1,
            LineDisposition::AlreadyReclassified => summary.already_reclassified += 1,
        }
    }
    summary
}

async fn apply_updates(
    tx: &mut Tx<'_>,
    command: &ReclassifyCommand,
    batch_id: i64,
    planned: &[PlannedItem],
) -> Result<(), ReclassifyFailure> {
    for item in planned {
        let Some(update) = &item.update else {
            continue;
        };
        let result = sqlx::query(
            "UPDATE items
             SET deposit_batch_id = $1, source_id = $2, updated_at = NOW()
             WHERE id = $3",
        )
        .bind(batch_id)
        .bind(command.site_id)
        .bind(update.item_id)
        .execute(&mut **tx)
        .await
        .map_err(failed)?;
        if result.rows_affected() != 1 {
            return Err(ReclassifyFailure::failed(format!(
                "expected to update item {} and updated {} rows",
                update.item_id,
                result.rows_affected()
            )));
        }
        let payload = serde_json::to_value(ReclassifyAuditPayload {
            actor_type: ACTOR_TYPE,
            actor_name: ACTOR_NAME,
            lender_id: command.lender_id,
            batch_id,
            old_source_id: update.old_source_id,
            new_source_id: command.site_id,
            barcode: update.barcode.clone(),
            item_id: update.item_id,
            biblio_id: update.biblio_id,
        })
        .map_err(|err| ReclassifyFailure::failed(format!("audit payload: {err}")))?;
        Repository::audit_insert_tx(
            tx,
            audit::event::DEPOSIT_BATCH_RECLASSIFIED,
            None,
            Some("item"),
            Some(update.item_id),
            None,
            Some(payload),
            "success",
            None,
            None,
            None,
        )
        .await
        .map_err(failed)?;
    }
    Ok(())
}

/// Render the report. JSON object keys are camelCase and ids are decimal strings.
#[must_use]
pub fn render_report(report: &Report, format: ReportFormat) -> String {
    match format {
        ReportFormat::Text => render_text(report),
        ReportFormat::Json => match serde_json::to_string_pretty(report) {
            Ok(body) => format!("{body}\n"),
            Err(err) => format!("{{\"error\":\"failed to encode report: {err}\"}}\n"),
        },
    }
}

fn render_text(report: &Report) -> String {
    let mut out = String::new();
    for warning in &report.warnings {
        let count = match warning.count {
            Some(count) => format!("\tcount={count}"),
            None => String::new(),
        };
        push_line(
            &mut out,
            format!("WARNING\tcode={}\t{}{count}", warning.code, warning.message),
        );
    }
    let mode = if report.applied { "apply" } else { "dry-run" };
    push_line(&mut out, format!("mode={mode}"));
    push_line(&mut out, format!("lender_id={}", report.lender_id));
    push_line(
        &mut out,
        format!("batch_reference={}", report.batch_reference),
    );
    push_line(
        &mut out,
        format!("batch_action={}", report.batch_action.as_str()),
    );
    push_line(
        &mut out,
        format!(
            "batch_id={}",
            report.batch_id.map(|id| id.to_string()).unwrap_or_default()
        ),
    );
    push_line(&mut out, format!("site_id={}", report.site_id));
    push_line(&mut out, format!("arrived_on={}", report.arrived_on));
    push_line(&mut out, format!("return_due_on={}", report.return_due_on));
    for line in &report.items {
        push_line(&mut out, format_line(line));
    }
    for duplicate in &report.possible_duplicate_records {
        push_line(
            &mut out,
            format!(
                "possible_duplicate_record\trecord_id={}\tother_record_id={}\tisbn={}",
                duplicate.record_id, duplicate.other_record_id, duplicate.isbn
            ),
        );
    }
    push_line(
        &mut out,
        format!(
            "summary\tto_reclassify={}\ton_loan={}\twith_holds={}\trefused={}\talready_reclassified={}",
            report.summary.to_reclassify,
            report.summary.on_loan,
            report.summary.with_holds,
            report.summary.refused,
            report.summary.already_reclassified
        ),
    );
    if let Some(remaining) = report.remaining_on_old_source {
        push_line(&mut out, format!("remaining_on_old_source={remaining}"));
        if remaining == 0 {
            push_line(&mut out, "old source can be archived manually");
        }
    }
    out
}

fn format_line(line: &ReportLine) -> String {
    let mut parts = Vec::new();
    parts.push(match line.disposition {
        LineDisposition::Reclassify => "reclassify".to_string(),
        LineDisposition::Refused => "refused".to_string(),
        LineDisposition::AlreadyReclassified => "already_reclassified".to_string(),
    });
    if let Some(reason) = line.reason {
        parts.push(format!("reason={}", reason_code(reason)));
    }
    parts.push(format!("barcode={}", line.barcode.as_deref().unwrap_or("")));
    parts.push(format!("item_id={}", fmt_opt_id(line.item_id)));
    parts.push(format!("biblio_id={}", fmt_opt_id(line.biblio_id)));
    parts.push(format!(
        "title={}",
        one_line(line.title.as_deref().unwrap_or(""))
    ));
    parts.push(format!("old_source_id={}", fmt_opt_id(line.old_source_id)));
    parts.push(format!("new_source_id={}", fmt_opt_id(line.new_source_id)));
    parts.push(format!("on_loan={}", line.on_loan));
    parts.push(format!("has_holds={}", line.has_holds));
    parts.join("\t")
}

fn reason_code(reason: LineReason) -> &'static str {
    match reason {
        LineReason::BarcodeUnknown => "barcode_unknown",
        LineReason::ItemArchived => "item_archived",
        LineReason::ItemInOtherBatch => "item_in_other_batch",
        LineReason::ItemInTransit => "item_in_transit",
        LineReason::AlreadyReclassified => "already_reclassified",
    }
}

fn fmt_opt_id(id: Option<i64>) -> String {
    id.map(|value| value.to_string()).unwrap_or_default()
}

fn one_line(value: &str) -> String {
    value.replace(['\n', '\r', '\t'], " ")
}

fn push_line(out: &mut String, line: impl AsRef<str>) {
    out.push_str(line.as_ref());
    out.push('\n');
}

#[cfg(test)]
mod tests {
    use super::{
        decide, parse_barcode_list, parse_invocation, reason_code, render_report, Decision,
        ItemFacts, LineReason, ReportFormat, Selection,
    };
    use chrono::NaiveDate;

    fn facts(batch: Option<i64>, source: Option<i64>, transit: bool) -> ItemFacts {
        ItemFacts {
            archived: false,
            withdrawn: false,
            deposit_batch_id: batch,
            source_id: source,
            in_transit: transit,
        }
    }

    #[test]
    fn decision_covers_each_outcome() {
        let site = 4;
        let target = Some(9);
        assert_eq!(
            decide(&facts(None, Some(1), false), target, site),
            Decision::Reclassify
        );
        assert_eq!(
            decide(&facts(target, Some(site), true), target, site),
            Decision::Already
        );
        assert_eq!(
            decide(&facts(Some(3), Some(1), false), target, site),
            Decision::Refused(LineReason::ItemInOtherBatch)
        );
        assert_eq!(
            decide(&facts(Some(3), Some(1), false), None, site),
            Decision::Refused(LineReason::ItemInOtherBatch)
        );
        assert_eq!(
            decide(&facts(None, Some(1), true), target, site),
            Decision::Refused(LineReason::ItemInTransit)
        );
        assert_eq!(
            decide(&facts(target, Some(1), false), target, site),
            Decision::Reclassify
        );
        let archived = ItemFacts {
            archived: true,
            ..facts(None, Some(1), false)
        };
        assert_eq!(
            decide(&archived, target, site),
            Decision::Refused(LineReason::ItemArchived)
        );
        let withdrawn = ItemFacts {
            withdrawn: true,
            ..facts(target, Some(site), false)
        };
        assert_eq!(
            decide(&withdrawn, target, site),
            Decision::Refused(LineReason::ItemArchived)
        );
        assert_eq!(reason_code(LineReason::ItemInTransit), "item_in_transit");
    }

    #[test]
    fn barcode_list_accepts_lines_csv_and_dedupes() {
        assert_eq!(
            parse_barcode_list(" ABC \n\nABC\nDEF\n"),
            vec!["ABC".to_string(), "DEF".to_string()]
        );
        assert_eq!(
            parse_barcode_list("title,barcode\nTale,\"ABC\"\nTale,ABC\nTale,XYZ\n"),
            vec!["ABC".to_string(), "XYZ".to_string()]
        );
        assert_eq!(
            parse_barcode_list("ABC,ignored\nDEF,also\n"),
            vec!["ABC".to_string(), "DEF".to_string()]
        );
        assert!(parse_barcode_list("   \n").is_empty());
    }

    #[test]
    fn parse_requires_one_selection_and_keeps_dry_run_default() {
        let missing = parse_invocation(&[
            "--lender".into(),
            "1".into(),
            "--batch-reference".into(),
            " REF ".into(),
            "--site".into(),
            "2".into(),
        ])
        .unwrap_err();
        assert_eq!(missing.exit_code(), 2);

        let both = parse_invocation(&[
            "--lender".into(),
            "1".into(),
            "--batch-reference".into(),
            "REF".into(),
            "--site".into(),
            "2".into(),
            "--from-source".into(),
            "3".into(),
            "--barcodes".into(),
            "missing.csv".into(),
        ])
        .unwrap_err();
        assert!(both.to_string().contains("exactly one"));

        let path =
            std::env::temp_dir().join(format!("elidune-reclassify-{}.txt", std::process::id()));
        std::fs::write(&path, "B1\nB1\nB2\n").unwrap();
        let invocation = parse_invocation(&[
            "--lender".into(),
            "12".into(),
            "--batch-reference".into(),
            " CITY-2024 ".into(),
            "--return-due-on".into(),
            "2026-09-01".into(),
            "--received-on".into(),
            "2026-03-01".into(),
            "--notes".into(),
            " spring ".into(),
            "--site".into(),
            "4".into(),
            "--barcodes".into(),
            path.display().to_string(),
            "--format".into(),
            "json".into(),
        ])
        .unwrap();
        let _ = std::fs::remove_file(&path);
        assert!(!invocation.command.apply);
        assert_eq!(invocation.format, ReportFormat::Json);
        assert_eq!(invocation.command.batch_reference, "CITY-2024");
        assert_eq!(invocation.command.notes.as_deref(), Some("spring"));
        assert_eq!(
            invocation.command.return_due_on,
            Some(NaiveDate::from_ymd_opt(2026, 9, 1).unwrap())
        );
        assert_eq!(
            invocation.command.selection,
            Selection::Barcodes(vec!["B1".into(), "B2".into()])
        );
    }

    #[test]
    fn text_report_lists_reason_codes() {
        let report = super::Report {
            dry_run: true,
            applied: false,
            lender_id: 12,
            batch_reference: "REF".into(),
            batch_id: None,
            batch_action: super::BatchAction::Create,
            site_id: 4,
            arrived_on: NaiveDate::from_ymd_opt(2026, 3, 1).unwrap(),
            return_due_on: NaiveDate::from_ymd_opt(2026, 9, 1).unwrap(),
            notes: None,
            warnings: Vec::new(),
            remaining_on_old_source: None,
            items: vec![super::ReportLine {
                barcode: Some("NOPE".into()),
                item_id: None,
                biblio_id: None,
                title: None,
                old_source_id: None,
                new_source_id: Some(4),
                disposition: super::LineDisposition::Refused,
                reason: Some(LineReason::BarcodeUnknown),
                on_loan: false,
                has_holds: false,
            }],
            possible_duplicate_records: vec![super::PossibleDuplicateRecord {
                record_id: 8,
                other_record_id: 9,
                isbn: "9780306406157".into(),
                reason: super::DuplicateReason::PossibleDuplicateRecord,
            }],
            summary: super::Summary {
                to_reclassify: 0,
                on_loan: 0,
                with_holds: 0,
                refused: 1,
                already_reclassified: 0,
            },
        };
        let text = render_report(&report, ReportFormat::Text);
        assert!(text.contains("mode=dry-run"));
        assert!(text.contains("reason=barcode_unknown"));
        assert!(text.contains("possible_duplicate_record"));
        let json = render_report(&report, ReportFormat::Json);
        assert!(json.contains("\"lenderId\": \"12\""));
        assert!(json.contains("\"barcode_unknown\""));
        assert!(json.contains("\"possible_duplicate_record\""));
        assert!(json.contains("\"warnings\": []"));
        assert!(json.contains("\"remainingOnOldSource\": null"));
    }

    #[test]
    fn return_due_warning_leads_the_text_report() {
        let report = super::Report {
            dry_run: true,
            applied: false,
            lender_id: 12,
            batch_reference: "REF".into(),
            batch_id: None,
            batch_action: super::BatchAction::Create,
            site_id: 4,
            arrived_on: NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(),
            return_due_on: NaiveDate::from_ymd_opt(2020, 6, 1).unwrap(),
            notes: None,
            warnings: vec![super::ReportWarning {
                code: "return_due_on_past".into(),
                message:
                    "copies will immediately be refused for loans (deposit_return_due) and holds"
                        .into(),
                count: None,
            }],
            remaining_on_old_source: Some(0),
            items: Vec::new(),
            possible_duplicate_records: Vec::new(),
            summary: super::Summary::default(),
        };
        let text = render_report(&report, ReportFormat::Text);
        assert!(text.starts_with("WARNING\tcode=return_due_on_past\t"));
        assert!(text.contains("deposit_return_due"));
        assert!(text.find("WARNING").unwrap() < text.find("mode=").unwrap());
        assert!(text
            .trim_end()
            .ends_with("old source can be archived manually"));
        assert!(text.find("summary\t").unwrap() < text.find("remaining_on_old_source=0").unwrap());
        let json = render_report(&report, ReportFormat::Json);
        assert!(json.contains("\"code\": \"return_due_on_past\""));
        assert!(json.contains("\"warnings\""));
    }
}
