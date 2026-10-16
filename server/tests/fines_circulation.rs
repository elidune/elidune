//! Checkout and renew are gated on the unpaid-fine threshold.

mod common;

use axum::http::StatusCode;
use common::fixtures;
use common::TestApp;
use serde_json::json;
use sqlx::Row;

fn untouched_default_statement() -> String {
    let migration = include_str!("../migrations/051_unpaid_threshold_nullable.sql");
    let start = migration
        .find("-- BEGIN UNTOUCHED DEFAULT THRESHOLD")
        .expect("start marker");
    let end = migration
        .find("-- END UNTOUCHED DEFAULT THRESHOLD")
        .expect("end marker");
    migration[start..end]
        .lines()
        .filter(|line| !line.trim_start().starts_with("--"))
        .collect::<Vec<_>>()
        .join("\n")
}

async fn migration_fixture_pool() -> Option<sqlx::PgPool> {
    if std::env::var("DATABASE_URL").is_err() && std::env::var("CI").is_err() {
        eprintln!("Skipping integration test: DATABASE_URL not set");
        return None;
    }
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL");
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect(&url)
        .await
        .expect("connect");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("migrate");
    Some(pool)
}

async fn prepare_threshold_fixtures(tx: &mut sqlx::Transaction<'_, sqlx::Postgres>) {
    sqlx::query(
        r#"
        CREATE TEMP TABLE circulation_settings (
            id BIGINT PRIMARY KEY,
            unpaid_fine_threshold NUMERIC(12, 2),
            unpaid_threshold_mode TEXT
        ) ON COMMIT DROP
        "#,
    )
    .execute(&mut **tx)
    .await
    .expect("temp circulation_settings");
    sqlx::query(
        r#"
        CREATE TEMP TABLE audit_log (
            id BIGSERIAL PRIMARY KEY,
            event_type TEXT NOT NULL,
            entity_type TEXT,
            entity_id BIGINT,
            outcome TEXT NOT NULL DEFAULT 'success'
        ) ON COMMIT DROP
        "#,
    )
    .execute(&mut **tx)
    .await
    .expect("temp audit_log");
    sqlx::query(
        r#"
        CREATE TEMP TABLE public_types (
            id BIGINT PRIMARY KEY,
            unpaid_fine_threshold NUMERIC(12, 2),
            unpaid_threshold_mode TEXT
        ) ON COMMIT DROP
        "#,
    )
    .execute(&mut **tx)
    .await
    .expect("temp public_types");
    sqlx::query(
        r#"
        INSERT INTO public_types (id, unpaid_fine_threshold) VALUES
            (1, 0),
            (2, NULL),
            (3, 4.50)
        "#,
    )
    .execute(&mut **tx)
    .await
    .expect("public type fixtures");
}

async fn apply_untouched_default(tx: &mut sqlx::Transaction<'_, sqlx::Postgres>) {
    sqlx::query("UPDATE circulation_settings SET unpaid_threshold_mode = NULL")
        .execute(&mut **tx)
        .await
        .expect("clear circulation mode");
    sqlx::query("UPDATE public_types SET unpaid_threshold_mode = NULL")
        .execute(&mut **tx)
        .await
        .expect("clear public type mode");
    for statement in untouched_default_statement().split(';') {
        let statement = statement.trim();
        if statement.is_empty() {
            continue;
        }
        sqlx::query(statement)
            .execute(&mut **tx)
            .await
            .expect("untouched default update");
    }
}

async fn mode_text(tx: &mut sqlx::Transaction<'_, sqlx::Postgres>, table: &str, id: i64) -> String {
    let sql = format!("SELECT unpaid_threshold_mode FROM {table} WHERE id = $1");
    let row = sqlx::query(&sql)
        .bind(id)
        .fetch_one(&mut **tx)
        .await
        .expect("read mode");
    row.get(0)
}

async fn threshold_text(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    table: &str,
    id: i64,
) -> Option<String> {
    let sql = format!("SELECT unpaid_fine_threshold::text FROM {table} WHERE id = $1");
    let row = sqlx::query(&sql)
        .bind(id)
        .fetch_one(&mut **tx)
        .await
        .expect("read threshold");
    row.get(0)
}

async fn create_borrowable_item(app: &TestApp, admin_token: &str, prefix: &str) -> i64 {
    let payload = json!({
        "title": format!("Fine threshold {prefix}"),
        "mediaType": "printedText",
        "lang": "french",
        "items": [{
            "barcode": format!("{prefix}-{}", fixtures::unique_suffix()),
            "borrowable": true
        }]
    });
    let (status, body) = app
        .post_json("/api/v1/biblios", &payload, Some(admin_token))
        .await;
    assert_eq!(status, StatusCode::CREATED, "create biblio: {body}");
    fixtures::json_id(&body["biblio"]["items"][0]["id"])
}

fn policy_lock() -> &'static tokio::sync::Mutex<()> {
    static LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    &LOCK
}

async fn checkout(
    app: &TestApp,
    admin_token: &str,
    user_id: i64,
    item_id: i64,
    force: bool,
) -> (StatusCode, serde_json::Value) {
    app.post_json(
        "/api/v1/loans",
        &json!({
            "userId": user_id.to_string(),
            "itemId": item_id.to_string(),
            "force": force
        }),
        Some(admin_token),
    )
    .await
}

async fn fine_override_payload(app: &TestApp, loan_id: i64) -> serde_json::Value {
    let pool = app.state.services.repository.pool();
    for _ in 0..40 {
        let row: Option<(String, serde_json::Value)> = sqlx::query_as(
            r#"
            SELECT entity_type, payload
            FROM audit_log
            WHERE event_type = 'loan.fine_threshold_overridden' AND entity_id = $1
            "#,
        )
        .bind(loan_id)
        .fetch_optional(pool)
        .await
        .expect("audit query");
        if let Some((entity_type, payload)) = row {
            assert_eq!(entity_type, "loan");
            return payload;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    panic!("loan.fine_threshold_overridden missing for loan {loan_id}");
}

#[tokio::test]
async fn unpaid_under_threshold_allows_checkout() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let _policy = policy_lock().lock().await;
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_id, _) = fixtures::create_reader(&app, &admin_token, "fineunder").await;

    let (status, body) = app
        .put_json(
            "/api/v1/fines/policy",
            &json!({ "unpaidThresholdMode": "amount", "unpaidThresholdCents": 1000 }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "set policy: {body}");

    let item_id = create_borrowable_item(&app, &admin_token, "UNDER").await;
    let (status, body) = checkout(&app, &admin_token, reader_id, item_id, false).await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "checkout under threshold: {body}"
    );
}

#[tokio::test]
async fn unpaid_over_threshold_blocks_checkout_and_force_is_audited() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let _policy = policy_lock().lock().await;
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_id, _) = fixtures::create_reader(&app, &admin_token, "fineover").await;

    let (status, body) = app
        .put_json(
            "/api/v1/fines/policy",
            &json!({ "unpaidThresholdMode": "amount", "unpaidThresholdCents": 500 }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "set policy: {body}");

    let first_item = create_borrowable_item(&app, &admin_token, "OVER1").await;
    let (status, body) = checkout(&app, &admin_token, reader_id, first_item, false).await;
    assert_eq!(status, StatusCode::CREATED, "seed loan: {body}");
    let loan_id = fixtures::json_id(&body["id"]);

    app.state
        .services
        .repository
        .fines_create(
            loan_id,
            reader_id,
            elidune_server::models::fine::Cents(1500),
            Some("overdue"),
        )
        .await
        .expect("create unpaid fine");

    let blocked_item = create_borrowable_item(&app, &admin_token, "OVER2").await;
    let (status, body) = checkout(&app, &admin_token, reader_id, blocked_item, false).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "blocked: {body}");
    let message = body["message"].as_str().unwrap_or_default();
    assert!(message.contains("force=true"), "{message}");

    let (status, body) = checkout(&app, &admin_token, reader_id, blocked_item, true).await;
    assert_eq!(status, StatusCode::CREATED, "force checkout: {body}");
    let forced_loan_id = fixtures::json_id(&body["id"]);

    let payload = fine_override_payload(&app, forced_loan_id).await;
    assert_eq!(payload["reason"], "fines_over_threshold", "{payload}");
    assert_eq!(payload["loanId"], json!(forced_loan_id), "{payload}");
    assert_eq!(payload["patron_id"], json!(reader_id), "{payload}");
    assert!(payload.get("operation").is_none(), "{payload}");
    assert!(payload.get("unpaid").is_none(), "{payload}");
}

#[tokio::test]
async fn unpaid_over_threshold_blocks_renew_unless_forced() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let _policy = policy_lock().lock().await;
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_id, _) = fixtures::create_reader(&app, &admin_token, "finerenew").await;

    let (status, body) = app
        .put_json(
            "/api/v1/fines/policy",
            &json!({ "unpaidThresholdMode": "amount", "unpaidThresholdCents": 0 }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "set policy: {body}");

    let item_id = create_borrowable_item(&app, &admin_token, "RENEW").await;
    let (status, body) = checkout(&app, &admin_token, reader_id, item_id, false).await;
    assert_eq!(status, StatusCode::CREATED, "checkout: {body}");
    let loan_id = fixtures::json_id(&body["id"]);
    fixtures::pull_loan_into_renewal_window(&app, loan_id).await;

    app.state
        .services
        .repository
        .fines_create(
            loan_id,
            reader_id,
            elidune_server::models::fine::Cents(250),
            Some("late"),
        )
        .await
        .expect("create unpaid fine");

    let (status, body) = app
        .post_empty(
            &format!("/api/v1/loans/{loan_id}/renew"),
            Some(&admin_token),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "renew blocked: {body}"
    );

    let (status, body) = app
        .post_empty(
            &format!("/api/v1/loans/{loan_id}/renew?force=true"),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "force renew: {body}");
}

#[tokio::test]
async fn untouched_default_zero_threshold_becomes_null() {
    let Some(pool) = migration_fixture_pool().await else {
        return;
    };
    let mut tx = pool.begin().await.expect("begin");
    prepare_threshold_fixtures(&mut tx).await;

    sqlx::query("INSERT INTO circulation_settings (id, unpaid_fine_threshold) VALUES (1, 0)")
        .execute(&mut *tx)
        .await
        .expect("seed zero");
    apply_untouched_default(&mut tx).await;
    assert_eq!(mode_text(&mut tx, "circulation_settings", 1).await, "none");
    assert_eq!(
        threshold_text(&mut tx, "circulation_settings", 1)
            .await
            .as_deref(),
        Some("0.00")
    );

    sqlx::query("UPDATE circulation_settings SET unpaid_fine_threshold = 0 WHERE id = 1")
        .execute(&mut *tx)
        .await
        .expect("restore zero");
    sqlx::query(
        r#"
        INSERT INTO audit_log (event_type, entity_type, entity_id, outcome)
        VALUES ('fine.policy_updated', 'circulation_settings', 1, 'failure')
        "#,
    )
    .execute(&mut *tx)
    .await
    .expect("failure audit");
    apply_untouched_default(&mut tx).await;
    assert_eq!(
        mode_text(&mut tx, "circulation_settings", 1).await,
        "none",
        "a failed policy write does not count as a modification"
    );

    tx.rollback().await.unwrap();
}

#[tokio::test]
async fn explicit_threshold_including_zero_is_unchanged() {
    let Some(pool) = migration_fixture_pool().await else {
        return;
    };
    let mut tx = pool.begin().await.expect("begin");
    prepare_threshold_fixtures(&mut tx).await;

    sqlx::query("INSERT INTO circulation_settings (id, unpaid_fine_threshold) VALUES (1, 0)")
        .execute(&mut *tx)
        .await
        .expect("explicit zero");
    sqlx::query(
        r#"
        INSERT INTO audit_log (event_type, entity_type, entity_id, outcome)
        VALUES ('fine.policy_updated', 'circulation_settings', 1, 'success')
        "#,
    )
    .execute(&mut *tx)
    .await
    .expect("success audit");
    apply_untouched_default(&mut tx).await;
    assert_eq!(
        mode_text(&mut tx, "circulation_settings", 1).await,
        "amount"
    );
    assert_eq!(
        threshold_text(&mut tx, "circulation_settings", 1)
            .await
            .as_deref(),
        Some("0.00")
    );

    sqlx::query("UPDATE circulation_settings SET unpaid_fine_threshold = 12.50 WHERE id = 1")
        .execute(&mut *tx)
        .await
        .expect("explicit amount");
    apply_untouched_default(&mut tx).await;
    assert_eq!(
        mode_text(&mut tx, "circulation_settings", 1).await,
        "amount"
    );
    assert_eq!(
        threshold_text(&mut tx, "circulation_settings", 1)
            .await
            .as_deref(),
        Some("12.50")
    );

    sqlx::query("DELETE FROM audit_log")
        .execute(&mut *tx)
        .await
        .expect("clear audit");
    sqlx::query("UPDATE circulation_settings SET unpaid_fine_threshold = 4.50 WHERE id = 1")
        .execute(&mut *tx)
        .await
        .expect("nonzero without audit");
    apply_untouched_default(&mut tx).await;
    assert_eq!(
        mode_text(&mut tx, "circulation_settings", 1).await,
        "amount"
    );
    assert_eq!(
        threshold_text(&mut tx, "circulation_settings", 1)
            .await
            .as_deref(),
        Some("4.50"),
        "only an untouched 0 becomes none"
    );

    assert_eq!(mode_text(&mut tx, "public_types", 1).await, "amount");
    assert_eq!(
        threshold_text(&mut tx, "public_types", 1).await.as_deref(),
        Some("0.00")
    );
    assert_eq!(mode_text(&mut tx, "public_types", 2).await, "inherit");
    assert_eq!(threshold_text(&mut tx, "public_types", 2).await, None);
    assert_eq!(mode_text(&mut tx, "public_types", 3).await, "amount");
    assert_eq!(
        threshold_text(&mut tx, "public_types", 3).await.as_deref(),
        Some("4.50")
    );

    tx.rollback().await.unwrap();
}

#[tokio::test]
async fn no_unpaid_threshold_means_no_block() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let _policy = policy_lock().lock().await;
    let admin_token = fixtures::ensure_first_setup(&app).await;

    let (status, body) = app
        .put_json(
            "/api/v1/fines/policy",
            &json!({ "unpaidThresholdMode": "none" }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "disable global threshold: {body}");
    assert_eq!(body["unpaidThresholdMode"], "none", "{body}");
    assert!(
        body.get("unpaidThresholdCents")
            .is_none_or(|cents| cents.is_null()),
        "none omits cents: {body}"
    );

    let name = format!("nothresh_{}", fixtures::unique_suffix());
    let (status, created) = app
        .post_json(
            "/api/v1/public-types",
            &json!({
                "name": name,
                "label": name,
                "unpaidThresholdMode": "inherit"
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "public type: {created}");
    assert_eq!(created["unpaidThresholdMode"], "inherit", "{created}");
    assert!(
        created
            .get("unpaidThresholdCents")
            .is_none_or(|cents| cents.is_null()),
        "inherit omits cents: {created}"
    );
    let public_type_id = fixtures::json_id(&created["id"]);
    let (status, settings) = app
        .put_json(
            &format!("/api/v1/public-types/{public_type_id}/loan-settings"),
            &json!({ "settings": [{ "nbMax": 5, "duration": 21, "nbRenews": 1 }] }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "loan settings: {settings}");

    let suffix = fixtures::unique_suffix();
    let card = format!("CARD-{suffix}");
    let (status, patron) = app
        .post_json(
            "/api/v1/users",
            &json!({
                "login": format!("noblock_{suffix}"),
                "password": "readerpass1234",
                "firstname": "Nora",
                "lastname": "Blanc",
                "email": format!("noblock_{suffix}@test.local"),
                "accountType": "reader",
                "publicType": public_type_id.to_string(),
                "barcode": card,
                "sex": "f",
                "birthdate": "1990-06-01",
                "addrCity": "Paris"
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "patron: {patron}");
    let reader_id = fixtures::json_id(&patron["id"]);
    fixtures::assign_public_type(&app, reader_id, public_type_id).await;
    app.state
        .services
        .users
        .set_must_change_password(reader_id, false)
        .await
        .expect("clear password flag");

    let first_item = create_borrowable_item(&app, &admin_token, "NULL1").await;
    let (status, loan) = checkout(&app, &admin_token, reader_id, first_item, false).await;
    assert_eq!(status, StatusCode::CREATED, "seed loan: {loan}");
    let loan_id = fixtures::json_id(&loan["id"]);
    app.state
        .services
        .repository
        .fines_create(
            loan_id,
            reader_id,
            elidune_server::models::fine::Cents(2500),
            Some("open"),
        )
        .await
        .expect("create unpaid fine");

    let second_item = create_borrowable_item(&app, &admin_token, "NULL2").await;
    let (status, borrowed) = checkout(&app, &admin_token, reader_id, second_item, false).await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "null threshold allows checkout with unpaid fines: {borrowed}"
    );

    let (status, scan) = app
        .get_json_with_auth(&format!("/api/v1/users/by-card/{card}"), &admin_token)
        .await;
    assert_eq!(status, StatusCode::OK, "card scan: {scan}");
    let reasons = scan["loanStatus"]["blockReasons"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert!(
        !reasons.iter().any(|reason| reason == "unpaid_fines"),
        "card scan must not report unpaid_fines: {scan}"
    );
    assert_eq!(scan["loanStatus"]["unpaidFinesCents"], 2500, "{scan}");
}

async fn create_mode_audience(
    app: &TestApp,
    admin_token: &str,
    mode: &str,
    cents: Option<i64>,
) -> i64 {
    let name = format!("mode_{}", fixtures::unique_suffix());
    let mut body = json!({
        "name": name,
        "label": name,
        "unpaidThresholdMode": mode
    });
    if let Some(cents) = cents {
        body["unpaidThresholdCents"] = json!(cents);
    }
    let (status, created) = app
        .post_json("/api/v1/public-types", &body, Some(admin_token))
        .await;
    assert_eq!(status, StatusCode::CREATED, "public type: {created}");
    let public_type_id = fixtures::json_id(&created["id"]);
    let (status, settings) = app
        .put_json(
            &format!("/api/v1/public-types/{public_type_id}/loan-settings"),
            &json!({ "settings": [{ "nbMax": 5, "duration": 21, "nbRenews": 1 }] }),
            Some(admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "loan settings: {settings}");
    public_type_id
}

async fn patron_with_unpaid(
    app: &TestApp,
    admin_token: &str,
    public_type_id: i64,
    prefix: &str,
    unpaid_cents: i64,
) -> (i64, String) {
    let suffix = fixtures::unique_suffix();
    let card = format!("CARD-{prefix}-{suffix}");
    let (status, patron) = app
        .post_json(
            "/api/v1/users",
            &json!({
                "login": format!("{prefix}_{suffix}"),
                "password": "readerpass1234",
                "firstname": "Ada",
                "lastname": "Fine",
                "email": format!("{prefix}_{suffix}@test.local"),
                "accountType": "reader",
                "publicType": public_type_id.to_string(),
                "barcode": card,
                "sex": "f",
                "birthdate": "1990-06-01",
                "addrCity": "Paris"
            }),
            Some(admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "patron: {patron}");
    let reader_id = fixtures::json_id(&patron["id"]);
    fixtures::assign_public_type(app, reader_id, public_type_id).await;
    app.state
        .services
        .users
        .set_must_change_password(reader_id, false)
        .await
        .expect("clear password flag");
    let seed = create_borrowable_item(app, admin_token, &format!("{prefix}SEED")).await;
    let (status, loan) = checkout(app, admin_token, reader_id, seed, false).await;
    assert_eq!(status, StatusCode::CREATED, "seed loan: {loan}");
    app.state
        .services
        .repository
        .fines_create(
            fixtures::json_id(&loan["id"]),
            reader_id,
            elidune_server::models::fine::Cents(unpaid_cents),
            Some("open"),
        )
        .await
        .expect("create unpaid fine");
    (reader_id, card)
}

fn assert_no_unpaid_fines(scan: &serde_json::Value) {
    let reasons = scan["loanStatus"]["blockReasons"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert!(
        !reasons.iter().any(|reason| reason == "unpaid_fines"),
        "card scan must not report unpaid_fines: {scan}"
    );
}

#[tokio::test]
async fn global_10_euros_youth_type_none_15_due_not_blocked() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let _policy = policy_lock().lock().await;
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (status, body) = app
        .put_json(
            "/api/v1/fines/policy",
            &json!({ "unpaidThresholdMode": "amount", "unpaidThresholdCents": 1000 }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "global 10 euros: {body}");

    let youth = create_mode_audience(&app, &admin_token, "none", None).await;
    let (reader_id, card) = patron_with_unpaid(&app, &admin_token, youth, "youthnone", 1500).await;
    let item = create_borrowable_item(&app, &admin_token, "YOUTH").await;
    let (status, borrowed) = checkout(&app, &admin_token, reader_id, item, false).await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "type none ignores a global amount: {borrowed}"
    );
    let (status, scan) = app
        .get_json_with_auth(&format!("/api/v1/users/by-card/{card}"), &admin_token)
        .await;
    assert_eq!(status, StatusCode::OK, "card scan: {scan}");
    assert_no_unpaid_fines(&scan);
}

#[tokio::test]
async fn inherit_follows_global_threshold() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let _policy = policy_lock().lock().await;
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (status, body) = app
        .put_json(
            "/api/v1/fines/policy",
            &json!({ "unpaidThresholdMode": "amount", "unpaidThresholdCents": 1000 }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "global amount: {body}");

    let audience = create_mode_audience(&app, &admin_token, "inherit", None).await;
    let (reader_id, card) = patron_with_unpaid(&app, &admin_token, audience, "inherit", 1500).await;
    let blocked = create_borrowable_item(&app, &admin_token, "INHBLOCK").await;
    let (status, body) = checkout(&app, &admin_token, reader_id, blocked, false).await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "inherit follows global amount: {body}"
    );

    let (status, body) = app
        .put_json(
            "/api/v1/fines/policy",
            &json!({ "unpaidThresholdMode": "none" }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "global none: {body}");
    let (status, borrowed) = checkout(&app, &admin_token, reader_id, blocked, false).await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "inherit follows global none: {borrowed}"
    );
    let (status, scan) = app
        .get_json_with_auth(&format!("/api/v1/users/by-card/{card}"), &admin_token)
        .await;
    assert_eq!(status, StatusCode::OK, "card scan: {scan}");
    assert_no_unpaid_fines(&scan);
}

#[tokio::test]
async fn public_type_amount_overrides_global() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let _policy = policy_lock().lock().await;
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (status, body) = app
        .put_json(
            "/api/v1/fines/policy",
            &json!({ "unpaidThresholdMode": "amount", "unpaidThresholdCents": 1000 }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "global 10 euros: {body}");

    let audience = create_mode_audience(&app, &admin_token, "amount", Some(2000)).await;
    let (reader_id, _) = patron_with_unpaid(&app, &admin_token, audience, "override", 1500).await;
    let item = create_borrowable_item(&app, &admin_token, "OVR").await;
    let (status, borrowed) = checkout(&app, &admin_token, reader_id, item, false).await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "type amount 20 euros allows 15 due under a global 10: {borrowed}"
    );

    let (status, updated) = app
        .put_json(
            &format!("/api/v1/public-types/{audience}"),
            &json!({ "unpaidThresholdMode": "amount", "unpaidThresholdCents": 500 }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "lower type amount: {updated}");
    assert_eq!(updated["unpaidThresholdCents"], 500, "{updated}");

    let (status, body) = app
        .put_json(
            "/api/v1/fines/policy",
            &json!({ "unpaidThresholdMode": "none" }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "global none: {body}");
    let next = create_borrowable_item(&app, &admin_token, "OVR2").await;
    let (status, blocked) = checkout(&app, &admin_token, reader_id, next, false).await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "type amount still blocks when global is none: {blocked}"
    );
}

#[tokio::test]
async fn put_without_mode_leaves_threshold_unchanged() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let _policy = policy_lock().lock().await;
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (status, body) = app
        .put_json(
            "/api/v1/fines/policy",
            &json!({ "unpaidThresholdMode": "amount", "unpaidThresholdCents": 1000 }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "set global: {body}");

    let (status, body) = app
        .put_json("/api/v1/fines/policy", &json!({}), Some(&admin_token))
        .await;
    assert_eq!(status, StatusCode::OK, "empty put: {body}");
    assert_eq!(body["unpaidThresholdMode"], "amount", "{body}");
    assert_eq!(body["unpaidThresholdCents"], 1000, "{body}");

    let (status, body) = app
        .put_json(
            "/api/v1/fines/policy",
            &json!({ "unpaidThresholdCents": 50 }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "cents without mode: {body}");
    assert_eq!(body["unpaidThresholdCents"], 1000, "{body}");

    let audience = create_mode_audience(&app, &admin_token, "amount", Some(450)).await;
    let (status, kept) = app
        .put_json(
            &format!("/api/v1/public-types/{audience}"),
            &json!({ "label": "kept label" }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "omit mode: {kept}");
    assert_eq!(kept["unpaidThresholdMode"], "amount", "{kept}");
    assert_eq!(kept["unpaidThresholdCents"], 450, "{kept}");

    let (status, kept) = app
        .put_json(
            &format!("/api/v1/public-types/{audience}"),
            &json!({ "unpaidThresholdCents": 10 }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "cents without mode: {kept}");
    assert_eq!(kept["unpaidThresholdMode"], "amount", "{kept}");
    assert_eq!(kept["unpaidThresholdCents"], 450, "{kept}");
}

#[tokio::test]
async fn amount_mode_without_cents_is_422() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let _policy = policy_lock().lock().await;
    let admin_token = fixtures::ensure_first_setup(&app).await;

    let (status, body) = app
        .put_json(
            "/api/v1/fines/policy",
            &json!({ "unpaidThresholdMode": "amount" }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["code"], "validation_error", "{body}");
    assert!(
        body["message"]
            .as_str()
            .unwrap_or_default()
            .contains("unpaidThresholdCents"),
        "{body}"
    );

    let (status, _) = app
        .put_json(
            "/api/v1/fines/policy",
            &json!({ "unpaidThresholdMode": null }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    let name = format!("nocents_{}", fixtures::unique_suffix());
    let (status, body) = app
        .post_json(
            "/api/v1/public-types",
            &json!({
                "name": name,
                "label": name,
                "unpaidThresholdMode": "amount"
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["code"], "validation_error", "{body}");

    let audience = create_mode_audience(&app, &admin_token, "inherit", None).await;
    let (status, body) = app
        .put_json(
            &format!("/api/v1/public-types/{audience}"),
            &json!({ "unpaidThresholdMode": "amount" }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["code"], "validation_error", "{body}");

    let (status, _) = app
        .put_json(
            &format!("/api/v1/public-types/{audience}"),
            &json!({ "unpaidThresholdMode": null }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    let (status, body) = app
        .put_json(
            "/api/v1/fines/policy",
            &json!({ "unpaidThresholdMode": "amount", "unpaidThresholdCents": -1 }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
}
