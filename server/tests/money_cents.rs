//! Integer-cent storage for item prices, subscription prices, and fines.

mod common;

use std::path::PathBuf;

use axum::http::StatusCode;
use common::fixtures;
use common::TestApp;
use serde_json::json;
use sqlx::Connection;

fn money_migration_path() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("migrations");
    let mut found = Vec::new();
    for entry in std::fs::read_dir(&dir).expect("migrations directory") {
        let entry = entry.expect("migration entry");
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.ends_with("_money_integer_cents.sql") {
            found.push(entry.path());
        }
    }
    assert_eq!(found.len(), 1, "one cents migration file: {found:?}");
    found.pop().expect("path")
}

fn money_migration_sql() -> String {
    std::fs::read_to_string(money_migration_path()).expect("read cents migration")
}

/// Split on semicolons that are outside `--` comments and dollar quotes.
fn split_sql(sql: &str) -> Vec<String> {
    let mut statements = Vec::new();
    let mut current = String::new();
    let mut chars = sql.chars().peekable();
    let mut dollar: Option<String> = None;
    while let Some(ch) = chars.next() {
        if dollar.is_none() && ch == '-' && chars.peek() == Some(&'-') {
            current.push(ch);
            current.push(chars.next().expect("second dash"));
            for next in chars.by_ref() {
                current.push(next);
                if next == '\n' {
                    break;
                }
            }
            continue;
        }
        if ch == '$' {
            let mut tag = String::from("$");
            while let Some(next) = chars.peek().copied() {
                if next == '$' || next.is_ascii_alphanumeric() || next == '_' {
                    tag.push(next);
                    chars.next();
                    if next == '$' {
                        break;
                    }
                } else {
                    break;
                }
            }
            if tag.ends_with('$') && tag.len() >= 2 {
                if dollar.as_deref() == Some(tag.as_str()) {
                    dollar = None;
                } else if dollar.is_none() {
                    dollar = Some(tag.clone());
                }
                current.push_str(&tag);
                continue;
            }
        }
        if ch == ';' && dollar.is_none() {
            push_statement(&mut statements, &current);
            current.clear();
            continue;
        }
        current.push(ch);
    }
    push_statement(&mut statements, &current);
    statements
}

fn push_statement(statements: &mut Vec<String>, raw: &str) {
    let executable = raw.lines().any(|line| {
        let trimmed = line.trim();
        !trimmed.is_empty() && !trimmed.starts_with("--")
    });
    if executable {
        statements.push(raw.trim().to_string());
    }
}

fn database_url() -> Option<String> {
    if std::env::var("DATABASE_URL").is_err() && std::env::var("CI").is_err() {
        eprintln!("Skipping integration test: DATABASE_URL not set");
        return None;
    }
    Some(std::env::var("DATABASE_URL").expect("DATABASE_URL"))
}

async fn scratch_connection(schema: &str) -> Option<sqlx::PgConnection> {
    let url = database_url()?;
    let mut conn = sqlx::postgres::PgConnection::connect(&url)
        .await
        .expect("connect");
    sqlx::query(&format!("DROP SCHEMA IF EXISTS {schema} CASCADE"))
        .execute(&mut conn)
        .await
        .expect("drop scratch schema");
    sqlx::query(&format!("CREATE SCHEMA {schema}"))
        .execute(&mut conn)
        .await
        .expect("create scratch schema");
    sqlx::query(&format!("SET search_path TO {schema}"))
        .execute(&mut conn)
        .await
        .expect("search_path");
    Some(conn)
}

async fn exec_statements(conn: &mut sqlx::PgConnection, sql: &str) -> Result<(), sqlx::Error> {
    for statement in split_sql(sql) {
        sqlx::query(&statement).execute(&mut *conn).await?;
    }
    Ok(())
}

#[test]
fn migration_sql_is_one_file_found_by_suffix() {
    let sql = money_migration_sql();
    let statements = split_sql(&sql);
    assert!(statements.len() > 5, "{}", statements.len());
    assert!(statements.iter().any(|statement| {
        statement.contains("elidune_migrate_item_price") && statement.contains("RETURN NEXT")
    }));
}

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
async fn migration_converts_euros_and_records_rejects() {
    let Some(mut conn) = scratch_connection("money_cents_fixture").await else {
        return;
    };
    exec_statements(
        &mut conn,
        r#"
        CREATE TABLE items (
            id BIGINT PRIMARY KEY,
            price TEXT,
            borrowable BOOLEAN NOT NULL DEFAULT TRUE
        );
        CREATE TABLE fines (
            id BIGINT PRIMARY KEY,
            amount NUMERIC(12,2) NOT NULL,
            paid_amount NUMERIC(12,2) NOT NULL DEFAULT 0,
            refunded_amount NUMERIC(12,2) NOT NULL DEFAULT 0,
            CONSTRAINT fines_refunded_lte_paid CHECK (refunded_amount <= paid_amount)
        );
        CREATE TABLE fine_rules (
            id INTEGER PRIMARY KEY,
            daily_rate NUMERIC(12,2) NOT NULL,
            max_amount NUMERIC
        );
        CREATE TABLE circulation_settings (
            id BIGINT PRIMARY KEY,
            unpaid_fine_threshold NUMERIC(12,2) NOT NULL
        );
        CREATE TABLE public_types (
            id BIGINT PRIMARY KEY,
            subscription_price INTEGER,
            unpaid_fine_threshold NUMERIC
        );
        INSERT INTO items (id, price) VALUES
            (1, '12,50'),
            (2, '12,50 €'),
            (3, '12.5'),
            (4, '15'),
            (5, '12.500'),
            (6, NULL),
            (7, '   '),
            (8, 'abc'),
            (9, '-1'),
            (10, '12.501'),
            (11, '€12.50'),
            (12, '12.');
        INSERT INTO fines (id, amount, paid_amount, refunded_amount) VALUES
            (1, 12.50, 0, 0),
            (2, 15.00, 15.00, 15.00);
        INSERT INTO fine_rules (id, daily_rate, max_amount) VALUES
            (1, 0.20, 15.00),
            (2, 0.10, 1.234);
        INSERT INTO circulation_settings (id, unpaid_fine_threshold) VALUES (1, 0);
        INSERT INTO public_types (id, subscription_price, unpaid_fine_threshold) VALUES
            (1, 1500, 4.50),
            (2, 0, -1.50);
        "#,
    )
    .await
    .expect("seed old money schema");

    exec_statements(&mut conn, &money_migration_sql())
        .await
        .expect("cents migration");

    let prices: Vec<(i64, Option<i64>, bool)> =
        sqlx::query_as("SELECT id, price, price_deferred FROM items ORDER BY id")
            .fetch_all(&mut conn)
            .await
            .expect("prices");
    assert_eq!(
        prices,
        vec![
            (1, Some(1250), false),
            (2, Some(1250), false),
            (3, Some(1250), false),
            (4, Some(1500), false),
            (5, Some(1250), false),
            (6, None, false),
            (7, None, false),
            (8, None, true),
            (9, None, true),
            (10, None, true),
            (11, None, true),
            (12, None, true),
        ]
    );

    let fine: (i64, i64, i64) =
        sqlx::query_as("SELECT amount, paid_amount, refunded_amount FROM fines WHERE id = 1")
            .fetch_one(&mut conn)
            .await
            .expect("fine");
    assert_eq!(fine, (1250, 0, 0));
    let refunded: (i64, i64, i64) =
        sqlx::query_as("SELECT amount, paid_amount, refunded_amount FROM fines WHERE id = 2")
            .fetch_one(&mut conn)
            .await
            .expect("refunded fine");
    assert_eq!(refunded, (1500, 1500, 1500));

    let rule: (i64, Option<i64>) =
        sqlx::query_as("SELECT daily_rate, max_amount FROM fine_rules WHERE id = 1")
            .fetch_one(&mut conn)
            .await
            .expect("rule");
    assert_eq!(rule, (20, Some(1500)));
    let bad_cap: Option<i64> = sqlx::query_scalar("SELECT max_amount FROM fine_rules WHERE id = 2")
        .fetch_one(&mut conn)
        .await
        .expect("bad cap");
    assert_eq!(bad_cap, None);

    let threshold: i64 =
        sqlx::query_scalar("SELECT unpaid_fine_threshold FROM circulation_settings WHERE id = 1")
            .fetch_one(&mut conn)
            .await
            .expect("threshold");
    assert_eq!(threshold, 0);

    let adult: (i64, Option<i64>) = sqlx::query_as(
        "SELECT subscription_price, unpaid_fine_threshold FROM public_types WHERE id = 1",
    )
    .fetch_one(&mut conn)
    .await
    .expect("adult");
    assert_eq!(adult, (1500, Some(450)));
    let cleared: (i64, Option<i64>) = sqlx::query_as(
        "SELECT subscription_price, unpaid_fine_threshold FROM public_types WHERE id = 2",
    )
    .fetch_one(&mut conn)
    .await
    .expect("cleared threshold");
    assert_eq!(cleared.0, 0);
    assert_eq!(cleared.1, None);

    let rejects: Vec<(String, String, Option<String>)> = sqlx::query_as(
        "SELECT table_name, column_name, raw_value FROM money_migration_rejects ORDER BY id",
    )
    .fetch_all(&mut conn)
    .await
    .expect("rejects");
    assert_eq!(rejects.len(), 8, "{rejects:?}");
    assert_eq!(
        rejects
            .iter()
            .filter(|(table, column, _)| table == "items" && column == "price")
            .count(),
        5
    );

    sqlx::query("DROP SCHEMA money_cents_fixture CASCADE")
        .execute(&mut conn)
        .await
        .expect("cleanup");
}

#[tokio::test]
async fn migration_aborts_when_a_required_amount_is_not_exact_cents() {
    let Some(mut conn) = scratch_connection("money_cents_abort").await else {
        return;
    };
    exec_statements(
        &mut conn,
        r#"
        CREATE TABLE items (id BIGINT PRIMARY KEY, price TEXT);
        CREATE TABLE fines (
            id BIGINT PRIMARY KEY,
            amount NUMERIC NOT NULL,
            paid_amount NUMERIC NOT NULL,
            refunded_amount NUMERIC NOT NULL
        );
        CREATE TABLE fine_rules (
            id INTEGER PRIMARY KEY,
            daily_rate NUMERIC NOT NULL,
            max_amount NUMERIC
        );
        CREATE TABLE circulation_settings (
            id BIGINT PRIMARY KEY,
            unpaid_fine_threshold NUMERIC NOT NULL
        );
        CREATE TABLE public_types (
            id BIGINT PRIMARY KEY,
            subscription_price INTEGER,
            unpaid_fine_threshold NUMERIC
        );
        INSERT INTO fines (id, amount, paid_amount, refunded_amount) VALUES (1, 1.234, 0, 0);
        INSERT INTO fine_rules (id, daily_rate) VALUES (1, 0.20);
        INSERT INTO circulation_settings (id, unpaid_fine_threshold) VALUES (1, 0);
        "#,
    )
    .await
    .expect("seed abort fixture");

    let error = exec_statements(&mut conn, &money_migration_sql())
        .await
        .expect_err("extra decimals must abort");
    let message = error.to_string();
    assert!(message.contains("money migration aborted"), "{message}");

    sqlx::query("DROP SCHEMA money_cents_abort CASCADE")
        .execute(&mut conn)
        .await
        .expect("cleanup");
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

    sqlx::query("DELETE FROM fine_rules WHERE lower(media_type) = 'replacement'")
        .execute(app.state.services.repository.pool())
        .await
        .expect("remove replacement rule");
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
