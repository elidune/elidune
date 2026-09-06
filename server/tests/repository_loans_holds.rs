//! Repository-level tests: loan/hold atomicity and checkout race safety.

mod common;

use axum::http::StatusCode;
use common::fixtures;
use common::TestApp;
use elidune_server::error::AppError;
use elidune_server::models::hold::{CreateHold, HoldStatus};
use elidune_server::models::loan::CreateLoan;
use elidune_server::repository::Repository;
use serde_json::json;

async fn seed_borrowable_item(app: &TestApp, admin_token: &str, barcode: &str, title: &str) -> i64 {
    let barcode = format!("{barcode}-{}", fixtures::unique_suffix());
    let biblio_payload = json!({
        "title": title,
        "mediaType": "printedText",
        "lang": "french",
        "items": [{ "barcode": barcode, "borrowable": true }]
    });

    let (status, body) = app
        .post_json("/api/v1/biblios", &biblio_payload, Some(admin_token))
        .await;
    assert_eq!(status, StatusCode::CREATED, "create biblio: {body}");
    fixtures::json_id(&body["biblio"]["items"][0]["id"])
}

fn create_loan(user_id: i64, item_id: i64, force: bool) -> CreateLoan {
    CreateLoan {
        user_id,
        item_id: Some(item_id),
        item_identification: None,
        force,
    }
}

fn assert_already_borrowed(err: AppError) {
    match err {
        AppError::BusinessRule(msg) => assert_eq!(msg, "Item is already borrowed"),
        other => panic!("expected already borrowed business rule, got {other:?}"),
    }
}

fn assert_duplicate_hold(err: AppError) {
    match err {
        AppError::Conflict(msg) => {
            assert_eq!(msg, "User already has an active hold for this item");
        }
        other => panic!("expected duplicate hold conflict, got {other:?}"),
    }
}

fn create_hold(user_id: i64, item_id: i64) -> CreateHold {
    CreateHold::for_item(user_id, item_id)
}

#[tokio::test]
async fn loan_return_atomically_advances_next_hold() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };

    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_a_id, _) = fixtures::create_reader(&app, &admin_token, "repohold_a").await;
    let (reader_b_id, _) = fixtures::create_reader(&app, &admin_token, "repohold_b").await;

    let biblio_payload = json!({
        "title": "Repo Hold Atomic Test",
        "mediaType": "printedText",
        "lang": "french",
        "items": [{ "barcode": format!("REPO-HOLD-{}", fixtures::unique_suffix()), "borrowable": true }]
    });

    let (status, body) = app
        .post_json("/api/v1/biblios", &biblio_payload, Some(&admin_token))
        .await;
    assert_eq!(status, StatusCode::CREATED, "create biblio: {body}");
    let item_id = fixtures::json_id(&body["biblio"]["items"][0]["id"]);

    let repo = app.state.services.repository.as_ref().clone();

    let hold_a = repo
        .holds_create(&CreateHold::for_item(reader_a_id, item_id))
        .await
        .expect("hold for reader A");
    let hold_b = repo
        .holds_create(&CreateHold::for_item(reader_b_id, item_id))
        .await
        .expect("hold for reader B");

    assert_eq!(hold_a.status, HoldStatus::Pending);
    assert_eq!(hold_b.status, HoldStatus::Pending);
    assert!(hold_a.position < hold_b.position);

    let checkout = repo
        .loans_create(&create_loan(reader_a_id, item_id, false))
        .await
        .expect("checkout to reader A");

    let return_outcome = repo
        .loans_return(checkout.loan_id)
        .await
        .expect("return loan");

    let readied = return_outcome
        .readied_hold
        .as_ref()
        .expect("next hold should become ready in same transaction");
    assert_eq!(readied.user_id, reader_b_id);
    assert_eq!(readied.id, hold_b.id);
    assert_eq!(readied.status, HoldStatus::Ready);
    assert!(readied.expires_at.is_some());
}

#[tokio::test]
async fn sequential_checkout_same_item_fails_with_already_borrowed() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };

    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_a_id, _) = fixtures::create_reader(&app, &admin_token, "seqloan_a").await;
    let (reader_b_id, _) = fixtures::create_reader(&app, &admin_token, "seqloan_b").await;
    let item_id =
        seed_borrowable_item(&app, &admin_token, "SEQ-LOAN-001", "Sequential Loan Test").await;
    let repo = app.state.services.repository.as_ref().clone();

    repo.loans_create(&create_loan(reader_a_id, item_id, false))
        .await
        .expect("first checkout");

    let err = repo
        .loans_create(&create_loan(reader_b_id, item_id, false))
        .await
        .expect_err("second checkout must fail");
    assert_already_borrowed(err);
    assert_eq!(repo.loans_count_active_for_item(item_id).await.unwrap(), 1);
}

#[tokio::test]
async fn concurrent_checkout_same_item_only_one_succeeds() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };

    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_a_id, _) = fixtures::create_reader(&app, &admin_token, "raceloan_a").await;
    let (reader_b_id, _) = fixtures::create_reader(&app, &admin_token, "raceloan_b").await;
    let item_id =
        seed_borrowable_item(&app, &admin_token, "RACE-LOAN-001", "Concurrent Loan Test").await;
    let repo = app.state.services.repository.as_ref().clone();

    let loan_a = create_loan(reader_a_id, item_id, false);
    let loan_b = create_loan(reader_b_id, item_id, false);
    let (r1, r2) = tokio::join!(repo.loans_create(&loan_a), repo.loans_create(&loan_b),);

    let oks = [&r1, &r2].iter().filter(|r| r.is_ok()).count();
    assert_eq!(
        oks, 1,
        "exactly one concurrent checkout should succeed: {r1:?} {r2:?}"
    );

    let err = if r1.is_err() {
        r1.err().unwrap()
    } else {
        r2.err().unwrap()
    };
    assert_already_borrowed(err);
    assert_eq!(repo.loans_count_active_for_item(item_id).await.unwrap(), 1);
}

#[tokio::test]
async fn unique_index_rejects_second_active_loan_on_item() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };

    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_a_id, _) = fixtures::create_reader(&app, &admin_token, "uniqloan_a").await;
    let (reader_b_id, _) = fixtures::create_reader(&app, &admin_token, "uniqloan_b").await;
    let item_id =
        seed_borrowable_item(&app, &admin_token, "UNIQ-LOAN-001", "Unique Index Loan").await;
    let repo = app.state.services.repository.as_ref().clone();
    let pool = repo.pool();

    sqlx::query(
        "INSERT INTO loans (user_id, item_id, date, expiry_at, nb_renews) VALUES ($1, $2, NOW(), NOW() + interval '21 days', 0)",
    )
    .bind(reader_a_id)
    .bind(item_id)
    .execute(pool)
    .await
    .expect("first raw insert");

    let err = sqlx::query(
        "INSERT INTO loans (user_id, item_id, date, expiry_at, nb_renews) VALUES ($1, $2, NOW(), NOW() + interval '21 days', 0)",
    )
    .bind(reader_b_id)
    .bind(item_id)
    .execute(pool)
    .await
    .expect_err("second active loan must violate unique index");

    let db = err.as_database_error().expect("database error");
    assert_eq!(db.code().as_deref(), Some("23505"));
    assert_eq!(db.constraint(), Some("idx_loans_one_active_per_item"));
}

#[tokio::test]
async fn force_checkout_replaces_existing_active_loan() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };

    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_a_id, _) = fixtures::create_reader(&app, &admin_token, "forceloan_a").await;
    let (reader_b_id, _) = fixtures::create_reader(&app, &admin_token, "forceloan_b").await;
    let item_id =
        seed_borrowable_item(&app, &admin_token, "FORCE-LOAN-001", "Force Loan Test").await;
    let repo = app.state.services.repository.as_ref().clone();

    let first = repo
        .loans_create(&create_loan(reader_a_id, item_id, false))
        .await
        .expect("first checkout");

    let second = repo
        .loans_create(&create_loan(reader_b_id, item_id, true))
        .await
        .expect("force checkout should replace the active loan");

    assert_ne!(first.loan_id, second.loan_id);
    assert_eq!(repo.loans_count_active_for_item(item_id).await.unwrap(), 1);

    let active_ids = repo.loans_get_active_ids_for_item(item_id).await.unwrap();
    assert_eq!(active_ids, vec![second.loan_id]);
}

#[tokio::test]
async fn concurrent_force_checkouts_leave_one_active_loan() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };

    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_a_id, _) = fixtures::create_reader(&app, &admin_token, "forcerace_a").await;
    let (reader_b_id, _) = fixtures::create_reader(&app, &admin_token, "forcerace_b").await;
    let item_id =
        seed_borrowable_item(&app, &admin_token, "FORCE-RACE-001", "Force Race Loan").await;
    let repo = app.state.services.repository.as_ref().clone();

    let loan_a = create_loan(reader_a_id, item_id, true);
    let loan_b = create_loan(reader_b_id, item_id, true);
    let (r1, r2) = tokio::join!(repo.loans_create(&loan_a), repo.loans_create(&loan_b),);

    assert!(r1.is_ok(), "force checkout A: {r1:?}");
    assert!(r2.is_ok(), "force checkout B: {r2:?}");
    assert_eq!(repo.loans_count_active_for_item(item_id).await.unwrap(), 1);
}

#[tokio::test]
async fn concurrent_checkout_respects_hold_queue() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };

    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_a_id, _) = fixtures::create_reader(&app, &admin_token, "holdrace_a").await;
    let (reader_b_id, _) = fixtures::create_reader(&app, &admin_token, "holdrace_b").await;
    let item_id = seed_borrowable_item(&app, &admin_token, "HOLD-RACE-001", "Hold Race Loan").await;
    let repo = app.state.services.repository.as_ref().clone();

    repo.holds_create(&CreateHold::for_item(reader_a_id, item_id))
        .await
        .expect("hold for reader A");

    let loan_a = create_loan(reader_a_id, item_id, false);
    let loan_b = create_loan(reader_b_id, item_id, false);
    let (r_a, r_b) = tokio::join!(repo.loans_create(&loan_a), repo.loans_create(&loan_b),);

    let a_ok = r_a.is_ok();
    let b_ok = r_b.is_ok();
    assert!(a_ok, "queued patron A must be able to checkout: {r_a:?}");
    assert!(!b_ok, "patron B must not steal the held copy: {r_b:?}");

    match r_b.err().unwrap() {
        AppError::BusinessRule(msg) => {
            assert!(
                msg.contains("active hold") || msg == "Item is already borrowed",
                "unexpected business rule: {msg}"
            );
        }
        other => panic!("expected business rule for B, got {other:?}"),
    }

    assert_eq!(repo.loans_count_active_for_item(item_id).await.unwrap(), 1);
    let active_ids = repo.loans_get_active_ids_for_item(item_id).await.unwrap();
    let a_loan_id = r_a.unwrap().loan_id;
    assert_eq!(active_ids, vec![a_loan_id]);
}

#[tokio::test]
async fn sequential_place_hold_same_user_item_conflicts() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };

    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_id, _) = fixtures::create_reader(&app, &admin_token, "seqhold_a").await;
    let item_id =
        seed_borrowable_item(&app, &admin_token, "SEQ-HOLD-001", "Sequential Hold Test").await;
    let holds = &app.state.services.holds;

    holds
        .place_hold(create_hold(reader_id, item_id), None, None)
        .await
        .expect("first hold");

    let err = holds
        .place_hold(create_hold(reader_id, item_id), None, None)
        .await
        .expect_err("second hold must fail");
    assert_duplicate_hold(err);
    assert_eq!(
        app.state
            .services
            .repository
            .holds_count_for_item(item_id)
            .await
            .unwrap(),
        1
    );
}

#[tokio::test]
async fn concurrent_place_hold_same_user_item_only_one_succeeds() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };

    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_id, _) = fixtures::create_reader(&app, &admin_token, "racehold_dup").await;
    let item_id = seed_borrowable_item(
        &app,
        &admin_token,
        "RACE-HOLD-DUP",
        "Concurrent Duplicate Hold",
    )
    .await;
    let holds = app.state.services.holds.clone();

    let (r1, r2) = tokio::join!(
        holds.place_hold(create_hold(reader_id, item_id), None, None),
        holds.place_hold(create_hold(reader_id, item_id), None, None),
    );

    let oks = [&r1, &r2].iter().filter(|r| r.is_ok()).count();
    assert_eq!(
        oks, 1,
        "exactly one concurrent place_hold should succeed: {r1:?} {r2:?}"
    );

    let err = if r1.is_err() {
        r1.err().unwrap()
    } else {
        r2.err().unwrap()
    };
    assert_duplicate_hold(err);
    assert_eq!(
        app.state
            .services
            .repository
            .holds_count_for_item(item_id)
            .await
            .unwrap(),
        1
    );
}

#[tokio::test]
async fn concurrent_place_hold_assigns_distinct_positions() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };

    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_a_id, _) = fixtures::create_reader(&app, &admin_token, "racehold_pos_a").await;
    let (reader_b_id, _) = fixtures::create_reader(&app, &admin_token, "racehold_pos_b").await;
    let item_id = seed_borrowable_item(
        &app,
        &admin_token,
        "RACE-HOLD-POS",
        "Concurrent Hold Positions",
    )
    .await;
    let repo = app.state.services.repository.as_ref().clone();
    let hold_req_a = create_hold(reader_a_id, item_id);
    let hold_req_b = create_hold(reader_b_id, item_id);

    let (r1, r2) = tokio::join!(
        repo.holds_create(&hold_req_a),
        repo.holds_create(&hold_req_b),
    );

    let hold_a = r1.expect("hold A must succeed");
    let hold_b = r2.expect("hold B must succeed");
    assert_ne!(
        hold_a.position, hold_b.position,
        "concurrent place_hold must not assign the same queue position"
    );

    let mut positions = [hold_a.position, hold_b.position];
    positions.sort_unstable();
    assert_eq!(positions, [1, 2]);
    assert_eq!(repo.holds_count_for_item(item_id).await.unwrap(), 2);
}

#[tokio::test]
async fn unique_index_rejects_second_active_hold_for_user_item() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };

    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_id, _) = fixtures::create_reader(&app, &admin_token, "uniqhold_a").await;
    let item_id =
        seed_borrowable_item(&app, &admin_token, "UNIQ-HOLD-001", "Unique Index Hold").await;
    let pool = app.state.services.repository.pool();
    let suffix = fixtures::unique_suffix() as i64;

    sqlx::query(
        "INSERT INTO holds (id, user_id, item_id, biblio_id, position)
         SELECT $1, $2, $3, biblio_id, 1 FROM items WHERE id = $3",
    )
    .bind(suffix)
    .bind(reader_id)
    .bind(item_id)
    .execute(pool)
    .await
    .expect("first raw insert");

    let err = sqlx::query(
        "INSERT INTO holds (id, user_id, item_id, biblio_id, position)
             SELECT $1, $2, $3, biblio_id, 2 FROM items WHERE id = $3",
    )
    .bind(suffix.wrapping_add(1))
    .bind(reader_id)
    .bind(item_id)
    .execute(pool)
    .await
    .expect_err("second active hold must violate unique index");

    let db = err.as_database_error().expect("database error");
    assert_eq!(db.code().as_deref(), Some("23505"));
    assert_eq!(db.constraint(), Some("idx_holds_one_active_per_user_item"));
}

#[tokio::test]
async fn cancelled_hold_does_not_block_new_active_hold() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };

    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_id, _) = fixtures::create_reader(&app, &admin_token, "rehold_a").await;
    let item_id =
        seed_borrowable_item(&app, &admin_token, "REHOLD-001", "Rehold After Cancel").await;
    let repo = app.state.services.repository.as_ref().clone();

    let first = repo
        .holds_create(&create_hold(reader_id, item_id))
        .await
        .expect("first hold");
    repo.holds_cancel(first.id).await.expect("cancel hold");

    let second = repo
        .holds_create(&create_hold(reader_id, item_id))
        .await
        .expect("new hold after cancel must succeed");
    assert_ne!(first.id, second.id);
    assert_eq!(second.status, HoldStatus::Pending);
    assert_eq!(repo.holds_count_for_item(item_id).await.unwrap(), 1);
}

async fn seed_ready_then_pending(
    app: &TestApp,
    admin_token: &str,
    barcode: &str,
    title: &str,
    login_a: &str,
    login_b: &str,
) -> (Repository, i64, i64) {
    let (reader_a_id, _) = fixtures::create_reader(app, admin_token, login_a).await;
    let (reader_b_id, _) = fixtures::create_reader(app, admin_token, login_b).await;
    let item_id = seed_borrowable_item(app, admin_token, barcode, title).await;
    let repo = app.state.services.repository.as_ref().clone();

    let hold_a = repo
        .holds_create(&create_hold(reader_a_id, item_id))
        .await
        .expect("hold for first patron");
    let hold_b = repo
        .holds_create(&create_hold(reader_b_id, item_id))
        .await
        .expect("hold for next patron");

    repo.holds_mark_ready(hold_a.id, 7)
        .await
        .expect("first hold becomes ready");

    (repo, hold_a.id, hold_b.id)
}

#[tokio::test]
async fn cancel_ready_hold_atomically_advances_next_pending() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };

    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (repo, ready_id, next_id) = seed_ready_then_pending(
        &app,
        &admin_token,
        "CANCEL-READY-001",
        "Cancel Ready Advances Queue",
        "cancelready_a",
        "cancelready_b",
    )
    .await;

    let cancelled = repo
        .holds_cancel(ready_id)
        .await
        .expect("cancel ready hold");
    assert_eq!(cancelled.status, HoldStatus::Cancelled);

    let next = repo
        .holds_get_by_id(next_id)
        .await
        .expect("next hold still exists");
    assert_eq!(
        next.status,
        HoldStatus::Ready,
        "cancelling a ready hold must promote the next pending patron"
    );
    assert!(next.notified_at.is_some());
    assert!(next.expires_at.is_some());
}

#[tokio::test]
async fn expire_ready_hold_atomically_advances_next_pending() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };

    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (repo, ready_id, next_id) = seed_ready_then_pending(
        &app,
        &admin_token,
        "EXPIRE-READY-001",
        "Expire Ready Advances Queue",
        "expireready_a",
        "expireready_b",
    )
    .await;

    sqlx::query("UPDATE holds SET expires_at = NOW() - interval '1 hour' WHERE id = $1")
        .bind(ready_id)
        .execute(repo.pool())
        .await
        .expect("make ready hold overdue");

    let expired_ids = repo
        .holds_expire_overdue()
        .await
        .expect("expire overdue ready holds");
    assert!(
        expired_ids.contains(&ready_id),
        "overdue ready hold must be expired: {expired_ids:?}"
    );

    let expired = repo
        .holds_get_by_id(ready_id)
        .await
        .expect("expired hold still exists");
    assert_eq!(expired.status, HoldStatus::Expired);

    let next = repo
        .holds_get_by_id(next_id)
        .await
        .expect("next hold still exists");
    assert_eq!(
        next.status,
        HoldStatus::Ready,
        "expiring a ready hold must promote the next pending patron"
    );
    assert!(next.notified_at.is_some());
    assert!(next.expires_at.is_some());
}

#[tokio::test]
async fn cancel_pending_hold_does_not_ready_next_while_ready_exists() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };

    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (repo, ready_id, pending_id) = seed_ready_then_pending(
        &app,
        &admin_token,
        "CANCEL-PENDING-001",
        "Cancel Pending Does Not Double Ready",
        "cancelpend_a",
        "cancelpend_b",
    )
    .await;

    let (reader_c_id, _) = fixtures::create_reader(&app, &admin_token, "cancelpend_c").await;
    let item_id = repo
        .holds_get_by_id(ready_id)
        .await
        .unwrap()
        .item_id
        .expect("ready hold has a copy");
    let hold_c = repo
        .holds_create(&create_hold(reader_c_id, item_id))
        .await
        .expect("third pending hold");

    let cancelled = repo
        .holds_cancel(pending_id)
        .await
        .expect("cancel pending hold");
    assert_eq!(cancelled.status, HoldStatus::Cancelled);

    let still_ready = repo.holds_get_by_id(ready_id).await.unwrap();
    assert_eq!(still_ready.status, HoldStatus::Ready);

    let third = repo.holds_get_by_id(hold_c.id).await.unwrap();
    assert_eq!(
        third.status,
        HoldStatus::Pending,
        "cancelling a pending hold must not mark another patron ready"
    );
}

fn assert_max_renewals(err: AppError) {
    match err {
        AppError::BusinessRule(msg) => {
            assert!(
                msg.starts_with("Maximum renewals reached"),
                "unexpected business rule: {msg}"
            );
        }
        other => panic!("expected max-renewals business rule, got {other:?}"),
    }
}

fn assert_hold_queue_blocks_renew(err: AppError) {
    match err {
        AppError::BusinessRule(msg) | AppError::Conflict(msg) => {
            assert!(
                msg.to_lowercase().contains("hold"),
                "expected hold-queue refusal, got {msg}"
            );
        }
        other => panic!("expected BusinessRule/Conflict for hold queue, got {other:?}"),
    }
}

#[tokio::test]
async fn concurrent_renew_same_loan_cannot_exceed_max_renewals() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };

    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_id, _) = fixtures::create_reader(&app, &admin_token, "renewrace").await;
    let item_id = seed_borrowable_item(
        &app,
        &admin_token,
        "RENEW-RACE-001",
        "Concurrent Renew Test",
    )
    .await;
    let repo = app.state.services.repository.as_ref().clone();

    let checkout = repo
        .loans_create(&create_loan(reader_id, item_id, false))
        .await
        .expect("checkout");

    // Leave one renewal slot (default max is 2) so both racing renews would pass a stale read.
    sqlx::query("UPDATE loans SET nb_renews = 1 WHERE id = $1")
        .bind(checkout.loan_id)
        .execute(repo.pool())
        .await
        .expect("seed one used renewal");

    let (r1, r2) = tokio::join!(
        repo.loans_renew(checkout.loan_id),
        repo.loans_renew(checkout.loan_id),
    );

    let oks = [&r1, &r2].iter().filter(|r| r.is_ok()).count();
    assert_eq!(
        oks, 1,
        "exactly one concurrent renew should succeed: {r1:?} {r2:?}"
    );

    let err = if r1.is_err() {
        r1.err().unwrap()
    } else {
        r2.err().unwrap()
    };
    assert_max_renewals(err);

    let loan = repo
        .loans_get_by_id(checkout.loan_id)
        .await
        .expect("loan after race");
    assert_eq!(loan.nb_renews, Some(2));
}

#[tokio::test]
async fn renew_blocked_when_hold_queue_has_waiting_patron() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };

    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_a_id, _) = fixtures::create_reader(&app, &admin_token, "renewhold_a").await;
    let (reader_b_id, _) = fixtures::create_reader(&app, &admin_token, "renewhold_b").await;
    let item_id = seed_borrowable_item(
        &app,
        &admin_token,
        "RENEW-HOLD-001",
        "Renew Hold Queue Test",
    )
    .await;
    let repo = app.state.services.repository.as_ref().clone();

    let checkout = repo
        .loans_create(&create_loan(reader_a_id, item_id, false))
        .await
        .expect("checkout to reader A");

    repo.holds_create(&CreateHold::for_item(reader_b_id, item_id))
        .await
        .expect("pending hold for reader B");

    let err = repo
        .loans_renew(checkout.loan_id)
        .await
        .expect_err("renew must fail while another patron is queued");
    assert_hold_queue_blocks_renew(err);

    let loan = repo
        .loans_get_by_id(checkout.loan_id)
        .await
        .expect("loan unchanged");
    assert_eq!(loan.nb_renews.unwrap_or(0), 0);
}

#[tokio::test]
async fn renew_blocked_when_ready_hold_exists_for_copy() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };

    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_a_id, _) = fixtures::create_reader(&app, &admin_token, "renewready_a").await;
    let (reader_b_id, _) = fixtures::create_reader(&app, &admin_token, "renewready_b").await;
    let item_id = seed_borrowable_item(
        &app,
        &admin_token,
        "RENEW-READY-001",
        "Renew Ready Hold Test",
    )
    .await;
    let repo = app.state.services.repository.as_ref().clone();

    let checkout = repo
        .loans_create(&create_loan(reader_a_id, item_id, false))
        .await
        .expect("checkout to reader A");

    let hold = repo
        .holds_create(&CreateHold::for_item(reader_b_id, item_id))
        .await
        .expect("pending hold for reader B");
    repo.holds_mark_ready(hold.id, 7)
        .await
        .expect("mark hold ready");

    let err = repo
        .loans_renew(checkout.loan_id)
        .await
        .expect_err("renew must fail while a ready hold exists");
    assert_hold_queue_blocks_renew(err);

    let loan = repo
        .loans_get_by_id(checkout.loan_id)
        .await
        .expect("loan unchanged");
    assert_eq!(loan.nb_renews.unwrap_or(0), 0);
}
