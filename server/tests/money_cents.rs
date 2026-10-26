//! Integer-cent storage for item prices, subscription prices, and fines.

mod common;

use axum::http::StatusCode;
use common::fixtures;
use common::TestApp;
use serde_json::json;

#[test]
fn stats_sources_do_not_read_stored_money_columns() {
    let sources = [
        include_str!("../src/api/stats.rs"),
        include_str!("../src/models/dto/stats.rs"),
        include_str!("../src/models/stats_builder.rs"),
    ];
    for source in sources {
        assert!(!source.contains("items.price"));
        assert!(!source.contains("subscription_price"));
        assert!(!source.contains("fines.amount"));
    }
}

#[tokio::test]
async fn adult_subscription_price_stays_integer_cents() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let (status, body) = app.get_json("/api/v1/public-types").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let adult = body
        .as_array()
        .expect("list")
        .iter()
        .find(|row| row["name"] == "adult")
        .expect("adult public type");
    assert_eq!(adult["subscriptionPrice"], 1500, "{adult}");
}

#[tokio::test]
async fn item_price_accepts_a_comma_and_returns_canonical_euros() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let barcode = format!("CENTS-{}", fixtures::unique_suffix());
    let (status, created) = app
        .post_json(
            "/api/v1/biblios",
            &json!({
                "title": format!("Cents {}", fixtures::unique_suffix()),
                "mediaType": "printedText",
                "lang": "french",
                "items": [{
                    "barcode": barcode,
                    "borrowable": true,
                    "price": "12,50"
                }]
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    assert_eq!(created["biblio"]["items"][0]["price"], "12.50", "{created}");

    let item_id = fixtures::json_id(&created["biblio"]["items"][0]["id"]);
    let (status, updated) = app
        .put_json(
            &format!("/api/v1/items/{item_id}"),
            &json!({
                "barcode": barcode,
                "borrowable": true,
                "price": "15"
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    assert_eq!(updated["price"], "15.00", "{updated}");
    assert!(updated.get("priceDeferred").is_none(), "{updated}");
}

#[tokio::test]
async fn rejected_item_price_is_exposed_as_price_deferred() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let barcode = format!("DEFER-{}", fixtures::unique_suffix());
    let (status, created) = app
        .post_json(
            "/api/v1/biblios",
            &json!({
                "title": format!("Deferred {}", fixtures::unique_suffix()),
                "mediaType": "printedText",
                "lang": "french",
                "items": [{
                    "barcode": barcode,
                    "borrowable": true,
                    "price": "4.00"
                }]
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let item_id = fixtures::json_id(&created["biblio"]["items"][0]["id"]);

    let (status, before) = app
        .get_json_with_auth(&format!("/api/v1/items/{item_id}"), &admin_token)
        .await;
    assert_eq!(status, StatusCode::OK, "{before}");
    assert_eq!(before["items"][0]["price"], "4.00", "{before}");
    assert!(
        before["items"][0].get("priceDeferred").is_none(),
        "{before}"
    );

    sqlx::query("UPDATE items SET price = NULL, price_deferred = TRUE WHERE id = $1")
        .bind(item_id)
        .execute(app.state.services.repository.pool())
        .await
        .expect("mark rejected price");

    let (status, body) = app
        .get_json_with_auth(&format!("/api/v1/items/{item_id}"), &admin_token)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body["items"][0]["price"].is_null(), "{body}");
    assert_eq!(body["items"][0]["priceDeferred"], true, "{body}");
}

#[tokio::test]
async fn money_checks_reject_negative_cents() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (status, created) = app
        .post_json(
            "/api/v1/biblios",
            &json!({
                "title": format!("Check {}", fixtures::unique_suffix()),
                "mediaType": "printedText",
                "lang": "french",
                "items": [{
                    "barcode": format!("CHK-{}", fixtures::unique_suffix()),
                    "borrowable": true,
                    "price": "1.00"
                }]
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let item_id = fixtures::json_id(&created["biblio"]["items"][0]["id"]);
    let pool = app.state.services.repository.pool();

    let defs: Vec<(String, String)> = sqlx::query_as(
        r#"
        SELECT c.conname, pg_get_constraintdef(c.oid)
        FROM pg_constraint c
        JOIN pg_class t ON t.oid = c.conrelid
        JOIN pg_namespace n ON n.oid = t.relnamespace
        WHERE n.nspname = 'public'
          AND c.contype = 'c'
          AND t.relname IN ('items', 'fines', 'fine_rules', 'circulation_settings', 'public_types')
        "#,
    )
    .fetch_all(pool)
    .await
    .expect("check definitions");
    for name in [
        "items_price_nonnegative",
        "public_types_subscription_price_nonnegative",
        "fines_amount_check",
        "fines_paid_amount_check",
        "fines_refunded_amount_check",
        "fine_rules_daily_rate_check",
        "fine_rules_max_amount_check",
        "circulation_settings_unpaid_fine_threshold_check",
        "public_types_unpaid_fine_threshold_check",
        "fines_paid_amount_lte_amount",
    ] {
        let def = defs
            .iter()
            .find(|(constraint, _)| constraint == name)
            .unwrap_or_else(|| panic!("missing {name}: {defs:?}"));
        let nonnegative = def.1.contains(">= 0") || def.1.contains(">= (0)");
        let paid_cap = name == "fines_paid_amount_lte_amount"
            && def.1.contains("paid_amount")
            && (def.1.contains("<= amount") || def.1.contains("<= (amount)"));
        assert!(nonnegative || paid_cap, "{name}: {}", def.1);
    }

    let fine_id = (fixtures::unique_suffix() % i64::MAX as u128) as i64;
    expect_check(
        pool,
        &format!("UPDATE items SET price = -1 WHERE id = {item_id}"),
        "items_price_nonnegative",
    )
    .await;
    expect_check(
        pool,
        "UPDATE circulation_settings SET unpaid_fine_threshold = -1 WHERE id = 1",
        "circulation_settings_unpaid_fine_threshold_check",
    )
    .await;
    expect_check(
        pool,
        "UPDATE public_types SET unpaid_fine_threshold = -1 WHERE name = 'adult'",
        "public_types_unpaid_fine_threshold_check",
    )
    .await;
    expect_check(
        pool,
        "UPDATE public_types SET subscription_price = -1 WHERE name = 'adult'",
        "public_types_subscription_price_nonnegative",
    )
    .await;
    expect_check(
        pool,
        "UPDATE fine_rules SET daily_rate = -1 WHERE id = (SELECT id FROM fine_rules ORDER BY id LIMIT 1)",
        "fine_rules_daily_rate_check",
    )
    .await;
    expect_check(
        pool,
        "UPDATE fine_rules SET max_amount = -1 WHERE id = (SELECT id FROM fine_rules ORDER BY id LIMIT 1)",
        "fine_rules_max_amount_check",
    )
    .await;
    expect_check(
        pool,
        &format!("INSERT INTO fines (id, loan_id, amount) VALUES ({fine_id}, 1, -1)"),
        "fines_amount_check",
    )
    .await;
    expect_check(
        pool,
        &format!(
            "INSERT INTO fines (id, loan_id, amount, paid_amount) VALUES ({fine_id}, 1, 0, -1)"
        ),
        "fines_paid_amount_check",
    )
    .await;
    expect_check(
        pool,
        &format!(
            "INSERT INTO fines (id, loan_id, amount, refunded_amount) VALUES ({fine_id}, 1, 0, -1)"
        ),
        "fines_refunded_amount_check",
    )
    .await;
    expect_check(
        pool,
        &format!("INSERT INTO fines (id, loan_id, amount, paid_amount) VALUES ({fine_id}, {fine_id}, 100, 101)"),
        "fines_paid_amount_lte_amount",
    )
    .await;
}

async fn expect_check(pool: &sqlx::PgPool, sql: &str, needle: &str) {
    let mut tx = pool.begin().await.expect("begin");
    let err = sqlx::query(sql).execute(&mut *tx).await.expect_err(sql);
    let message = err.to_string();
    assert!(message.contains(needle), "{sql}\n{message}");
    tx.rollback().await.expect("rollback");
}

#[tokio::test]
async fn lost_bill_without_price_asks_for_an_amount_unless_a_replacement_rule_exists() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    sqlx::query("DELETE FROM fine_rules WHERE lower(media_type) = 'replacement'")
        .execute(app.state.services.repository.pool())
        .await
        .expect("clear replacement rule");

    let (reader_id, _) = fixtures::create_reader(&app, &admin_token, "noprice").await;
    let (loan_id, _) = checkout_without_price(&app, &admin_token, reader_id).await;
    let (status, body) = app
        .post_json(
            &format!("/api/v1/loans/{loan_id}/lost"),
            &json!({ "bill": true }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert!(
        body["message"]
            .as_str()
            .unwrap_or_default()
            .contains("no default replacement rule"),
        "{body}"
    );

    let (status, rule) = app
        .put_json(
            "/api/v1/fines/rules",
            &json!({
                "mediaType": "replacement",
                "dailyRate": "2.50",
                "graceDays": 0
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{rule}");
    assert_eq!(rule["dailyRate"], "2.50", "{rule}");

    let (reader_id, _) = fixtures::create_reader(&app, &admin_token, "ruleprice").await;
    let (loan_id, _) = checkout_without_price(&app, &admin_token, reader_id).await;
    let (status, billed) = app
        .post_json(
            &format!("/api/v1/loans/{loan_id}/lost"),
            &json!({ "bill": true }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{billed}");
    assert_eq!(billed["charge"]["amount"], "2.50", "{billed}");

    let (reader_id, _) = fixtures::create_reader(&app, &admin_token, "zeroprice").await;
    let (loan_id, _) = checkout_with_price(&app, &admin_token, reader_id, "0.00").await;
    let (status, billed) = app
        .post_json(
            &format!("/api/v1/loans/{loan_id}/lost"),
            &json!({ "bill": true }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{billed}");
    assert_eq!(billed["charge"]["amount"], "2.50", "{billed}");

    let (status, rule) = app
        .put_json(
            "/api/v1/fines/rules",
            &json!({
                "mediaType": "replacement",
                "dailyRate": "0.00",
                "graceDays": 0
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{rule}");
    assert_eq!(rule["dailyRate"], "0.00", "{rule}");

    let (reader_id, _) = fixtures::create_reader(&app, &admin_token, "zerorule").await;
    let (loan_id, _) = checkout_without_price(&app, &admin_token, reader_id).await;
    let (status, body) = app
        .post_json(
            &format!("/api/v1/loans/{loan_id}/lost"),
            &json!({ "bill": true }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert!(
        body["message"]
            .as_str()
            .unwrap_or_default()
            .contains("no default replacement rule"),
        "{body}"
    );

    let (reader_id, _) = fixtures::create_reader(&app, &admin_token, "zeroexplicit").await;
    let (loan_id, _) = checkout_with_price(&app, &admin_token, reader_id, "4.00").await;
    let (status, body) = app
        .post_json(
            &format!("/api/v1/loans/{loan_id}/lost"),
            &json!({ "bill": true, "amount": "0.00" }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(
        body["message"]
            .as_str()
            .unwrap_or_default()
            .contains("must be positive"),
        "{body}"
    );

    sqlx::query("DELETE FROM fine_rules WHERE lower(media_type) = 'replacement'")
        .execute(app.state.services.repository.pool())
        .await
        .expect("remove replacement rule");
}

async fn checkout_with_price(
    app: &TestApp,
    admin_token: &str,
    reader_id: i64,
    price: &str,
) -> (i64, i64) {
    let (status, body) = app
        .post_json(
            "/api/v1/biblios",
            &json!({
                "title": format!("Priced {}", fixtures::unique_suffix()),
                "mediaType": "printedText",
                "lang": "french",
                "items": [{
                    "barcode": format!("PR-{}", fixtures::unique_suffix()),
                    "borrowable": true,
                    "price": price
                }]
            }),
            Some(admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let item_id = fixtures::json_id(&body["biblio"]["items"][0]["id"]);
    let (status, loan) = app
        .post_json(
            "/api/v1/loans",
            &json!({
                "userId": reader_id.to_string(),
                "itemId": item_id.to_string()
            }),
            Some(admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{loan}");
    (fixtures::json_id(&loan["id"]), item_id)
}

async fn checkout_without_price(app: &TestApp, admin_token: &str, reader_id: i64) -> (i64, i64) {
    let (status, body) = app
        .post_json(
            "/api/v1/biblios",
            &json!({
                "title": format!("No price {}", fixtures::unique_suffix()),
                "mediaType": "printedText",
                "lang": "french",
                "items": [{
                    "barcode": format!("NP-{}", fixtures::unique_suffix()),
                    "borrowable": true,
                    "priceDeferred": true
                }]
            }),
            Some(admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let item_id = fixtures::json_id(&body["biblio"]["items"][0]["id"]);
    let (status, loan) = app
        .post_json(
            "/api/v1/loans",
            &json!({
                "userId": reader_id.to_string(),
                "itemId": item_id.to_string()
            }),
            Some(admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{loan}");
    (fixtures::json_id(&loan["id"]), item_id)
}
