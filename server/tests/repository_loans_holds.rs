//! Repository-level tests: loan/hold atomicity and checkout race safety.

mod common;

use axum::http::StatusCode;
use common::fixtures;
use common::TestApp;
use elidune_server::error::AppError;
use elidune_server::models::hold::{CreateHold, HoldStatus};
use elidune_server::models::loan::CreateLoan;
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
        .holds_create(&CreateHold {
            user_id: reader_a_id,
            item_id,
            notes: None,
        })
        .await
        .expect("hold for reader A");
    let hold_b = repo
        .holds_create(&CreateHold {
            user_id: reader_b_id,
            item_id,
            notes: None,
        })
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

    repo.holds_create(&CreateHold {
        user_id: reader_a_id,
        item_id,
        notes: None,
    })
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
