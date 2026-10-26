//! Serials MVP 1: frequencies, numbering patterns, prediction preview, serials rights.

mod common;

use axum::http::StatusCode;
use common::fixtures;
use common::TestApp;
use serde_json::{json, Value};

fn frequency_by_description<'a>(body: &'a Value, description: &str) -> &'a Value {
    body["frequencies"]
        .as_array()
        .expect("frequencies")
        .iter()
        .find(|row| row["description"] == description)
        .unwrap_or_else(|| panic!("missing frequency {description}: {body}"))
}

#[tokio::test]
async fn seeded_frequencies_and_serials_rights() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (_, reader_token) = fixtures::create_reader(&app, &admin_token, "serialsreader").await;

    let (status, _) = app
        .get_json_with_auth("/api/v1/serials/frequencies", &reader_token)
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, _) = app.get_json("/api/v1/serials/frequencies").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, body) = app
        .get_json_with_auth("/api/v1/serials/frequencies?perPage=50", &admin_token)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let monthly = frequency_by_description(&body, "1/month");
    assert_eq!(monthly["unit"], "month");
    assert_eq!(monthly["issuesPerUnit"], 1);
    assert_eq!(monthly["unitsPerIssue"], 1);
    let irregular = frequency_by_description(&body, "Irregular");
    assert!(irregular["unit"].is_null(), "{irregular}");

    let (status, admin_type) = app
        .get_json_with_auth("/api/v1/account-types/admin", &admin_token)
        .await;
    assert_eq!(status, StatusCode::OK, "{admin_type}");
    assert_eq!(admin_type["serialsRights"], "w");

    let (status, librarian) = app
        .get_json_with_auth("/api/v1/account-types/librarian", &admin_token)
        .await;
    assert_eq!(status, StatusCode::OK, "{librarian}");
    assert_eq!(librarian["serialsRights"], "w");

    let (status, reader) = app
        .get_json_with_auth("/api/v1/account-types/reader", &admin_token)
        .await;
    assert_eq!(status, StatusCode::OK, "{reader}");
    assert_eq!(reader["serialsRights"], "n");

    let (status, patterns) = app
        .get_json_with_auth(
            "/api/v1/serials/numbering-patterns?perPage=50",
            &admin_token,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{patterns}");
    let formulas: Vec<&str> = patterns["patterns"]
        .as_array()
        .expect("patterns")
        .iter()
        .filter_map(|row| row["formula"].as_str())
        .collect();
    assert!(formulas.contains(&"n° {X}"), "{patterns}");
    assert!(formulas.contains(&"vol. {X}, n° {Y}"), "{patterns}");
    assert!(
        formulas.contains(&"n° {X}, {MonthName} {Year}"),
        "{patterns}"
    );
}

#[tokio::test]
async fn read_right_can_preview_but_not_write_patterns() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let code = format!("sread{}", fixtures::unique_token());
    let (status, created) = app
        .post_json(
            "/api/v1/account-types",
            &json!({
                "code": code,
                "name": format!("Serials read {code}"),
                "serialsRights": "r",
                "settingsRights": "n"
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    assert_eq!(created["serialsRights"], "r");

    let login = format!("sread_{}", fixtures::unique_suffix());
    let public_type_id = fixtures::public_type_id_by_name(&app, &admin_token, "adult").await;
    let (status, user) = app
        .post_json(
            "/api/v1/users",
            &json!({
                "login": login,
                "password": "readerpass1234",
                "firstname": "Serials",
                "lastname": "Reader",
                "email": format!("{login}@test.local"),
                "accountType": code,
                "publicType": public_type_id.to_string(),
                "sex": "f",
                "birthdate": "1990-06-01",
                "addrCity": "Paris"
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{user}");
    let user_id = fixtures::json_id(&user["id"]);
    app.state
        .services
        .users
        .set_must_change_password(user_id, false)
        .await
        .expect("clear must_change_password");
    let (status, session) = app
        .post_json(
            "/api/v1/auth/login",
            &json!({ "username": login, "password": "readerpass1234" }),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{session}");
    let token = session["token"].as_str().expect("token");

    let (status, denied) = app
        .post_json(
            "/api/v1/serials/frequencies",
            &json!({
                "description": format!("nope {code}"),
                "unit": "week",
                "issuesPerUnit": 1,
                "unitsPerIssue": 1
            }),
            Some(token),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{denied}");

    let (status, preview) = app
        .post_json(
            "/api/v1/serials/predictions",
            &json!({
                "frequency": { "unit": "month", "issuesPerUnit": 1, "unitsPerIssue": 1 },
                "pattern": {
                    "formula": "n° {X}, {MonthName} {Year}",
                    "locale": "fr",
                    "x": { "add": 1, "every": 1 }
                },
                "firstIssueDate": "2026-03-01",
                "starting": { "x": 412, "y": 1, "z": 1 },
                "length": { "kind": "issues", "count": 1 }
            }),
            Some(token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    assert_eq!(preview["issues"][0]["displayLabel"], "n° 412, mars 2026");
}

#[tokio::test]
async fn frequency_and_pattern_crud_and_prediction() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let suffix = fixtures::unique_suffix();

    let (status, bad) = app
        .post_json(
            "/api/v1/serials/frequencies",
            &json!({
                "description": format!("bad {suffix}"),
                "unit": "week",
                "issuesPerUnit": 0,
                "unitsPerIssue": 1
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{bad}");

    let (status, frequency) = app
        .post_json(
            "/api/v1/serials/frequencies",
            &json!({
                "description": format!("fortnight {suffix}"),
                "unit": "week",
                "issuesPerUnit": 1,
                "unitsPerIssue": 2,
                "displayOrder": 40
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{frequency}");
    let frequency_id = frequency["id"].as_str().expect("id string");

    let (status, updated) = app
        .put_json(
            &format!("/api/v1/serials/frequencies/{frequency_id}"),
            &json!({
                "description": format!("fortnight edited {suffix}"),
                "unit": "week",
                "issuesPerUnit": 1,
                "unitsPerIssue": 2,
                "displayOrder": 41
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    assert_eq!(updated["displayOrder"], 41);

    let (status, pattern) = app
        .post_json(
            "/api/v1/serials/numbering-patterns",
            &json!({
                "label": format!("Number {suffix}"),
                "formula": "n° {X}, {MonthName} {Year}",
                "locale": "fr",
                "x": { "name": "Numéro", "add": 1, "every": 1, "format": "number" }
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{pattern}");
    assert_eq!(pattern["x"]["name"], "Numéro");
    let pattern_id = pattern["id"].as_str().expect("pattern id");

    let (status, broken) = app
        .put_json(
            &format!("/api/v1/serials/numbering-patterns/{pattern_id}"),
            &json!({
                "label": format!("Number {suffix}"),
                "formula": "{X}",
                "x": { "add": 1, "every": 1, "whenMoreThan": 12 }
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{broken}");

    let (status, preview) = app
        .post_json(
            "/api/v1/serials/predictions",
            &json!({
                "frequencyId": frequency_id,
                "patternId": pattern_id,
                "firstIssueDate": "2026-01-05",
                "starting": { "x": 1, "y": 0, "z": 0 },
                "length": { "kind": "issues", "count": 2 }
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    assert_eq!(preview["issues"][0]["plannedDate"], "2026-01-05");
    assert_eq!(preview["issues"][1]["plannedDate"], "2026-01-19");
    assert_eq!(preview["issues"][0]["displayLabel"], "n° 1, janvier 2026");
    assert_eq!(preview["issues"][1]["displayLabel"], "n° 2, janvier 2026");

    let (status, irregular) = app
        .post_json(
            "/api/v1/serials/predictions",
            &json!({
                "frequency": { "issuesPerUnit": 1, "unitsPerIssue": 1 },
                "pattern": { "formula": "{X}", "x": { "add": 1, "every": 1 } },
                "firstIssueDate": "2026-01-05",
                "starting": { "x": 1, "y": 0, "z": 0 },
                "length": { "kind": "issues", "count": 1 }
            }),
            Some(&admin_token),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{irregular}");

    let (status, _) = app
        .delete_with_auth(
            &format!("/api/v1/serials/numbering-patterns/{pattern_id}"),
            &admin_token,
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, _) = app
        .delete_with_auth(
            &format!("/api/v1/serials/frequencies/{frequency_id}"),
            &admin_token,
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, missing) = app
        .get_json_with_auth(
            &format!("/api/v1/serials/frequencies/{frequency_id}"),
            &admin_token,
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{missing}");
}
