//! Reclassify historical deposit copies onto a lender batch.
mod common;

use chrono::{DateTime, TimeZone, Utc};
use common::{fixtures, TestApp};
use elidune_server::deposit_reclassify::{
    execute, BatchAction, LineDisposition, LineReason, ReclassifyCommand, ReclassifyFailure,
    Selection,
};
use sqlx::PgPool;

fn command(
    lender_id: i64,
    reference: &str,
    site_id: i64,
    selection: Selection,
    apply: bool,
) -> ReclassifyCommand {
    ReclassifyCommand {
        lender_id,
        batch_reference: reference.to_string(),
        return_due_on: Some(chrono::NaiveDate::from_ymd_opt(2027, 9, 1).unwrap()),
        received_on: Some(chrono::NaiveDate::from_ymd_opt(2027, 3, 1).unwrap()),
        notes: Some("historical import".into()),
        site_id,
        selection,
        apply,
    }
}

async fn source(pool: &PgPool, name: &str) -> i64 {
    sqlx::query_scalar("INSERT INTO sources (name) VALUES ($1) RETURNING id")
        .bind(name)
        .fetch_one(pool)
        .await
        .expect("source")
}

async fn lender(pool: &PgPool, name: &str) -> i64 {
    sqlx::query_scalar("INSERT INTO lenders (name) VALUES ($1) RETURNING id")
        .bind(name)
        .fetch_one(pool)
        .await
        .expect("lender")
}

async fn biblio(pool: &PgPool, title: &str, isbn: Option<&str>) -> i64 {
    sqlx::query_scalar(
        "INSERT INTO biblios (title, isbn, media_type) VALUES ($1, $2, 'printedText') RETURNING id",
    )
    .bind(title)
    .bind(isbn)
    .fetch_one(pool)
    .await
    .expect("biblio")
}

async fn item(pool: &PgPool, biblio_id: i64, source_id: i64, barcode: &str) -> i64 {
    sqlx::query_scalar(
        "INSERT INTO items (biblio_id, source_id, barcode, borrowable)
         VALUES ($1, $2, $3, TRUE) RETURNING id",
    )
    .bind(biblio_id)
    .bind(source_id)
    .bind(barcode)
    .fetch_one(pool)
    .await
    .expect("item")
}

async fn reader(pool: &PgPool, login: &str) -> i64 {
    sqlx::query_scalar(
        "INSERT INTO users (login, account_type, password) VALUES ($1, 'reader', 'x') RETURNING id",
    )
    .bind(login)
    .fetch_one(pool)
    .await
    .expect("reader")
}

fn next_id() -> i64 {
    use std::sync::atomic::{AtomicI64, Ordering};
    static N: AtomicI64 = AtomicI64::new(1);
    Utc::now()
        .timestamp_micros()
        .saturating_mul(10)
        .saturating_add(N.fetch_add(1, Ordering::Relaxed))
}

fn due() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 12, 1, 15, 0, 0).unwrap()
}

struct LoanSnap {
    expiry_at: Option<DateTime<Utc>>,
    returned_at: Option<DateTime<Utc>>,
    nb_renews: i16,
}

async fn loan_snap(pool: &PgPool, loan_id: i64) -> LoanSnap {
    let row: (Option<DateTime<Utc>>, Option<DateTime<Utc>>, i16) =
        sqlx::query_as("SELECT expiry_at, returned_at, nb_renews FROM loans WHERE id = $1")
            .bind(loan_id)
            .fetch_one(pool)
            .await
            .expect("loan");
    LoanSnap {
        expiry_at: row.0,
        returned_at: row.1,
        nb_renews: row.2,
    }
}

async fn item_place(
    pool: &PgPool,
    item_id: i64,
) -> (Option<i64>, Option<i64>, Option<DateTime<Utc>>) {
    sqlx::query_as(
        "SELECT deposit_batch_id, source_id, deposit_returned_at FROM items WHERE id = $1",
    )
    .bind(item_id)
    .fetch_one(pool)
    .await
    .expect("item place")
}

#[tokio::test]
async fn dry_run_writes_nothing_apply_is_idempotent_and_reports_duplicates() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let pool = app.state.services.repository.pool();
    let token = fixtures::unique_token();
    let site_id = source(pool, &format!("site-{token}")).await;
    let old_source = source(pool, &format!("lender-site-{token}")).await;
    let lender_id = lender(pool, &format!("Lender {token}")).await;
    let user_id = reader(pool, &format!("reader-{token}")).await;

    let digits = format!("{:013}", fixtures::unique_suffix() % 10_000_000_000_000);
    let dashed = format!(
        "{}-{}-{}-{}-{}",
        &digits[0..3],
        &digits[3..4],
        &digits[4..7],
        &digits[7..12],
        &digits[12..13]
    );
    let normalized = digits.clone();
    let spaced = format!("{} {}-{}", &digits[0..3], &digits[3..4], &digits[4..]);
    let biblio_a = biblio(pool, "Selected A", Some(&dashed)).await;
    let biblio_b = biblio(pool, "Selected B", Some(&normalized)).await;
    // Punctuation and spacing only. `Isbn` keeps letters, so an "ISBN " prefix would not match.
    let outside = biblio(pool, "Already owned", Some(&spaced)).await;
    let quiet_isbn = format!("2{}", &digits[1..]);
    let quiet = biblio(pool, "No match", Some(&quiet_isbn)).await;

    let open_barcode = format!("OPEN-{token}");
    let quiet_barcode = format!("QUIET-{token}");
    let peer_barcode = format!("PEER-{token}");
    let open_item = item(pool, biblio_a, old_source, &open_barcode).await;
    let quiet_item = item(pool, quiet, old_source, &quiet_barcode).await;
    let selected_peer = item(pool, biblio_b, old_source, &peer_barcode).await;

    let loan_id: i64 = sqlx::query_scalar(
        "INSERT INTO loans (user_id, item_id, expiry_at, nb_renews)
         VALUES ($1, $2, $3, 2) RETURNING id",
    )
    .bind(user_id)
    .bind(open_item)
    .bind(due())
    .fetch_one(pool)
    .await
    .expect("loan");
    let returned_loan: i64 = sqlx::query_scalar(
        "INSERT INTO loans (user_id, item_id, expiry_at, returned_at, nb_renews)
         VALUES ($1, $2, $3, NOW(), 1) RETURNING id",
    )
    .bind(user_id)
    .bind(quiet_item)
    .bind(due())
    .fetch_one(pool)
    .await
    .expect("returned loan");
    let hold_id = next_id();
    sqlx::query(
        "INSERT INTO holds (id, user_id, biblio_id, item_id, status, position)
         VALUES ($1, $2, $3, $4, 'pending', 1)",
    )
    .bind(hold_id)
    .bind(user_id)
    .bind(biblio_a)
    .bind(open_item)
    .execute(pool)
    .await
    .expect("hold");
    sqlx::query(
        "INSERT INTO holds (id, user_id, biblio_id, item_id, status, position)
         VALUES ($1, $2, $3, $4, 'cancelled', 1)",
    )
    .bind(next_id())
    .bind(user_id)
    .bind(quiet)
    .bind(quiet_item)
    .execute(pool)
    .await
    .expect("cancelled hold");
    let fine_id = next_id();
    sqlx::query(
        "INSERT INTO fines (id, loan_id, user_id, amount, status, charge_type)
         VALUES ($1, $2, $3, 150, 'pending', 'overdue')",
    )
    .bind(fine_id)
    .bind(loan_id)
    .bind(user_id)
    .execute(pool)
    .await
    .expect("fine");

    sqlx::query("UPDATE items SET updated_at = '2020-01-01T00:00:00Z' WHERE id = $1")
        .bind(open_item)
        .execute(pool)
        .await
        .expect("stamp");

    let reference = format!("HIST-{token}");
    let selection = Selection::FromSource(old_source);
    let dry = command(lender_id, &reference, site_id, selection.clone(), false);
    let report = execute(pool, &dry).await.expect("dry run");
    assert!(report.dry_run);
    assert!(!report.applied);
    assert_eq!(report.batch_action, BatchAction::Create);
    assert!(report.batch_id.is_none());
    assert_eq!(report.summary.to_reclassify, 3);
    assert_eq!(report.summary.on_loan, 1);
    assert_eq!(report.summary.with_holds, 1);
    assert_eq!(report.summary.refused, 0);

    let open_line = report
        .items
        .iter()
        .find(|line| line.item_id == Some(open_item))
        .expect("open line");
    assert_eq!(open_line.disposition, LineDisposition::Reclassify);
    assert!(open_line.on_loan && open_line.has_holds);
    assert_eq!(open_line.old_source_id, Some(old_source));
    assert_eq!(open_line.new_source_id, Some(site_id));
    assert_eq!(open_line.biblio_id, Some(biblio_a));
    assert_eq!(open_line.title.as_deref(), Some("Selected A"));
    let quiet_line = report
        .items
        .iter()
        .find(|line| line.item_id == Some(quiet_item))
        .expect("quiet line");
    assert!(!quiet_line.on_loan && !quiet_line.has_holds);

    let pairs: Vec<(i64, i64)> = report
        .possible_duplicate_records
        .iter()
        .map(|row| (row.record_id, row.other_record_id))
        .collect();
    assert!(pairs.contains(&(biblio_a, outside)), "{pairs:?}");
    assert!(pairs.contains(&(biblio_b, outside)), "{pairs:?}");
    assert!(!pairs.iter().any(|(left, right)| {
        (*left == biblio_a && *right == biblio_b) || (*left == biblio_b && *right == biblio_a)
    }));
    assert!(report
        .possible_duplicate_records
        .iter()
        .all(|row| row.isbn == normalized));

    let batches: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM deposit_batches WHERE lender_id = $1 AND reference = $2",
    )
    .bind(lender_id)
    .bind(&reference)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(batches, 0);
    assert_eq!(
        item_place(pool, open_item).await,
        (None, Some(old_source), None)
    );
    let stamped: Option<DateTime<Utc>> =
        sqlx::query_scalar("SELECT updated_at FROM items WHERE id = $1")
            .bind(open_item)
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(
        stamped,
        Some(Utc.with_ymd_and_hms(2020, 1, 1, 0, 0, 0).unwrap())
    );
    let audits: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_log WHERE event_type = 'deposit_batch_reclassified' AND entity_id = $1",
    )
    .bind(open_item)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(audits, 0);

    let before_loan = loan_snap(pool, loan_id).await;
    let before_returned = loan_snap(pool, returned_loan).await;
    let applied = execute(
        pool,
        &command(lender_id, &reference, site_id, selection.clone(), true),
    )
    .await
    .expect("apply");
    assert!(applied.applied);
    assert_eq!(applied.batch_action, BatchAction::Create);
    let batch_id = applied.batch_id.expect("created batch");
    let (arrived, due_on, notes, status, batch_site): (chrono::NaiveDate, chrono::NaiveDate, Option<String>, String, i64) =
        sqlx::query_as(
            "SELECT arrived_on, return_due_on, notes, status, site_id FROM deposit_batches WHERE id = $1",
        )
        .bind(batch_id)
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(
        arrived,
        chrono::NaiveDate::from_ymd_opt(2027, 3, 1).unwrap()
    );
    assert_eq!(due_on, chrono::NaiveDate::from_ymd_opt(2027, 9, 1).unwrap());
    assert_eq!(notes.as_deref(), Some("historical import"));
    assert_eq!(status, "active");
    assert_eq!(batch_site, site_id);
    assert_eq!(
        item_place(pool, open_item).await,
        (Some(batch_id), Some(site_id), None)
    );
    assert_eq!(
        item_place(pool, selected_peer).await,
        (Some(batch_id), Some(site_id), None)
    );

    let title_a: String = sqlx::query_scalar("SELECT title FROM biblios WHERE id = $1")
        .bind(biblio_a)
        .fetch_one(pool)
        .await
        .unwrap();
    let title_out: String = sqlx::query_scalar("SELECT title FROM biblios WHERE id = $1")
        .bind(outside)
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(title_a, "Selected A");
    assert_eq!(title_out, "Already owned");

    let after_loan = loan_snap(pool, loan_id).await;
    assert_eq!(after_loan.expiry_at, before_loan.expiry_at);
    assert_eq!(after_loan.returned_at, before_loan.returned_at);
    assert_eq!(after_loan.nb_renews, before_loan.nb_renews);
    assert_eq!(after_loan.nb_renews, 2);
    let after_returned = loan_snap(pool, returned_loan).await;
    assert!(after_returned.returned_at.is_some());
    assert_eq!(after_returned.nb_renews, before_returned.nb_renews);
    let hold_status: String = sqlx::query_scalar("SELECT status FROM holds WHERE id = $1")
        .bind(hold_id)
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(hold_status, "pending");
    let fine_amount: i64 = sqlx::query_scalar("SELECT amount FROM fines WHERE id = $1")
        .bind(fine_id)
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(fine_amount, 150);

    let (user_id_audit, entity_type, payload): (Option<i64>, String, serde_json::Value) =
        sqlx::query_as(
            "SELECT user_id, entity_type, payload
             FROM audit_log
             WHERE event_type = 'deposit_batch_reclassified' AND entity_id = $1",
        )
        .bind(open_item)
        .fetch_one(pool)
        .await
        .expect("audit");
    assert!(user_id_audit.is_none());
    assert_eq!(entity_type, "item");
    assert_eq!(payload["actorType"], "script");
    assert_eq!(payload["actorName"], "reclassify-deposits");
    assert_eq!(payload["lenderId"], lender_id);
    assert_eq!(payload["batchId"], batch_id);
    assert_eq!(payload["oldSourceId"], old_source);
    assert_eq!(payload["newSourceId"], site_id);

    let updated_at: DateTime<Utc> =
        sqlx::query_scalar("SELECT updated_at FROM items WHERE id = $1")
            .bind(open_item)
            .fetch_one(pool)
            .await
            .unwrap();
    // The copies now sit on the target site, so a second --from-source pass does not see them.
    // Selecting the same barcodes again is the idempotent rerun.
    let again = execute(
        pool,
        &command(
            lender_id,
            &reference,
            site_id,
            Selection::Barcodes(vec![open_barcode, quiet_barcode, peer_barcode]),
            true,
        ),
    )
    .await
    .expect("rerun");
    assert_eq!(again.batch_action, BatchAction::Reuse);
    assert_eq!(again.batch_id, Some(batch_id));
    assert_eq!(again.summary.to_reclassify, 0);
    assert_eq!(again.summary.already_reclassified, 3);
    assert_eq!(again.summary.refused, 0);
    assert!(again.items.iter().all(|line| {
        line.reason == Some(LineReason::AlreadyReclassified)
            && line.disposition == LineDisposition::AlreadyReclassified
    }));
    let audit_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_log
         WHERE event_type = 'deposit_batch_reclassified' AND entity_id = ANY($1)",
    )
    .bind(vec![open_item, quiet_item, selected_peer])
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(audit_count, 3);
    let updated_again: DateTime<Utc> =
        sqlx::query_scalar("SELECT updated_at FROM items WHERE id = $1")
            .bind(open_item)
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(updated_again, updated_at);
    let batch_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM deposit_batches WHERE lender_id = $1 AND reference = $2",
    )
    .bind(lender_id)
    .bind(&reference)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(batch_count, 1);
}

#[tokio::test]
async fn refusals_are_reported_and_left_unchanged() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let pool = app.state.services.repository.pool();
    let token = fixtures::unique_token();
    let site_id = source(pool, &format!("site-{token}")).await;
    let old_source = source(pool, &format!("old-{token}")).await;
    let other_site = source(pool, &format!("other-{token}")).await;
    let lender_id = lender(pool, &format!("Lender {token}")).await;
    let user_id = reader(pool, &format!("reader-{token}")).await;
    let biblio_id = biblio(pool, "Copy", None).await;

    let reference = format!("BATCH-{token}");
    let batch_id: i64 = sqlx::query_scalar(
        "INSERT INTO deposit_batches (lender_id, reference, site_id, arrived_on, return_due_on)
         VALUES ($1, $2, $3, '2026-03-01', '2026-09-01') RETURNING id",
    )
    .bind(lender_id)
    .bind(&reference)
    .bind(site_id)
    .fetch_one(pool)
    .await
    .unwrap();
    let other_batch: i64 = sqlx::query_scalar(
        "INSERT INTO deposit_batches (lender_id, reference, site_id, arrived_on, return_due_on)
         VALUES ($1, $2, $3, '2026-01-01', '2026-06-01') RETURNING id",
    )
    .bind(lender_id)
    .bind(format!("OTHER-{token}"))
    .bind(site_id)
    .fetch_one(pool)
    .await
    .unwrap();

    let archived = item(pool, biblio_id, old_source, &format!("ARCH-{token}")).await;
    sqlx::query("UPDATE items SET archived_at = NOW() WHERE id = $1")
        .bind(archived)
        .execute(pool)
        .await
        .unwrap();
    let withdrawn = item(pool, biblio_id, old_source, &format!("WD-{token}")).await;
    sqlx::query("UPDATE items SET weeding_status = 'withdrawn' WHERE id = $1")
        .bind(withdrawn)
        .execute(pool)
        .await
        .unwrap();
    let other = item(pool, biblio_id, old_source, &format!("OTH-{token}")).await;
    sqlx::query("UPDATE items SET deposit_batch_id = $2 WHERE id = $1")
        .bind(other)
        .bind(other_batch)
        .execute(pool)
        .await
        .unwrap();
    let transit_item = item(pool, biblio_id, old_source, &format!("TR-{token}")).await;
    let transit_id = next_id();
    sqlx::query(
        "INSERT INTO item_transits (id, item_id, from_source_id, to_source_id, status)
         VALUES ($1, $2, $3, $4, 'in_transit')",
    )
    .bind(transit_id)
    .bind(transit_item)
    .bind(old_source)
    .bind(other_site)
    .execute(pool)
    .await
    .unwrap();
    let transit_loan: i64 = sqlx::query_scalar(
        "INSERT INTO loans (user_id, item_id, expiry_at, nb_renews) VALUES ($1, $2, $3, 4) RETURNING id",
    )
    .bind(user_id)
    .bind(transit_item)
    .bind(due())
    .fetch_one(pool)
    .await
    .unwrap();
    let received = item(pool, biblio_id, old_source, &format!("RCV-{token}")).await;
    sqlx::query(
        "INSERT INTO item_transits (id, item_id, from_source_id, to_source_id, status)
         VALUES ($1, $2, $3, $4, 'received')",
    )
    .bind(next_id())
    .bind(received)
    .bind(old_source)
    .bind(other_site)
    .execute(pool)
    .await
    .unwrap();
    let candidate = item(pool, biblio_id, old_source, &format!("CAND-{token}")).await;
    sqlx::query("UPDATE items SET weeding_status = 'candidate' WHERE id = $1")
        .bind(candidate)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO holds (id, user_id, biblio_id, item_id, status, position)
         VALUES ($1, $2, $3, $4, 'ready', 1)",
    )
    .bind(next_id())
    .bind(user_id)
    .bind(biblio_id)
    .bind(candidate)
    .execute(pool)
    .await
    .unwrap();
    let already = item(pool, biblio_id, site_id, &format!("DONE-{token}")).await;
    sqlx::query("UPDATE items SET deposit_batch_id = $2 WHERE id = $1")
        .bind(already)
        .bind(batch_id)
        .execute(pool)
        .await
        .unwrap();
    let requested = item(pool, biblio_id, old_source, &format!("REQ-{token}")).await;
    sqlx::query(
        "INSERT INTO item_transits (id, item_id, from_source_id, to_source_id, status)
         VALUES ($1, $2, $3, $4, 'requested')",
    )
    .bind(next_id())
    .bind(requested)
    .bind(old_source)
    .bind(site_id)
    .execute(pool)
    .await
    .unwrap();

    let unknown = format!("MISS-{token}");
    let codes = vec![
        format!("ARCH-{token}"),
        format!("WD-{token}"),
        format!("OTH-{token}"),
        format!("TR-{token}"),
        format!("RCV-{token}"),
        format!("CAND-{token}"),
        format!("DONE-{token}"),
        format!("REQ-{token}"),
        unknown.clone(),
    ];
    let report = execute(
        pool,
        &command(
            lender_id,
            &reference,
            site_id,
            Selection::Barcodes(codes),
            true,
        ),
    )
    .await
    .expect("apply refusals");
    assert_eq!(report.batch_action, BatchAction::Reuse);
    assert_eq!(report.batch_id, Some(batch_id));
    assert_eq!(report.summary.to_reclassify, 2);
    assert_eq!(report.summary.with_holds, 1);
    assert_eq!(report.summary.on_loan, 0);
    assert_eq!(report.summary.refused, 6);
    assert_eq!(report.summary.already_reclassified, 1);

    let reason = |barcode: &str| {
        report
            .items
            .iter()
            .find(|line| line.barcode.as_deref() == Some(barcode))
            .map(|line| line.reason)
            .expect(barcode)
    };
    assert_eq!(
        reason(&format!("ARCH-{token}")),
        Some(LineReason::ItemArchived)
    );
    assert_eq!(
        reason(&format!("WD-{token}")),
        Some(LineReason::ItemArchived)
    );
    assert_eq!(
        reason(&format!("OTH-{token}")),
        Some(LineReason::ItemInOtherBatch)
    );
    assert_eq!(
        reason(&format!("TR-{token}")),
        Some(LineReason::ItemInTransit)
    );
    assert_eq!(
        reason(&format!("REQ-{token}")),
        Some(LineReason::ItemInTransit)
    );
    assert_eq!(reason(&unknown), Some(LineReason::BarcodeUnknown));
    assert_eq!(
        reason(&format!("DONE-{token}")),
        Some(LineReason::AlreadyReclassified)
    );
    assert_eq!(reason(&format!("RCV-{token}")), None);
    assert_eq!(reason(&format!("CAND-{token}")), None);

    assert_eq!(item_place(pool, archived).await.1, Some(old_source));
    assert_eq!(item_place(pool, withdrawn).await.0, None);
    assert_eq!(item_place(pool, other).await.0, Some(other_batch));
    assert_eq!(item_place(pool, transit_item).await.1, Some(old_source));
    assert_eq!(item_place(pool, requested).await.0, None);
    assert_eq!(
        item_place(pool, received).await,
        (Some(batch_id), Some(site_id), None)
    );
    assert_eq!(
        item_place(pool, candidate).await,
        (Some(batch_id), Some(site_id), None)
    );
    let transit_status: String =
        sqlx::query_scalar("SELECT status FROM item_transits WHERE id = $1")
            .bind(transit_id)
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(transit_status, "in_transit");
    let expiry: Option<DateTime<Utc>> =
        sqlx::query_scalar("SELECT expiry_at FROM loans WHERE id = $1")
            .bind(transit_loan)
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(expiry, Some(due()));
    let hold_status: String = sqlx::query_scalar("SELECT status FROM holds WHERE item_id = $1")
        .bind(candidate)
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(hold_status, "ready");
    let already_audits: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_log
         WHERE event_type = 'deposit_batch_reclassified' AND entity_id = $1",
    )
    .bind(already)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(already_audits, 0);
}

#[tokio::test]
async fn from_source_skips_archived_rows_and_refuses_withdrawn() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let pool = app.state.services.repository.pool();
    let token = fixtures::unique_token();
    let site_id = source(pool, &format!("site-{token}")).await;
    let old_source = source(pool, &format!("old-{token}")).await;
    let lender_id = lender(pool, &format!("Lender {token}")).await;
    let biblio_id = biblio(pool, "Shelf", None).await;
    let live = item(pool, biblio_id, old_source, &format!("LIVE-{token}")).await;
    let archived = item(pool, biblio_id, old_source, &format!("GONE-{token}")).await;
    sqlx::query("UPDATE items SET archived_at = NOW() WHERE id = $1")
        .bind(archived)
        .execute(pool)
        .await
        .unwrap();
    let withdrawn = item(pool, biblio_id, old_source, &format!("WEED-{token}")).await;
    sqlx::query("UPDATE items SET weeding_status = 'withdrawn' WHERE id = $1")
        .bind(withdrawn)
        .execute(pool)
        .await
        .unwrap();
    let elsewhere = item(pool, biblio_id, site_id, &format!("ELSE-{token}")).await;

    let report = execute(
        pool,
        &command(
            lender_id,
            &format!("SRC-{token}"),
            site_id,
            Selection::FromSource(old_source),
            true,
        ),
    )
    .await
    .unwrap();
    let ids: Vec<i64> = report
        .items
        .iter()
        .filter_map(|line| line.item_id)
        .collect();
    assert!(ids.contains(&live));
    assert!(ids.contains(&withdrawn));
    assert!(!ids.contains(&archived));
    assert!(!ids.contains(&elsewhere));
    assert_eq!(item_place(pool, live).await.0, report.batch_id);
    assert_eq!(item_place(pool, withdrawn).await.0, None);
    assert_eq!(item_place(pool, archived).await.1, Some(old_source));
}

#[tokio::test]
async fn batch_reuse_creation_and_invalid_arguments_write_nothing() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let pool = app.state.services.repository.pool();
    let token = fixtures::unique_token();
    let site_id = source(pool, &format!("site-{token}")).await;
    let old_source = source(pool, &format!("old-{token}")).await;
    let lender_id = lender(pool, &format!("Lender {token}")).await;
    let other_lender = lender(pool, &format!("Other {token}")).await;
    let biblio_id = biblio(pool, "Reuse", None).await;
    let barcode = format!("REUSE-{token}");
    let item_id = item(pool, biblio_id, old_source, &barcode).await;
    let reference = format!("REUSE-{token}");
    let batch_id: i64 = sqlx::query_scalar(
        "INSERT INTO deposit_batches
            (lender_id, reference, site_id, arrived_on, return_due_on, notes)
         VALUES ($1, $2, $3, '2024-01-15', '2024-08-01', 'kept')
         RETURNING id",
    )
    .bind(lender_id)
    .bind(&reference)
    .bind(site_id)
    .fetch_one(pool)
    .await
    .unwrap();

    let mut reuse = command(
        lender_id,
        &reference,
        site_id,
        Selection::Barcodes(vec![barcode.clone()]),
        true,
    );
    reuse.return_due_on = None;
    reuse.received_on = Some(chrono::NaiveDate::from_ymd_opt(2030, 1, 1).unwrap());
    reuse.notes = Some("ignored on reuse".into());
    let report = execute(pool, &reuse).await.expect("reuse");
    assert_eq!(report.batch_action, BatchAction::Reuse);
    assert_eq!(report.batch_id, Some(batch_id));
    assert_eq!(
        report.arrived_on,
        chrono::NaiveDate::from_ymd_opt(2024, 1, 15).unwrap()
    );
    assert_eq!(report.notes.as_deref(), Some("kept"));
    assert_eq!(
        item_place(pool, item_id).await,
        (Some(batch_id), Some(site_id), None)
    );
    let stored_notes: String =
        sqlx::query_scalar("SELECT notes FROM deposit_batches WHERE id = $1")
            .bind(batch_id)
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(stored_notes, "kept");

    sqlx::query("UPDATE items SET source_id = $2 WHERE id = $1")
        .bind(item_id)
        .bind(old_source)
        .execute(pool)
        .await
        .unwrap();
    let moved = execute(pool, &reuse).await.expect("move source");
    assert_eq!(moved.summary.to_reclassify, 1);
    assert_eq!(moved.summary.already_reclassified, 0);
    assert_eq!(item_place(pool, item_id).await.1, Some(site_id));
    let audits: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_log
         WHERE event_type = 'deposit_batch_reclassified' AND entity_id = $1",
    )
    .bind(item_id)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(audits, 2);

    let returning = format!("RET-{token}");
    let returning_id: i64 = sqlx::query_scalar(
        "INSERT INTO deposit_batches (lender_id, reference, site_id, arrived_on, return_due_on, status)
         VALUES ($1, $2, $3, '2026-01-01', '2026-06-01', 'returning') RETURNING id",
    )
    .bind(lender_id)
    .bind(&returning)
    .bind(site_id)
    .fetch_one(pool)
    .await
    .unwrap();
    let fresh = item(pool, biblio_id, old_source, &format!("FRESH-{token}")).await;
    let err = execute(
        pool,
        &command(
            lender_id,
            &returning,
            site_id,
            Selection::Barcodes(vec![format!("FRESH-{token}")]),
            true,
        ),
    )
    .await
    .expect_err("inactive batch");
    assert!(matches!(err, ReclassifyFailure::Invalid(_)));
    assert_eq!(err.exit_code(), 2);
    assert!(err.to_string().contains("returning"));
    assert_eq!(item_place(pool, fresh).await.0, None);
    let still: String = sqlx::query_scalar("SELECT status FROM deposit_batches WHERE id = $1")
        .bind(returning_id)
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(still, "returning");

    let archived_lender = lender(pool, &format!("Archived {token}")).await;
    sqlx::query("UPDATE lenders SET archived_at = NOW() WHERE id = $1")
        .bind(archived_lender)
        .execute(pool)
        .await
        .unwrap();
    let err = execute(
        pool,
        &command(
            archived_lender,
            &format!("NEW-{token}"),
            site_id,
            Selection::FromSource(old_source),
            true,
        ),
    )
    .await
    .expect_err("archived lender");
    assert!(err.to_string().contains("archived"));
    let err = execute(
        pool,
        &command(
            9_000_000_000_000,
            &format!("NEW-{token}"),
            site_id,
            Selection::FromSource(old_source),
            false,
        ),
    )
    .await
    .expect_err("unknown lender");
    assert!(err.to_string().contains("does not exist"));

    let mut missing_date = command(
        lender_id,
        &format!("NODATE-{token}"),
        site_id,
        Selection::Barcodes(vec![format!("FRESH-{token}")]),
        true,
    );
    missing_date.return_due_on = None;
    let err = execute(pool, &missing_date)
        .await
        .expect_err("missing date");
    assert!(err.to_string().contains("return-due-on"));
    let created: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM deposit_batches WHERE lender_id = $1 AND reference = $2",
    )
    .bind(lender_id)
    .bind(format!("NODATE-{token}"))
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(created, 0);
    assert_eq!(item_place(pool, fresh).await.1, Some(old_source));

    let err = execute(
        pool,
        &command(
            lender_id,
            &format!("BADSITE-{token}"),
            9_000_000_000_001,
            Selection::Barcodes(vec![format!("FRESH-{token}")]),
            true,
        ),
    )
    .await
    .expect_err("unknown site");
    assert!(err.to_string().contains("site"));

    let err = execute(
        pool,
        &command(
            lender_id,
            &format!("BADSRC-{token}"),
            site_id,
            Selection::FromSource(9_000_000_000_002),
            false,
        ),
    )
    .await
    .expect_err("unknown source");
    assert!(err.to_string().contains("does not exist"));

    let second = execute(
        pool,
        &command(
            other_lender,
            &reference,
            site_id,
            Selection::Barcodes(vec![format!("FRESH-{token}")]),
            true,
        ),
    )
    .await
    .expect("same reference, other lender");
    assert_eq!(second.batch_action, BatchAction::Create);
    assert_ne!(second.batch_id, Some(batch_id));
    assert_eq!(item_place(pool, fresh).await.0, second.batch_id);
}

#[tokio::test]
async fn existing_batch_site_mismatch_writes_nothing() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let pool = app.state.services.repository.pool();
    let token = fixtures::unique_token();
    let batch_site = source(pool, &format!("batch-site-{token}")).await;
    let other_site = source(pool, &format!("other-site-{token}")).await;
    let old_source = source(pool, &format!("old-{token}")).await;
    let lender_id = lender(pool, &format!("Lender {token}")).await;
    let biblio_id = biblio(pool, "Mismatch", None).await;
    let barcode = format!("MIS-{token}");
    let item_id = item(pool, biblio_id, old_source, &barcode).await;
    let reference = format!("MIS-{token}");
    let batch_id: i64 = sqlx::query_scalar(
        "INSERT INTO deposit_batches
            (lender_id, reference, site_id, arrived_on, return_due_on)
         VALUES ($1, $2, $3, '2026-01-01', '2027-06-01')
         RETURNING id",
    )
    .bind(lender_id)
    .bind(&reference)
    .bind(batch_site)
    .fetch_one(pool)
    .await
    .unwrap();

    for apply in [false, true] {
        let err = execute(
            pool,
            &command(
                lender_id,
                &reference,
                other_site,
                Selection::Barcodes(vec![barcode.clone()]),
                apply,
            ),
        )
        .await
        .expect_err("site mismatch");
        assert!(matches!(err, ReclassifyFailure::Invalid(_)));
        assert_eq!(err.exit_code(), 2);
        assert_eq!(err.code(), Some("deposit_site_mismatch"));
        assert!(err.to_string().contains("deposit_site_mismatch"));
    }

    assert_eq!(
        item_place(pool, item_id).await,
        (None, Some(old_source), None)
    );
    let stored_site: i64 = sqlx::query_scalar("SELECT site_id FROM deposit_batches WHERE id = $1")
        .bind(batch_id)
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(stored_site, batch_site);
    let audits: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_log
         WHERE event_type = 'deposit_batch_reclassified' AND entity_id = $1",
    )
    .bind(item_id)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(audits, 0);
}

fn warning_codes(report: &elidune_server::deposit_reclassify::Report) -> Vec<&str> {
    report
        .warnings
        .iter()
        .map(|warning| warning.code.as_str())
        .collect()
}

#[tokio::test]
async fn past_or_today_return_due_warns_and_still_applies() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let pool = app.state.services.repository.pool();
    let token = fixtures::unique_token();
    let site_id = source(pool, &format!("site-{token}")).await;
    let old_source = source(pool, &format!("old-{token}")).await;
    let lender_id = lender(pool, &format!("Lender {token}")).await;
    let biblio_id = biblio(pool, "Due", None).await;
    let today = Utc::now().date_naive();
    let past = chrono::NaiveDate::from_ymd_opt(2020, 6, 1).unwrap();
    let past_arrived = chrono::NaiveDate::from_ymd_opt(2020, 1, 1).unwrap();
    let future = chrono::NaiveDate::from_ymd_opt(2027, 9, 1).unwrap();

    let today_barcode = format!("TODAY-{token}");
    let today_item = item(pool, biblio_id, old_source, &today_barcode).await;
    let mut today_cmd = command(
        lender_id,
        &format!("TODAY-{token}"),
        site_id,
        Selection::Barcodes(vec![today_barcode]),
        true,
    );
    today_cmd.received_on = Some(today);
    today_cmd.return_due_on = Some(today);
    let today_report = execute(pool, &today_cmd)
        .await
        .expect("today still applies");
    assert_eq!(warning_codes(&today_report), vec!["return_due_on_past"]);
    assert!(today_report.warnings[0]
        .message
        .contains("deposit_return_due"));
    assert!(today_report.warnings[0].message.contains("holds"));
    assert!(today_report.warnings[0]
        .message
        .contains(&today.to_string()));
    assert_eq!(today_report.summary.to_reclassify, 1);
    assert_eq!(item_place(pool, today_item).await.1, Some(site_id));
    let text = elidune_server::deposit_reclassify::render_report(
        &today_report,
        elidune_server::deposit_reclassify::ReportFormat::Text,
    );
    assert!(text.starts_with("WARNING\tcode=return_due_on_past\t"));
    let json = elidune_server::deposit_reclassify::render_report(
        &today_report,
        elidune_server::deposit_reclassify::ReportFormat::Json,
    );
    assert!(json.contains("\"warnings\""));
    assert!(json.contains("\"code\": \"return_due_on_past\""));

    let past_barcode = format!("PAST-{token}");
    let past_item = item(pool, biblio_id, old_source, &past_barcode).await;
    let mut past_cmd = command(
        lender_id,
        &format!("PAST-{token}"),
        site_id,
        Selection::Barcodes(vec![past_barcode]),
        false,
    );
    past_cmd.received_on = Some(past_arrived);
    past_cmd.return_due_on = Some(past);
    let past_report = execute(pool, &past_cmd).await.expect("past dry run");
    assert_eq!(warning_codes(&past_report), vec!["return_due_on_past"]);
    assert!(past_report.warnings[0].message.contains("2020-06-01"));
    assert_eq!(item_place(pool, past_item).await.1, Some(old_source));
    let batches: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM deposit_batches WHERE lender_id = $1 AND reference = $2",
    )
    .bind(lender_id)
    .bind(format!("PAST-{token}"))
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(batches, 0);

    let reuse_barcode = format!("REUSE-DUE-{token}");
    let reuse_item = item(pool, biblio_id, old_source, &reuse_barcode).await;
    let reuse_ref = format!("REUSE-DUE-{token}");
    sqlx::query(
        "INSERT INTO deposit_batches
            (lender_id, reference, site_id, arrived_on, return_due_on, notes)
         VALUES ($1, $2, $3, '2024-01-15', '2024-08-01', 'kept')",
    )
    .bind(lender_id)
    .bind(&reuse_ref)
    .bind(site_id)
    .execute(pool)
    .await
    .unwrap();
    let mut reuse = command(
        lender_id,
        &reuse_ref,
        site_id,
        Selection::Barcodes(vec![reuse_barcode]),
        true,
    );
    reuse.return_due_on = Some(future);
    reuse.received_on = Some(chrono::NaiveDate::from_ymd_opt(2027, 3, 1).unwrap());
    let reuse_report = execute(pool, &reuse).await.expect("reuse stored past date");
    assert_eq!(warning_codes(&reuse_report), vec!["return_due_on_past"]);
    assert!(reuse_report.warnings[0].message.contains("2024-08-01"));
    assert!(!reuse_report.warnings[0].message.contains("2027-09-01"));
    assert_eq!(
        reuse_report.return_due_on,
        chrono::NaiveDate::from_ymd_opt(2024, 8, 1).unwrap()
    );
    assert_eq!(item_place(pool, reuse_item).await.1, Some(site_id));

    let future_barcode = format!("FUTURE-{token}");
    let future_item = item(pool, biblio_id, old_source, &future_barcode).await;
    let future_report = execute(
        pool,
        &command(
            lender_id,
            &format!("FUTURE-{token}"),
            site_id,
            Selection::Barcodes(vec![future_barcode]),
            true,
        ),
    )
    .await
    .expect("future date");
    assert!(future_report.warnings.is_empty());
    assert_eq!(item_place(pool, future_item).await.1, Some(site_id));
}

async fn deposit_batch(pool: &PgPool, lender_id: i64, reference: &str, site_id: i64) -> i64 {
    sqlx::query_scalar(
        "INSERT INTO deposit_batches
            (lender_id, reference, site_id, arrived_on, return_due_on)
         VALUES ($1, $2, $3, '2027-03-01', '2027-09-01')
         RETURNING id",
    )
    .bind(lender_id)
    .bind(reference)
    .bind(site_id)
    .fetch_one(pool)
    .await
    .expect("batch")
}

#[tokio::test]
async fn from_source_equal_to_site_is_invalid_and_mixed_copies_warn() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let pool = app.state.services.repository.pool();
    let token = fixtures::unique_token();
    let site_id = source(pool, &format!("site-{token}")).await;
    let old_source = source(pool, &format!("old-{token}")).await;
    let other_site = source(pool, &format!("desk-{token}")).await;
    let lender_id = lender(pool, &format!("Lender {token}")).await;
    let user_id = reader(pool, &format!("reader-{token}")).await;
    let biblio_id = biblio(pool, "Mixed", None).await;

    let same = item(pool, biblio_id, site_id, &format!("SAME-{token}")).await;
    for apply in [false, true] {
        let err = execute(
            pool,
            &command(
                lender_id,
                &format!("SAME-{token}-{apply}"),
                site_id,
                Selection::FromSource(site_id),
                apply,
            ),
        )
        .await
        .expect_err("from-source equals site");
        assert!(matches!(err, ReclassifyFailure::Invalid(_)));
        assert_eq!(err.exit_code(), 2);
        assert!(err.to_string().contains("--from-source"));
    }
    assert_eq!(item_place(pool, same).await, (None, Some(site_id), None));
    let created: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM deposit_batches WHERE reference LIKE $1")
            .bind(format!("SAME-{token}-%"))
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(created, 0);

    let batch_a = deposit_batch(pool, lender_id, &format!("A-{token}"), site_id).await;
    let batch_b = deposit_batch(pool, lender_id, &format!("B-{token}"), site_id).await;
    let in_a = item(pool, biblio_id, old_source, &format!("A-{token}")).await;
    let in_b = item(pool, biblio_id, old_source, &format!("B-{token}")).await;
    let loan_elsewhere = item(pool, biblio_id, old_source, &format!("LOAN-{token}")).await;
    let loan_here = item(pool, biblio_id, old_source, &format!("HERE-{token}")).await;
    let loan_unknown = item(pool, biblio_id, old_source, &format!("UNK-{token}")).await;
    let archived = item(pool, biblio_id, old_source, &format!("ARCH-{token}")).await;
    let plain = item(pool, biblio_id, old_source, &format!("PLAIN-{token}")).await;
    sqlx::query("UPDATE items SET deposit_batch_id = $2 WHERE id = $1")
        .bind(in_a)
        .bind(batch_a)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("UPDATE items SET deposit_batch_id = $2 WHERE id = $1")
        .bind(in_b)
        .bind(batch_b)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("UPDATE items SET archived_at = NOW(), deposit_batch_id = $2 WHERE id = $1")
        .bind(archived)
        .bind(batch_a)
        .execute(pool)
        .await
        .unwrap();
    for (item_id, loan_site) in [
        (loan_elsewhere, Some(other_site)),
        (loan_here, Some(site_id)),
        (loan_unknown, None),
    ] {
        sqlx::query(
            "INSERT INTO loans (user_id, item_id, expiry_at, nb_renews, site_id)
             VALUES ($1, $2, $3, 0, $4)",
        )
        .bind(user_id)
        .bind(item_id)
        .bind(due())
        .bind(loan_site)
        .execute(pool)
        .await
        .unwrap();
    }

    let mixed = execute(
        pool,
        &command(
            lender_id,
            &format!("MIX-{token}"),
            site_id,
            Selection::FromSource(old_source),
            false,
        ),
    )
    .await
    .expect("mixed source warns");
    let warning = mixed
        .warnings
        .iter()
        .find(|warning| warning.code == "from_source_mixed")
        .expect("mixed warning");
    assert_eq!(warning.count, Some(3));
    assert!(warning.message.contains("3 copies"));
    assert_eq!(item_place(pool, plain).await.1, Some(old_source));

    let loan_source = source(pool, &format!("loan-src-{token}")).await;
    let only_batched = item(pool, biblio_id, loan_source, &format!("ONLYB-{token}")).await;
    let only_loan_item = item(pool, biblio_id, loan_source, &format!("ONLYL-{token}")).await;
    let only_batch = deposit_batch(pool, lender_id, &format!("ONLY-{token}"), site_id).await;
    sqlx::query("UPDATE items SET deposit_batch_id = $2 WHERE id = $1")
        .bind(only_batched)
        .bind(only_batch)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO loans (user_id, item_id, expiry_at, nb_renews, site_id)
         VALUES ($1, $2, $3, 0, $4)",
    )
    .bind(user_id)
    .bind(only_loan_item)
    .bind(due())
    .bind(other_site)
    .execute(pool)
    .await
    .unwrap();
    let only_loan = execute(
        pool,
        &command(
            lender_id,
            &format!("ONE-{token}"),
            site_id,
            Selection::FromSource(loan_source),
            false,
        ),
    )
    .await
    .expect("one batch plus a foreign loan");
    let only_warning = only_loan
        .warnings
        .iter()
        .find(|warning| warning.code == "from_source_mixed")
        .expect("loan warning");
    assert_eq!(only_warning.count, Some(1));

    let quiet_source = source(pool, &format!("quiet-{token}")).await;
    let quiet_item = item(pool, biblio_id, quiet_source, &format!("QUIET-{token}")).await;
    let quiet_batch = deposit_batch(pool, lender_id, &format!("Q-{token}"), site_id).await;
    sqlx::query("UPDATE items SET deposit_batch_id = $2 WHERE id = $1")
        .bind(quiet_item)
        .bind(quiet_batch)
        .execute(pool)
        .await
        .unwrap();
    let quiet = execute(
        pool,
        &command(
            lender_id,
            &format!("QUIET-{token}"),
            site_id,
            Selection::FromSource(quiet_source),
            false,
        ),
    )
    .await
    .expect("one batch is not mixed");
    assert!(warning_codes(&quiet)
        .iter()
        .all(|code| *code != "from_source_mixed"));
    assert_eq!(item_place(pool, quiet_item).await.0, Some(quiet_batch));

    let by_barcode = execute(
        pool,
        &command(
            lender_id,
            &format!("BCODE-{token}"),
            site_id,
            Selection::Barcodes(vec![format!("PLAIN-{token}")]),
            false,
        ),
    )
    .await
    .expect("barcode mode");
    assert!(warning_codes(&by_barcode)
        .iter()
        .all(|code| *code != "from_source_mixed"));
}
