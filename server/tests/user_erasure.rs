//! Patron GDPR erasure: PII scrub, identity cut, archive stats dimensions.

mod common;

use axum::http::StatusCode;
use chrono::Datelike;
use common::fixtures;
use common::TestApp;
use rust_decimal::Decimal;
use serde_json::json;
use sqlx::Row;

async fn create_borrowable_item(app: &TestApp, admin_token: &str, prefix: &str) -> i64 {
    let payload = json!({
        "title": format!("Erasure {prefix}"),
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

async fn seed_direct_identifiers(app: &TestApp, user_id: i64, suffix: &str) {
    sqlx::query(
        r#"
        UPDATE users SET
            barcode = $2,
            phone = $3,
            notes = $4,
            addr_street = $5,
            addr_zip_code = $6,
            totp_secret = $7,
            recovery_codes = $8,
            two_factor_enabled = TRUE,
            two_factor_method = 'totp'
        WHERE id = $1
        "#,
    )
    .bind(user_id)
    .bind(format!("BC-{suffix}"))
    .bind("+33600000000")
    .bind("private medical note")
    .bind("1 rue Test")
    .bind(75001_i32)
    .bind("SECRET-TOTP")
    .bind("recovery-one")
    .execute(app.state.services.repository.pool())
    .await
    .expect("seed PII columns");
}

async fn assert_users_row_scrubbed(app: &TestApp, user_id: i64) {
    let row = sqlx::query(
        r#"
        SELECT login, password, firstname, lastname, email, addr_street, addr_zip_code,
               addr_city, phone, barcode, notes, birthdate, sex, fee, group_id,
               staff_type, hours_per_week, staff_start_date, staff_end_date,
               language, two_factor_enabled, two_factor_method, totp_secret,
               recovery_codes, recovery_codes_used, receive_reminders,
               must_change_password, status, public_type, account_type
        FROM users WHERE id = $1
        "#,
    )
    .bind(user_id)
    .fetch_one(app.state.services.repository.pool())
    .await
    .expect("load scrubbed user");

    for col in [
        "login",
        "password",
        "firstname",
        "lastname",
        "email",
        "addr_street",
        "addr_city",
        "phone",
        "barcode",
        "notes",
        "fee",
        "language",
        "two_factor_method",
        "totp_secret",
        "recovery_codes",
        "recovery_codes_used",
    ] {
        let v: Option<String> = row.get(col);
        assert!(v.is_none(), "{col} should be null, got {v:?}");
    }

    let zip: Option<i32> = row.get("addr_zip_code");
    assert!(zip.is_none(), "addr_zip_code should be null");
    let birth: Option<chrono::NaiveDate> = row.get("birthdate");
    assert!(birth.is_none(), "birthdate should be null");
    let sex: Option<String> = row.get("sex");
    assert!(sex.is_none(), "sex should be null");
    let group_id: Option<i64> = row.get("group_id");
    assert!(group_id.is_none(), "group_id should be null");
    let staff_type: Option<i16> = row.get("staff_type");
    assert!(staff_type.is_none(), "staff_type should be null");
    let hours: Option<f32> = row.get("hours_per_week");
    assert!(hours.is_none(), "hours_per_week should be null");
    let staff_start: Option<chrono::NaiveDate> = row.get("staff_start_date");
    assert!(staff_start.is_none(), "staff_start_date should be null");
    let staff_end: Option<chrono::NaiveDate> = row.get("staff_end_date");
    assert!(staff_end.is_none(), "staff_end_date should be null");

    let two_fa: Option<bool> = row.get("two_factor_enabled");
    assert_eq!(two_fa, Some(false));
    let reminders: bool = row.get("receive_reminders");
    assert!(!reminders);
    let must_change: bool = row.get("must_change_password");
    assert!(!must_change);

    let status: Option<String> = row.get("status");
    assert_eq!(status.as_deref(), Some("deleted"));
    let account_type: String = row.get("account_type");
    assert_eq!(account_type, "reader");
    let public_type: Option<i64> = row.get("public_type");
    assert!(public_type.is_some(), "public_type is a kept stats dim");
}

async fn assert_history_unlinked(app: &TestApp, user_id: i64) {
    let archive_links: i64 =
        sqlx::query_scalar("SELECT COUNT(*)::bigint FROM loans_archives WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(app.state.services.repository.pool())
            .await
            .expect("count archive links");
    assert_eq!(archive_links, 0, "archives must not keep user_id");

    let loan_links: i64 =
        sqlx::query_scalar("SELECT COUNT(*)::bigint FROM loans WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(app.state.services.repository.pool())
            .await
            .expect("count loan links");
    assert_eq!(loan_links, 0, "active loans must not keep user_id");

    let fine_links: i64 =
        sqlx::query_scalar("SELECT COUNT(*)::bigint FROM fines WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(app.state.services.repository.pool())
            .await
            .expect("count fine links");
    assert_eq!(fine_links, 0, "fines must not keep user_id");

    let holds: i64 = sqlx::query_scalar("SELECT COUNT(*)::bigint FROM holds WHERE user_id = $1")
        .bind(user_id)
        .fetch_one(app.state.services.repository.pool())
        .await
        .expect("count holds");
    assert_eq!(holds, 0, "holds must be cancelled");
}

async fn latest_user_deleted_audit(
    app: &TestApp,
    admin_token: &str,
    user_id: i64,
) -> serde_json::Value {
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    let (status, audit) = app
        .get_json_with_auth("/api/v1/audit?eventType=user.deleted", admin_token)
        .await;
    assert_eq!(status, StatusCode::OK, "audit query: {audit}");
    let entries = audit["entries"].as_array().cloned().unwrap_or_default();
    entries
        .into_iter()
        .find(|row| row["payload"]["id"] == user_id || row["entityId"] == user_id)
        .unwrap_or_else(|| panic!("expected user.deleted audit for {user_id}, got {audit}"))
}

fn assert_audit_has_no_erased_pii(entry: &serde_json::Value) {
    let payload = &entry["payload"];
    let dumped = payload.to_string().to_lowercase();
    for forbidden in [
        "readerpass",
        "test.local",
        "private medical",
        "secret-totp",
        "recovery-one",
        "+33600000000",
        "1 rue test",
        "bc-",
    ] {
        assert!(
            !dumped.contains(forbidden),
            "audit payload must not store erased PII: {payload}"
        );
    }
    assert_eq!(payload["anonymized"], true);
}

#[tokio::test]
async fn delete_without_loans_scrubs_pii_and_unlinks_history() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_id, reader_token) = fixtures::create_reader(&app, &admin_token, "eraseok").await;
    seed_direct_identifiers(&app, reader_id, "ok").await;

    let item_id = create_borrowable_item(&app, &admin_token, "ERSOK").await;
    let (status, body) = app
        .post_json(
            "/api/v1/loans",
            &json!({
                "userId": reader_id.to_string(),
                "itemId": item_id.to_string()
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "checkout: {body}");
    let loan_id = fixtures::json_id(&body["id"]);

    let (status, body) = app
        .post_empty(
            &format!("/api/v1/loans/{loan_id}/return"),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "return: {body}");

    let (status, body) = app
        .delete_with_auth(&format!("/api/v1/users/{reader_id}"), &admin_token)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "delete: {body}");

    assert_users_row_scrubbed(&app, reader_id).await;
    assert_history_unlinked(&app, reader_id).await;

    let archive = sqlx::query(
        r#"
        SELECT user_id, addr_city, account_type, borrower_public_type, age_band, loan_year
        FROM loans_archives
        WHERE item_id = $1
        ORDER BY id DESC
        LIMIT 1
        "#,
    )
    .bind(item_id)
    .fetch_one(app.state.services.repository.pool())
    .await
    .expect("archive row");

    let linked: Option<i64> = archive.get("user_id");
    assert!(linked.is_none());
    let city: Option<String> = archive.get("addr_city");
    assert_eq!(city.as_deref(), Some("Paris"));
    let account_type: Option<String> = archive.get("account_type");
    assert_eq!(account_type.as_deref(), Some("reader"));
    let public_type: Option<i64> = archive.get("borrower_public_type");
    assert!(public_type.is_some());
    let age_band: Option<String> = archive.get("age_band");
    assert_eq!(age_band.as_deref(), Some("30-49"));
    let loan_year: Option<i16> = archive.get("loan_year");
    assert_eq!(loan_year, Some(chrono::Utc::now().year() as i16));

    let (status, me) = app
        .get_json_with_auth("/api/v1/auth/me", &reader_token)
        .await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "erased patron JWT must be revoked: {me}"
    );

    let audit = latest_user_deleted_audit(&app, &admin_token, reader_id).await;
    assert_audit_has_no_erased_pii(&audit);
    assert_eq!(audit["payload"]["force"], false);
}

#[tokio::test]
async fn delete_with_active_loans_is_refused_without_force() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_id, _) = fixtures::create_reader(&app, &admin_token, "eraserefuse").await;
    let item_id = create_borrowable_item(&app, &admin_token, "ERREF").await;

    let (status, body) = app
        .post_json(
            "/api/v1/loans",
            &json!({
                "userId": reader_id.to_string(),
                "itemId": item_id.to_string()
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "checkout: {body}");

    let (status, body) = app
        .delete_with_auth(&format!("/api/v1/users/{reader_id}"), &admin_token)
        .await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "expected active-loan refusal: {body}"
    );

    let login: Option<String> = sqlx::query_scalar("SELECT login FROM users WHERE id = $1")
        .bind(reader_id)
        .fetch_one(app.state.services.repository.pool())
        .await
        .expect("login still present");
    assert!(login.is_some(), "PII must remain when delete is refused");
}

#[tokio::test]
async fn force_delete_with_loans_and_fines_snapshots_then_cuts_identity() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_id, _) = fixtures::create_reader(&app, &admin_token, "eraseforce").await;
    seed_direct_identifiers(&app, reader_id, "force").await;

    let item_id = create_borrowable_item(&app, &admin_token, "ERFOR").await;
    let (status, body) = app
        .post_json(
            "/api/v1/loans",
            &json!({
                "userId": reader_id.to_string(),
                "itemId": item_id.to_string()
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "checkout: {body}");
    let loan_id = fixtures::json_id(&body["id"]);

    app.state
        .services
        .repository
        .fines_create(loan_id, reader_id, Decimal::new(350, 2), Some("overdue"))
        .await
        .expect("create fine");

    let (status, body) = app
        .delete_with_auth(
            &format!("/api/v1/users/{reader_id}?force=true"),
            &admin_token,
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "force delete: {body}");

    assert_users_row_scrubbed(&app, reader_id).await;
    assert_history_unlinked(&app, reader_id).await;

    let archive = sqlx::query(
        r#"
        SELECT user_id, addr_city, account_type, borrower_public_type, age_band, loan_year, item_id
        FROM loans_archives
        WHERE item_id = $1
        ORDER BY id DESC
        LIMIT 1
        "#,
    )
    .bind(item_id)
    .fetch_one(app.state.services.repository.pool())
    .await
    .expect("force-returned archive");

    let linked: Option<i64> = archive.get("user_id");
    assert!(linked.is_none());
    let city: Option<String> = archive.get("addr_city");
    assert_eq!(city.as_deref(), Some("Paris"));
    let age_band: Option<String> = archive.get("age_band");
    assert_eq!(age_band.as_deref(), Some("30-49"));
    let loan_year: Option<i16> = archive.get("loan_year");
    assert_eq!(loan_year, Some(chrono::Utc::now().year() as i16));

    let fine_user: Option<i64> = sqlx::query_scalar("SELECT user_id FROM fines WHERE loan_id = $1")
        .bind(loan_id)
        .fetch_one(app.state.services.repository.pool())
        .await
        .expect("fine after erasure");
    assert!(fine_user.is_none());

    let amount: Decimal = sqlx::query_scalar("SELECT amount FROM fines WHERE loan_id = $1")
        .bind(loan_id)
        .fetch_one(app.state.services.repository.pool())
        .await
        .expect("fine amount kept");
    assert_eq!(amount, Decimal::new(350, 2));

    let audit = latest_user_deleted_audit(&app, &admin_token, reader_id).await;
    assert_audit_has_no_erased_pii(&audit);
    assert_eq!(audit["payload"]["force"], true);
    assert!(
        audit["payload"]["loansForceReturned"].as_u64().unwrap_or(0) >= 1,
        "force path should record returned loans: {}",
        audit["payload"]
    );
    assert!(
        audit["payload"]["finesUnlinked"].as_u64().unwrap_or(0) >= 1,
        "force path should record unlinked fines: {}",
        audit["payload"]
    );
}
