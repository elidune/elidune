//! `expectedReturnDate` on public copies that are on loan (#132).
//!
//! Readers and the OPAC receive a library-local calendar date (`YYYY-MM-DD`)
//! only when `availability` is `onLoan`. The field carries no time and no patron.

mod common;

use axum::http::StatusCode;
use chrono::{DateTime, Datelike, NaiveTime, TimeZone, Utc};
use common::fixtures;
use common::TestApp;
use elidune_server::services::calendar::{default_zone, local_date, local_to_utc, parse_timezone};
use serde_json::Value;

fn copies_of(body: &Value) -> &[Value] {
    if let Some(items) = body.get("items").and_then(Value::as_array) {
        return items;
    }
    body.as_array()
        .unwrap_or_else(|| panic!("no copies in {body}"))
}

fn copy_by_id(body: &Value, item_id: i64) -> &Value {
    copies_of(body)
        .iter()
        .find(|item| fixtures::json_id(&item["id"]) == item_id)
        .unwrap_or_else(|| panic!("copy {item_id} missing from {body}"))
}

fn assert_date_only(value: &Value, expected: &str) {
    let rendered = value
        .as_str()
        .unwrap_or_else(|| panic!("expected YYYY-MM-DD, got {value}"));
    assert_eq!(rendered, expected, "{value}");
    assert_eq!(rendered.len(), 10, "{rendered}");
    assert!(!rendered.contains('T'), "{rendered}");
    assert!(!rendered.contains(':'), "{rendered}");
}

fn assert_no_patron(body: &Value, secrets: &[&str]) {
    let text = body.to_string();
    for needle in secrets {
        assert!(!text.contains(needle), "response leaked {needle:?}: {text}");
    }
    for copy in copies_of(body) {
        for key in [
            "borrower",
            "loanId",
            "holdFor",
            "holdUserId",
            "holdUserName",
            "firstname",
            "lastname",
            "email",
            "login",
            "cardNumber",
            "initials",
            "phone",
        ] {
            assert!(copy.get(key).is_none(), "{key} leaked: {copy}");
        }
    }
}

fn library_zone(zone_name: &str) -> chrono_tz::Tz {
    parse_timezone(zone_name).unwrap_or_else(|_| default_zone())
}

fn library_local_date(zone_name: &str, expiry: DateTime<Utc>) -> String {
    local_date(library_zone(zone_name), expiry).to_string()
}

fn at_local_noon(zone: chrono_tz::Tz, date: chrono::NaiveDate) -> DateTime<Utc> {
    local_to_utc(zone, date, NaiveTime::from_hms_opt(12, 0, 0).expect("noon")).expect("local noon")
}

fn public_copy_paths(biblio_id: i64, item_id: i64, barcode: &str) -> Vec<String> {
    vec![
        format!("/api/v1/opac/biblios/{biblio_id}"),
        format!("/api/v1/biblios/{biblio_id}"),
        format!("/api/v1/biblios/{biblio_id}/items"),
        format!("/api/v1/items/{item_id}"),
        format!("/api/v1/items/barcode/{barcode}"),
    ]
}

fn assert_public_due_date_absent(copy: &Value) {
    assert!(
        copy.get("dueDate").is_none(),
        "public ItemPublicView must omit dueDate: {copy}"
    );
}

#[tokio::test]
async fn reader_and_opac_expected_return_date_is_local_date_only_on_loan() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (borrower_id, reader_token) =
        fixtures::create_reader(&app, &admin_token, "due_reader").await;

    let email = format!("isolde.due.{borrower_id}@secret.test");
    let card = format!("CARD-DUE-{borrower_id}");
    sqlx::query(
        r#"
        UPDATE users
        SET firstname = 'Isolde',
            lastname = 'Moreau',
            email = $2,
            phone = '0611223344',
            barcode = $3
        WHERE id = $1
        "#,
    )
    .bind(borrower_id)
    .bind(&email)
    .bind(&card)
    .execute(app.state.services.repository.pool())
    .await
    .expect("set borrower identity");

    let login: String = sqlx::query_scalar("SELECT login FROM users WHERE id = $1")
        .bind(borrower_id)
        .fetch_one(app.state.services.repository.pool())
        .await
        .expect("reader login");
    let secrets = [
        "Isolde",
        "Moreau",
        email.as_str(),
        "0611223344",
        card.as_str(),
        login.as_str(),
    ];

    let suffix = fixtures::unique_suffix();
    let loaned_barcode = format!("DUE-A-{suffix}");
    let free_barcode = format!("DUE-B-{suffix}");
    let on_site_barcode = format!("DUE-C-{suffix}");
    let (status, created) = app
        .post_json(
            "/api/v1/biblios",
            &serde_json::json!({
                "title": format!("Expected return {suffix}"),
                "mediaType": "printedText",
                "lang": "french",
                "items": [
                    { "barcode": loaned_barcode, "borrowable": true },
                    { "barcode": free_barcode, "borrowable": true },
                    { "barcode": on_site_barcode, "borrowable": false }
                ]
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let biblio_id = fixtures::json_id(&created["biblio"]["id"]);
    let id_for = |barcode: &str| -> i64 {
        let item = created["biblio"]["items"]
            .as_array()
            .expect("items")
            .iter()
            .find(|item| item["barcode"] == barcode)
            .unwrap_or_else(|| panic!("barcode {barcode} missing from {created}"));
        fixtures::json_id(&item["id"])
    };
    let loaned_id = id_for(&loaned_barcode);
    let free_id = id_for(&free_barcode);
    let on_site_id = id_for(&on_site_barcode);

    let (status, loan) = app
        .post_json(
            "/api/v1/loans",
            &serde_json::json!({
                "userId": borrower_id.to_string(),
                "itemId": loaned_id.to_string(),
                "force": false
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{loan}");
    let loan_id = fixtures::json_id(&loan["id"]);
    let zone_name_for_due: String =
        sqlx::query_scalar("SELECT timezone FROM library_info WHERE id = 1")
            .fetch_one(app.state.services.repository.pool())
            .await
            .expect("library timezone");
    let today = local_date(library_zone(&zone_name_for_due), Utc::now());
    let anchor = today
        .checked_add_days(chrono::Days::new(30))
        .expect("future day");
    // 23:30 UTC is the next civil day in Europe/Paris, so the local date can
    // differ from the UTC date while staying after today.
    let expiry = Utc
        .with_ymd_and_hms(anchor.year(), anchor.month(), anchor.day(), 23, 30, 0)
        .single()
        .expect("due instant");
    sqlx::query("UPDATE loans SET expiry_at = $2 WHERE id = $1")
        .bind(loan_id)
        .bind(expiry)
        .execute(app.state.services.repository.pool())
        .await
        .expect("set due instant");

    let expected = library_local_date(&zone_name_for_due, expiry);
    let utc_date = expiry.date_naive().to_string();

    let paths = public_copy_paths(biblio_id, loaned_id, &loaned_barcode);
    for path in &paths {
        let (status, body) = if path.contains("/opac/") {
            app.get_json(path).await
        } else {
            app.get_json_with_auth(path, &reader_token).await
        };
        assert_eq!(status, StatusCode::OK, "{path} {body}");
        assert_no_patron(&body, &secrets);
        let loaned = copy_by_id(&body, loaned_id);
        assert_eq!(loaned["availability"], "onLoan", "{path} {loaned}");
        assert_date_only(&loaned["expectedReturnDate"], &expected);
        assert_public_due_date_absent(loaned);
        if expected != utc_date {
            assert_ne!(
                loaned["expectedReturnDate"], utc_date,
                "{path} used the UTC date instead of the library date ({zone_name_for_due})"
            );
        }
        if path.contains("/items/") {
            assert_eq!(copies_of(&body).len(), 1, "{path}");
            assert_eq!(
                fixtures::json_id(&copies_of(&body)[0]["id"]),
                loaned_id,
                "{path}"
            );
            continue;
        }
        let free = copy_by_id(&body, free_id);
        assert_eq!(free["availability"], "available", "{path} {free}");
        assert!(free.get("expectedReturnDate").is_none(), "{path} {free}");
        assert_public_due_date_absent(free);
        let on_site = copy_by_id(&body, on_site_id);
        assert_eq!(on_site["availability"], "onSiteOnly", "{path} {on_site}");
        assert!(
            on_site.get("expectedReturnDate").is_none(),
            "{path} {on_site}"
        );
        assert_public_due_date_absent(on_site);
    }

    let (status, staff) = app
        .get_json_with_auth(&format!("/api/v1/biblios/{biblio_id}"), &admin_token)
        .await;
    assert_eq!(status, StatusCode::OK, "{staff}");
    let staff_loaned = copy_by_id(&staff, loaned_id);
    assert_eq!(staff_loaned["availability"], "onLoan", "{staff_loaned}");
    assert!(
        staff_loaned.get("expectedReturnDate").is_none(),
        "staff view keeps dueDate only: {staff_loaned}"
    );
    assert!(
        staff_loaned["dueDate"]
            .as_str()
            .unwrap_or_default()
            .contains('T'),
        "{staff_loaned}"
    );

    let (status, returned) = app
        .post_empty(
            &format!("/api/v1/loans/{loan_id}/return"),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{returned}");

    let (status, after) = app
        .get_json(&format!("/api/v1/opac/biblios/{biblio_id}"))
        .await;
    assert_eq!(status, StatusCode::OK, "{after}");
    let returned_copy = copy_by_id(&after, loaned_id);
    assert_eq!(
        returned_copy["availability"], "available",
        "{returned_copy}"
    );
    assert!(
        returned_copy.get("expectedReturnDate").is_none(),
        "{returned_copy}"
    );
    assert_no_patron(&after, &secrets);
}

fn assert_staff_due_date_present(copy: &Value) {
    assert!(
        copy.get("dueDate")
            .and_then(Value::as_str)
            .is_some_and(|value| value.contains('T')),
        "staff ItemStaffView keeps dueDate: {copy}"
    );
}

async fn fetch_public_copy(app: &TestApp, reader_token: &str, path: &str, item_id: i64) -> Value {
    let (status, body) = if path.contains("/opac/") {
        app.get_json(path).await
    } else {
        app.get_json_with_auth(path, reader_token).await
    };
    assert_eq!(status, StatusCode::OK, "{path} {body}");
    copy_by_id(&body, item_id).clone()
}

#[tokio::test]
async fn opac_expected_return_date_stops_the_local_day_after_it_is_due() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (borrower_id, reader_token) =
        fixtures::create_reader(&app, &admin_token, "due_boundary").await;

    let suffix = fixtures::unique_suffix();
    let barcode = format!("DUE-Y-{suffix}");
    let (status, created) = app
        .post_json(
            "/api/v1/biblios",
            &serde_json::json!({
                "title": format!("Due boundary {suffix}"),
                "mediaType": "printedText",
                "lang": "french",
                "items": [{ "barcode": barcode, "borrowable": true }]
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let biblio_id = fixtures::json_id(&created["biblio"]["id"]);
    let item_id = fixtures::json_id(&created["biblio"]["items"][0]["id"]);

    let (status, loan) = app
        .post_json(
            "/api/v1/loans",
            &serde_json::json!({
                "userId": borrower_id.to_string(),
                "itemId": item_id.to_string(),
                "force": false
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{loan}");
    let loan_id = fixtures::json_id(&loan["id"]);

    let zone_name: String = sqlx::query_scalar("SELECT timezone FROM library_info WHERE id = 1")
        .fetch_one(app.state.services.repository.pool())
        .await
        .expect("library timezone");
    let zone = library_zone(&zone_name);
    let today = local_date(zone, Utc::now());
    let yesterday = today
        .checked_sub_days(chrono::Days::new(1))
        .expect("yesterday");
    let paths = public_copy_paths(biblio_id, item_id, &barcode);

    sqlx::query("UPDATE loans SET expiry_at = $2 WHERE id = $1")
        .bind(loan_id)
        .bind(at_local_noon(zone, today))
        .execute(app.state.services.repository.pool())
        .await
        .expect("set due today");

    for path in &paths {
        let today_copy = fetch_public_copy(&app, &reader_token, path, item_id).await;
        assert_eq!(today_copy["availability"], "onLoan", "{path} {today_copy}");
        assert_date_only(&today_copy["expectedReturnDate"], &today.to_string());
        assert_public_due_date_absent(&today_copy);
    }
    let (status, staff_today) = app
        .get_json_with_auth(&format!("/api/v1/biblios/{biblio_id}"), &admin_token)
        .await;
    assert_eq!(status, StatusCode::OK, "{staff_today}");
    assert_staff_due_date_present(copy_by_id(&staff_today, item_id));

    sqlx::query("UPDATE loans SET expiry_at = $2 WHERE id = $1")
        .bind(loan_id)
        .bind(at_local_noon(zone, yesterday))
        .execute(app.state.services.repository.pool())
        .await
        .expect("set due yesterday");

    for path in &paths {
        let overdue_copy = fetch_public_copy(&app, &reader_token, path, item_id).await;
        assert_eq!(
            overdue_copy["availability"], "onLoan",
            "{path} {overdue_copy}"
        );
        assert!(
            overdue_copy.get("expectedReturnDate").is_none(),
            "overdue local due date must omit expectedReturnDate: {path} {overdue_copy}"
        );
        assert_public_due_date_absent(&overdue_copy);
    }
    let (status, staff_overdue) = app
        .get_json_with_auth(&format!("/api/v1/biblios/{biblio_id}"), &admin_token)
        .await;
    assert_eq!(status, StatusCode::OK, "{staff_overdue}");
    assert_staff_due_date_present(copy_by_id(&staff_overdue, item_id));

    let (status, mine) = app
        .get_json_with_auth("/api/v1/me/loans?perPage=50", &reader_token)
        .await;
    assert_eq!(status, StatusCode::OK, "{mine}");
    let row = mine["items"]
        .as_array()
        .expect("loans")
        .iter()
        .find(|row| fixtures::json_id(&row["id"]) == loan_id)
        .unwrap_or_else(|| panic!("loan {loan_id} missing from {mine}"));
    assert!(
        row["dueDate"]
            .as_str()
            .is_some_and(|value| value.contains('T')),
        "own overdue loan keeps dueDate: {row}"
    );
    assert_eq!(row["isOverdue"], true, "{row}");
}
