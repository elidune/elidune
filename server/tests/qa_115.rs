//! Extra coverage for #115.
mod common;
use axum::http::StatusCode;
use common::{fixtures, TestApp};
use elidune_server::models::circulation::CirculationStatus as CS;
use serde_json::json;

async fn site(app: &TestApp, t: &str, n: &str) -> i64 {
    let (s, b) = app
        .post_json(
            "/api/v1/sites",
            &json!({"name": format!("{n}-{}", fixtures::unique_suffix())}),
            Some(t),
        )
        .await;
    assert_eq!(s, StatusCode::CREATED, "{b}");
    fixtures::json_id(&b["id"])
}
async fn copy(app: &TestApp, t: &str, title: &str, site: i64) -> (i64, i64) {
    let p = json!({"title": title, "mediaType": "printedText", "lang": "french",
        "items": [{"barcode": format!("QA-{}", fixtures::unique_suffix()), "borrowable": true, "siteId": site.to_string()}]});
    let (s, b) = app.post_json("/api/v1/biblios", &p, Some(t)).await;
    assert_eq!(s, StatusCode::CREATED, "{b}");
    (
        fixtures::json_id(&b["biblio"]["id"]),
        fixtures::json_id(&b["biblio"]["items"][0]["id"]),
    )
}
async fn set_status(app: &TestApp, id: i64, st: i16) {
    sqlx::query("UPDATE items SET circulation_status=$2, borrowable=FALSE WHERE id=$1")
        .bind(id)
        .bind(st)
        .execute(app.state.services.repository.pool())
        .await
        .unwrap();
}
async fn flags(app: &TestApp, id: i64) -> (i16, bool) {
    sqlx::query_as("SELECT circulation_status, borrowable FROM items WHERE id=$1")
        .bind(id)
        .fetch_one(app.state.services.repository.pool())
        .await
        .unwrap()
}
async fn open_loan(app: &TestApp, t: &str, u: i64, i: i64) -> i64 {
    let (s, b) = app
        .post_json(
            "/api/v1/loans",
            &json!({"userId": u.to_string(), "itemId": i.to_string(), "force": false}),
            Some(t),
        )
        .await;
    assert_eq!(s, StatusCode::CREATED, "{b}");
    fixtures::json_id(&b["id"])
}
async fn ret(app: &TestApp, t: &str, l: i64) {
    let (s, b) = app
        .post_empty(&format!("/api/v1/loans/{l}/return"), Some(t))
        .await;
    assert_eq!(s, StatusCode::OK, "{b}");
}

#[tokio::test]
async fn qa_to_search_return_with_remote_hold_requests_transit() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let t = fixtures::ensure_first_setup(&app).await;
    let a = site(&app, &t, "QA-A").await;
    let b = site(&app, &t, "QA-B").await;
    let (bor, _) = fixtures::create_reader(&app, &t, "qatb").await;
    let (pat, pt) = fixtures::create_reader(&app, &t, "qatw").await;
    let (bib, item) = copy(&app, &t, "QA ToSearch Transit", a).await;
    let loan = open_loan(&app, &t, bor, item).await;
    set_status(&app, item, CS::TO_SEARCH).await;
    let (s, h) = app
        .post_json(
            "/api/v1/holds",
            &json!({"userId": pat.to_string(), "biblioId": bib.to_string(), "pickupSiteId": b.to_string()}),
            Some(&pt),
        )
        .await;
    assert_eq!(s, StatusCode::CREATED, "{h}");
    let hid = fixtures::json_id(&h["id"]);
    ret(&app, &t, loan).await;
    assert_eq!(flags(&app, item).await.0, 0);
    let (s, tr) = app
        .get_json_with_auth(&format!("/api/v1/holds/{hid}/transits"), &t)
        .await;
    assert_eq!(s, StatusCode::OK, "{tr}");
    assert_eq!(tr["status"], "requested");
    assert_eq!(fixtures::json_id(&tr["toSiteId"]), b);
}

async fn untouched(st: i16, tag: &str) {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let t = fixtures::ensure_first_setup(&app).await;
    let a = site(&app, &t, tag).await;
    let (bor, _) = fixtures::create_reader(&app, &t, tag).await;
    let (_, item) = copy(&app, &t, tag, a).await;
    let loan = open_loan(&app, &t, bor, item).await;
    set_status(&app, item, st).await;
    ret(&app, &t, loan).await;
    assert_eq!(
        flags(&app, item).await,
        (st, false),
        "{tag} must stay untouched"
    );
}
#[tokio::test]
async fn qa_lost_untouched_on_return() {
    untouched(CS::LOST, "qalost").await
}
#[tokio::test]
async fn qa_damaged_untouched_on_return() {
    untouched(CS::DAMAGED, "qadmg").await
}
