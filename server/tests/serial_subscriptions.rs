//! Serial subscriptions: CRUD, serials rights, and persisted prediction horizons.

mod common;

use axum::http::StatusCode;
use common::fixtures;
use common::TestApp;
use serde_json::{json, Value};

fn by_field<'a>(rows: &'a [Value], field: &str, expected: &str) -> &'a Value {
    rows.iter()
        .find(|row| row[field] == expected)
        .unwrap_or_else(|| panic!("missing {field} {expected}"))
}

async fn frequency_id(app: &TestApp, token: &str, description: &str) -> String {
    let (status, body) = app
        .get_json_with_auth("/api/v1/serials/frequencies?perPage=50", token)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let rows = body["frequencies"].as_array().expect("frequencies");
    by_field(rows, "description", description)["id"]
        .as_str()
        .expect("frequency id")
        .to_string()
}

async fn pattern_id(app: &TestApp, token: &str, formula: &str) -> String {
    let (status, body) = app
        .get_json_with_auth("/api/v1/serials/numbering-patterns?perPage=50", token)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let rows = body["patterns"].as_array().expect("patterns");
    by_field(rows, "formula", formula)["id"]
        .as_str()
        .expect("pattern id")
        .to_string()
}

fn id_string(body: &Value) -> String {
    if let Some(id) = body.as_str() {
        return id.to_string();
    }
    fixtures::json_id(body).to_string()
}

async fn create_site_id(app: &TestApp, token: &str) -> String {
    let (status, body) = app
        .post_json(
            "/api/v1/sites",
            &json!({ "name": format!("Serials site {}", fixtures::unique_suffix()) }),
            Some(token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    id_string(&body["id"])
}

async fn create_vendor(app: &TestApp, token: &str) -> String {
    let token_bit = fixtures::unique_token();
    let (status, body) = app
        .post_json(
            "/api/v1/acquisitions/vendors",
            &json!({
                "name": format!("Serials vendor {token_bit}"),
                "code": format!("sv{token_bit}")
            }),
            Some(token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    id_string(&body["id"])
}

async fn create_fund(app: &TestApp, token: &str) -> String {
    let token_bit = fixtures::unique_token();
    let (status, body) = app
        .post_json(
            "/api/v1/acquisitions/funds",
            &json!({
                "code": format!("sf{token_bit}"),
                "name": format!("Serials fund {token_bit}"),
                "fiscalYear": 2026,
                "allocatedAmount": "100.00"
            }),
            Some(token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    id_string(&body["id"])
}

async fn create_periodical(app: &TestApp, token: &str, title: &str, issn: &str) -> String {
    let (status, body) = app
        .post_json(
            "/api/v1/biblios",
            &json!({
                "title": title,
                "mediaType": "periodic",
                "lang": "french",
                "series": [{ "name": format!("Series {title}"), "issn": issn }]
            }),
            Some(token),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    id_string(&body["biblio"]["id"])
}

fn subscription_body(
    biblio_id: &str,
    site_id: &str,
    vendor_id: Option<&str>,
    fund_id: Option<&str>,
    frequency_id: &str,
    pattern_id: &str,
) -> Value {
    let mut body = json!({
        "biblioId": biblio_id,
        "siteId": site_id,
        "frequencyId": frequency_id,
        "numberingPatternId": pattern_id,
        "firstIssueDate": "2026-03-01",
        "startDate": "2026-03-01",
        "x": 412,
        "y": 0,
        "z": 0,
        "length": { "kind": "months", "count": 12 },
        "costCents": 4800,
        "issuePriceCents": 350,
        "callNumber": "PER 412",
        "location": "Reading room",
        "publicNote": "On the shelf",
        "internalNote": "Do not bind",
        "notes": "Annual invoice"
    });
    if let Some(vendor_id) = vendor_id {
        body["vendorId"] = json!(vendor_id);
    }
    if let Some(fund_id) = fund_id {
        body["fundId"] = json!(fund_id);
    }
    body
}

fn issue_fingerprint(issue: &Value) -> (u64, String, String, i64, i64, i64, u64) {
    (
        issue["slot"].as_u64().expect("slot"),
        issue["plannedDate"].as_str().expect("date").to_string(),
        issue["displayLabel"].as_str().expect("label").to_string(),
        issue["x"].as_i64().expect("x"),
        issue["y"].as_i64().expect("y"),
        issue["z"].as_i64().expect("z"),
        issue["sequence"].as_u64().expect("sequence"),
    )
}

#[tokio::test]
async fn serial_subscription_crud_search_and_rights() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin = fixtures::ensure_first_setup(&app).await;
    let (_, reader) = fixtures::create_reader(&app, &admin, "subreader").await;

    let (status, _) = app.get_json("/api/v1/serials/subscriptions").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = app
        .get_json_with_auth("/api/v1/serials/subscriptions", &reader)
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let code = format!("ssub{}", fixtures::unique_token());
    let (status, created_type) = app
        .post_json(
            "/api/v1/account-types",
            &json!({
                "code": code,
                "name": format!("Serials read {code}"),
                "serialsRights": "r",
                "settingsRights": "n"
            }),
            Some(&admin),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{created_type}");
    let login = format!("ssub_{}", fixtures::unique_suffix());
    let public_type_id = fixtures::public_type_id_by_name(&app, &admin, "adult").await;
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
            Some(&admin),
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
    let read_token = session["token"].as_str().expect("token").to_string();

    let monthly = frequency_id(&app, &admin, "1/month").await;
    let irregular = frequency_id(&app, &admin, "Irregular").await;
    let pattern = pattern_id(&app, &admin, "n° {X}").await;
    let site_id = create_site_id(&app, &admin).await;
    let other_site = create_site_id(&app, &admin).await;
    let vendor_id = create_vendor(&app, &admin).await;
    let fund_id = create_fund(&app, &admin).await;
    let suffix = fixtures::unique_suffix();
    let title = format!("Revue Horizon {suffix}");
    let issn = format!("977{suffix:08}");
    let issn = &issn[..issn.len().min(30)];
    let biblio_id = create_periodical(&app, &admin, &title, issn).await;

    let (status, printed) = app
        .post_json(
            "/api/v1/biblios",
            &json!({
                "title": format!("Not a periodical {suffix}"),
                "mediaType": "printedText",
                "lang": "french"
            }),
            Some(&admin),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{printed}");
    let printed_id = id_string(&printed["biblio"]["id"]);

    let readable = subscription_body(&biblio_id, &site_id, None, None, &monthly, &pattern);
    let (status, denied) = app
        .post_json("/api/v1/serials/subscriptions", &readable, Some(&reader))
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{denied}");
    let (status, denied) = app
        .post_json(
            "/api/v1/serials/subscriptions",
            &readable,
            Some(&read_token),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{denied}");

    let (status, not_periodical) = app
        .post_json(
            "/api/v1/serials/subscriptions",
            &subscription_body(&printed_id, &site_id, None, None, &monthly, &pattern),
            Some(&admin),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{not_periodical}");

    let (status, irregular_body) = app
        .post_json(
            "/api/v1/serials/subscriptions",
            &subscription_body(&biblio_id, &site_id, None, None, &irregular, &pattern),
            Some(&admin),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{irregular_body}");

    let payload = subscription_body(
        &biblio_id,
        &site_id,
        Some(&vendor_id),
        Some(&fund_id),
        &monthly,
        &pattern,
    );
    let (status, created) = app
        .post_json("/api/v1/serials/subscriptions", &payload, Some(&admin))
        .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    assert!(created["graceDays"].is_null(), "{created}");
    assert_eq!(created["effectiveGraceDays"], 15, "{created}");
    assert_eq!(created["createItems"], true);
    assert_eq!(created["costCents"], 4800);
    assert_eq!(created["issuePriceCents"], 350);
    assert_eq!(created["callNumber"], "PER 412");
    assert_eq!(created["location"], "Reading room");
    assert_eq!(created["publicNote"], "On the shelf");
    assert_eq!(created["internalNote"], "Do not bind");
    assert_eq!(created["status"], "active");
    assert_eq!(created["biblioTitle"], title);
    assert!(
        created["issns"]
            .as_array()
            .expect("issns")
            .iter()
            .any(|value| value == issn),
        "{created}"
    );
    let issues = created["issues"].as_array().expect("first horizon");
    assert!(!issues.is_empty(), "{created}");
    assert!(issues.iter().all(|issue| issue["status"] == "expected"));
    assert!(issues
        .iter()
        .all(|issue| !issue["displayLabel"].as_str().unwrap_or("").is_empty()));
    let id = created["id"].as_str().expect("id");

    let (status, listed_ok) = app
        .get_json_with_auth("/api/v1/serials/subscriptions", &read_token)
        .await;
    assert_eq!(status, StatusCode::OK, "{listed_ok}");

    let (status, by_title) = app
        .get_json_with_auth(&format!("/api/v1/serials/subscriptions?q={suffix}"), &admin)
        .await;
    assert_eq!(status, StatusCode::OK, "{by_title}");
    assert!(by_title["subscriptions"]
        .as_array()
        .expect("rows")
        .iter()
        .any(|row| row["id"] == id));

    let (status, by_issn) = app
        .get_json_with_auth(&format!("/api/v1/serials/subscriptions?q={issn}"), &admin)
        .await;
    assert_eq!(status, StatusCode::OK, "{by_issn}");
    assert!(by_issn["subscriptions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|row| row["id"] == id));

    let (status, by_site) = app
        .get_json_with_auth(
            &format!("/api/v1/serials/subscriptions?siteId={site_id}&status=active"),
            &admin,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{by_site}");
    assert!(by_site["subscriptions"]
        .as_array()
        .unwrap()
        .iter()
        .all(|row| row["siteId"] == site_id && row["status"] == "active"));

    let (status, by_vendor) = app
        .get_json_with_auth(
            &format!("/api/v1/serials/subscriptions?vendorId={vendor_id}"),
            &admin,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{by_vendor}");
    assert!(by_vendor["subscriptions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|row| row["id"] == id && row["vendorId"] == vendor_id));

    let (status, duplicate) = app
        .post_json("/api/v1/serials/subscriptions", &payload, Some(&admin))
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{duplicate}");

    let (status, other) = app
        .post_json(
            "/api/v1/serials/subscriptions",
            &subscription_body(&biblio_id, &other_site, None, None, &monthly, &pattern),
            Some(&admin),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{other}");
    let other_id = other["id"].as_str().expect("other id");

    let mut updated = created.clone();
    updated["location"] = json!("Closed stacks");
    updated["callNumber"] = json!("PER 412 BIS");
    let (status, saved) = app
        .put_json(
            &format!("/api/v1/serials/subscriptions/{id}"),
            &updated,
            Some(&admin),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert_eq!(saved["location"], "Closed stacks");
    assert_eq!(saved["callNumber"], "PER 412 BIS");
    assert_eq!(saved["issueCount"], created["issueCount"]);

    let mut retarget = saved.clone();
    retarget["frequencyId"] = json!(frequency_id(&app, &admin, "1/week").await);
    let (status, rejected) = app
        .put_json(
            &format!("/api/v1/serials/subscriptions/{id}"),
            &retarget,
            Some(&admin),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{rejected}");

    let mut closed = saved.clone();
    closed["status"] = json!("closed");
    let (status, closed_body) = app
        .put_json(
            &format!("/api/v1/serials/subscriptions/{id}"),
            &closed,
            Some(&admin),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{closed_body}");
    assert_eq!(closed_body["status"], "closed");

    let (status, reopened_slot) = app
        .post_json("/api/v1/serials/subscriptions", &payload, Some(&admin))
        .await;
    assert_eq!(status, StatusCode::CREATED, "{reopened_slot}");

    let (status, _) = app
        .delete_with_auth(&format!("/api/v1/serials/subscriptions/{id}"), &admin)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = app
        .delete_with_auth(&format!("/api/v1/serials/subscriptions/{other_id}"), &admin)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let reopened_id = reopened_slot["id"].as_str().expect("reopened");
    let (status, _) = app
        .delete_with_auth(
            &format!("/api/v1/serials/subscriptions/{reopened_id}"),
            &admin,
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn two_generated_horizons_match_one_prediction_without_duplicates() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin = fixtures::ensure_first_setup(&app).await;
    let monthly = frequency_id(&app, &admin, "1/month").await;
    let pattern = pattern_id(&app, &admin, "n° {X}").await;
    let site_id = create_site_id(&app, &admin).await;
    let suffix = fixtures::unique_suffix();
    let biblio_id = create_periodical(
        &app,
        &admin,
        &format!("Revue deux horizons {suffix}"),
        &format!("{:08}", suffix % 100_000_000),
    )
    .await;

    let (status, preview) = app
        .post_json(
            "/api/v1/serials/predictions",
            &json!({
                "frequencyId": monthly,
                "patternId": pattern,
                "firstIssueDate": "2026-01-31",
                "starting": { "x": 412, "y": 0, "z": 0 },
                "length": { "kind": "months", "count": 24 },
                "recurringSkips": [{ "month": 8 }]
            }),
            Some(&admin),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{preview}");

    let (status, created) = app
        .post_json(
            "/api/v1/serials/subscriptions",
            &json!({
                "biblioId": biblio_id,
                "siteId": site_id,
                "frequencyId": monthly,
                "numberingPatternId": pattern,
                "firstIssueDate": "2026-01-31",
                "x": 412,
                "y": 0,
                "z": 0,
                "length": { "kind": "months", "count": 24 },
                "recurringSkips": [{ "month": 8 }]
            }),
            Some(&admin),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let id = created["id"].as_str().expect("id");
    let first_issues = created["issues"].as_array().expect("first horizon");
    assert!(!first_issues.is_empty(), "{created}");

    let (status, generated) = app
        .post_json(
            &format!("/api/v1/serials/subscriptions/{id}/generate"),
            &json!({}),
            Some(&admin),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{generated}");
    let second_issues = generated["issues"].as_array().expect("second horizon");
    assert!(!second_issues.is_empty(), "{generated}");

    let (status, exhausted) = app
        .post_json(
            &format!("/api/v1/serials/subscriptions/{id}/generate"),
            &json!({}),
            Some(&admin),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{exhausted}");

    let (status, stored) = app
        .get_json_with_auth(
            &format!("/api/v1/serials/subscriptions/{id}/issues"),
            &admin,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{stored}");
    let stored = stored.as_array().expect("issue list");
    let preview_issues = preview["issues"].as_array().expect("preview issues");
    assert_eq!(stored.len(), preview_issues.len(), "{stored:?}");
    assert_eq!(stored.len(), first_issues.len() + second_issues.len());

    let stored_fp: Vec<_> = stored.iter().map(issue_fingerprint).collect();
    let preview_fp: Vec<_> = preview_issues.iter().map(issue_fingerprint).collect();
    assert_eq!(stored_fp, preview_fp);
    let slots: Vec<u64> = stored_fp.iter().map(|issue| issue.0).collect();
    let unique = slots.iter().collect::<std::collections::HashSet<_>>();
    assert_eq!(unique.len(), slots.len(), "duplicate slots: {slots:?}");

    let first_slots: std::collections::HashSet<u64> = first_issues
        .iter()
        .map(|issue| issue["slot"].as_u64().unwrap())
        .collect();
    assert!(second_issues
        .iter()
        .all(|issue| { !first_slots.contains(&issue["slot"].as_u64().unwrap()) }));
    assert_eq!(
        generated["subscription"]["nextSlot"],
        preview["nextState"]["nextSlot"]
    );
    assert_eq!(generated["subscription"]["x"], preview["nextState"]["x"]);

    let (status, _) = app
        .delete_with_auth(&format!("/api/v1/serials/subscriptions/{id}"), &admin)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn supplement_with_null_slot_and_sequence_does_not_shift_predicted_issues() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin = fixtures::ensure_first_setup(&app).await;
    let monthly = frequency_id(&app, &admin, "1/month").await;
    let pattern = pattern_id(&app, &admin, "n° {X}").await;
    let site_id = create_site_id(&app, &admin).await;
    let suffix = fixtures::unique_suffix();
    let biblio_id = create_periodical(
        &app,
        &admin,
        &format!("Revue hors-série {suffix}"),
        &format!("{:08}", suffix % 100_000_000),
    )
    .await;

    let (status, preview) = app
        .post_json(
            "/api/v1/serials/predictions",
            &json!({
                "frequencyId": monthly,
                "patternId": pattern,
                "firstIssueDate": "2026-01-31",
                "starting": { "x": 10, "y": 0, "z": 0 },
                "length": { "kind": "months", "count": 24 },
                "recurringSkips": [{ "month": 8 }]
            }),
            Some(&admin),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{preview}");

    let (status, created) = app
        .post_json(
            "/api/v1/serials/subscriptions",
            &json!({
                "biblioId": biblio_id,
                "siteId": site_id,
                "frequencyId": monthly,
                "numberingPatternId": pattern,
                "firstIssueDate": "2026-01-31",
                "x": 10,
                "y": 0,
                "z": 0,
                "length": { "kind": "months", "count": 24 },
                "recurringSkips": [{ "month": 8 }]
            }),
            Some(&admin),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let id = created["id"].as_str().expect("id");
    let subscription_id: i64 = id.parse().expect("subscription id");
    let next_slot_before = created["nextSlot"].clone();
    let first_fp: Vec<_> = created["issues"]
        .as_array()
        .expect("first horizon")
        .iter()
        .map(issue_fingerprint)
        .collect();

    let pool = app.state.services.repository.pool();
    let supplement_id = (suffix % 1_000_000_000_000) as i64;
    sqlx::query(
        "INSERT INTO serial_issues ( \
            id, serial_subscription_id, planned_date, display_label, sequence, slot, \
            value_x, value_y, value_z, status, supplement \
         ) VALUES ($1, $2, DATE '2026-07-01', 'hors-série', NULL, NULL, 0, 0, 0, 'expected', true)",
    )
    .bind(supplement_id)
    .bind(subscription_id)
    .execute(pool)
    .await
    .expect("insert supplement");

    let regular_without_slot = sqlx::query(
        "INSERT INTO serial_issues ( \
            id, serial_subscription_id, planned_date, display_label, sequence, slot, \
            value_x, value_y, value_z, status, supplement \
         ) VALUES ($1, $2, DATE '2026-07-02', 'not a supplement', 1000001, NULL, 0, 0, 0, 'expected', false)",
    )
    .bind(supplement_id + 1)
    .bind(subscription_id)
    .execute(pool)
    .await;
    assert!(
        regular_without_slot.is_err(),
        "a regular issue needs a publication slot"
    );
    let regular_without_sequence = sqlx::query(
        "INSERT INTO serial_issues ( \
            id, serial_subscription_id, planned_date, display_label, sequence, slot, \
            value_x, value_y, value_z, status, supplement \
         ) VALUES ($1, $2, DATE '2026-07-02', 'not a supplement', NULL, 999999, 0, 0, 0, 'expected', false)",
    )
    .bind(supplement_id + 2)
    .bind(subscription_id)
    .execute(pool)
    .await;
    assert!(
        regular_without_sequence.is_err(),
        "a regular issue needs a sequence"
    );

    let (status, after_insert) = app
        .get_json_with_auth(&format!("/api/v1/serials/subscriptions/{id}"), &admin)
        .await;
    assert_eq!(status, StatusCode::OK, "{after_insert}");
    assert_eq!(after_insert["nextSlot"], next_slot_before);

    let (status, generated) = app
        .post_json(
            &format!("/api/v1/serials/subscriptions/{id}/generate"),
            &json!({}),
            Some(&admin),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "null sequence supplement must not conflict with the next horizon: {generated}"
    );
    assert!(generated["issues"]
        .as_array()
        .expect("new issues")
        .iter()
        .all(|issue| {
            issue["supplement"] == false && issue["slot"].is_u64() && issue["sequence"].is_u64()
        }));

    let (status, stored) = app
        .get_json_with_auth(
            &format!("/api/v1/serials/subscriptions/{id}/issues"),
            &admin,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{stored}");
    let stored = stored.as_array().expect("issues");
    let supplements: Vec<_> = stored
        .iter()
        .filter(|issue| issue["supplement"] == true)
        .collect();
    assert_eq!(supplements.len(), 1);
    assert!(supplements[0]["slot"].is_null(), "{supplements:?}");
    assert!(supplements[0]["sequence"].is_null(), "{supplements:?}");
    assert_eq!(supplements[0]["displayLabel"], "hors-série");

    let regular: Vec<_> = stored
        .iter()
        .filter(|issue| issue["supplement"] == false)
        .collect();
    let regular_fp: Vec<_> = regular.iter().copied().map(issue_fingerprint).collect();
    let preview_fp: Vec<_> = preview["issues"]
        .as_array()
        .expect("preview")
        .iter()
        .map(issue_fingerprint)
        .collect();
    assert_eq!(regular_fp, preview_fp);
    assert_eq!(regular_fp[..first_fp.len()], first_fp[..]);
    let slots: Vec<u64> = regular_fp.iter().map(|issue| issue.0).collect();
    assert_eq!(
        slots.iter().collect::<std::collections::HashSet<_>>().len(),
        slots.len()
    );
    assert_eq!(
        generated["subscription"]["nextSlot"],
        preview["nextState"]["nextSlot"]
    );

    sqlx::query("DELETE FROM serial_issues WHERE id = $1")
        .bind(supplement_id)
        .execute(pool)
        .await
        .expect("remove supplement");
    let (status, _) = app
        .delete_with_auth(&format!("/api/v1/serials/subscriptions/{id}"), &admin)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}
