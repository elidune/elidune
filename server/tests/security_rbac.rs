//! RBAC smoke tests plus loans_rights docs/enforcement lock (#66).

mod common;

use axum::http::StatusCode;
use common::fixtures;
use common::TestApp;
use elidune_server::models::user::{Rights, UserClaims};
use serde_json::json;

const ACCESS_RIGHTS_DOC: &str = include_str!("../../docs/README-api-access-rights.md");
const LOANS_API: &str = include_str!("../src/api/loans.rs");
const BATCH_API: &str = include_str!("../src/api/batch.rs");
const USER_MODEL: &str = include_str!("../src/models/user.rs");

/// Circulation endpoints gated by `loans_rights` (not `holds_rights`).
/// Docs table cell for each path must mention the listed helper/column.
const LOANS_RIGHTS_DOC_ROWS: &[(&str, &str)] = &[
    ("POST /loans", "require_write_loans()"),
    ("POST /loans/:id/return", "require_write_loans()"),
    ("POST /loans/items/:item_id/return", "require_write_loans()"),
    ("POST /loans/items/:item_id/renew", "require_write_loans()"),
    ("POST /loans/batch-return", "require_write_loans()"),
    ("POST /loans/batch-create", "require_write_loans()"),
    ("GET /loans/overdue", "require_read_loans()"),
    ("GET /loans/:id/user", "require_write_loans()"),
    ("GET /loans/claims-returned", "require_write_loans()"),
    ("POST /loans/:id/lost", "require_write_loans()"),
    ("POST /loans/:id/damaged", "require_write_loans()"),
    ("POST /loans/:id/claimed-returned", "require_write_loans()"),
    (
        "POST /loans/:id/claims-returned/resolve",
        "require_write_loans()",
    ),
    ("GET /stats/loans", "require_read_loans()"),
    ("GET /stats/users", "require_read_loans()"),
];

fn markdown_section<'a>(doc: &'a str, heading: &str) -> &'a str {
    let start = doc
        .find(heading)
        .unwrap_or_else(|| panic!("missing heading {heading}"));
    let rest = &doc[start..];
    let body = rest.get(heading.len()..).unwrap_or("");
    let end = body.find("\n## ").unwrap_or(body.len());
    &rest[..heading.len() + end]
}

fn table_cell_for(doc: &str, endpoint: &str) -> String {
    let needle = format!("`{endpoint}`");
    for line in doc.lines() {
        if !line.starts_with('|') {
            continue;
        }
        let cols: Vec<&str> = line
            .split('|')
            .map(str::trim)
            .filter(|c| !c.is_empty())
            .collect();
        if cols.first().copied() == Some(needle.as_str()) {
            return cols.get(1).copied().unwrap_or("").to_string();
        }
    }
    panic!("endpoint `{endpoint}` is not documented in the access-rights matrix");
}

fn helper_fn_src<'a>(src: &'a str, name: &str) -> &'a str {
    let needle = format!("pub fn {name}(");
    let start = src
        .find(&needle)
        .unwrap_or_else(|| panic!("missing helper {name}"));
    let after = &src[start..];
    let rel = after
        .find("\n    pub fn ")
        .or_else(|| after.find("\n    /// "))
        .unwrap_or(after.len());
    &after[..rel]
}

#[test]
fn loans_access_rights_doc_matches_loans_rights_matrix() {
    let loans_section = markdown_section(ACCESS_RIGHTS_DOC, "## Loans and circulation");
    assert!(
        !loans_section.contains("require_write_holds()"),
        "loans docs still gate circulation on holds_rights / require_write_holds()"
    );
    assert!(
        loans_section.contains("loans_rights"),
        "loans section must describe loans_rights"
    );

    let helpers = markdown_section(ACCESS_RIGHTS_DOC, "Helpers on `UserClaims`:");
    let write_loans = table_cell_for(helpers, "require_write_loans()");
    assert!(
        write_loans.contains("loans_rights >= write"),
        "require_write_loans() helper must check loans_rights >= write, got {write_loans:?}"
    );
    let write_holds = table_cell_for(helpers, "require_write_holds()");
    assert!(
        !write_holds.to_lowercase().contains("checkout")
            && !write_holds.to_lowercase().contains("loan batch"),
        "require_write_holds() must not be documented as the loans write gate, got {write_holds:?}"
    );

    for (endpoint, helper) in LOANS_RIGHTS_DOC_ROWS {
        let auth = table_cell_for(ACCESS_RIGHTS_DOC, endpoint);
        assert!(
            auth.contains(helper),
            "{endpoint} docs must mention {helper}, got {auth:?}"
        );
        assert!(
            !auth.contains("require_write_holds()"),
            "{endpoint} docs still use require_write_holds()"
        );
    }

    let renew = table_cell_for(ACCESS_RIGHTS_DOC, "POST /loans/:id/renew");
    assert!(
        renew.contains("loans_rights") || renew.contains("require_write_loans()"),
        "POST /loans/:id/renew must be documented against loans_rights, got {renew:?}"
    );
    assert!(
        !renew.contains("require_write_holds()"),
        "POST /loans/:id/renew docs still use require_write_holds()"
    );

    let user_loans = table_cell_for(ACCESS_RIGHTS_DOC, "GET /users/:id/loans");
    assert!(
        user_loans.contains("loans_rights") || user_loans.contains("require_read_loans()"),
        "GET /users/:id/loans must mention loans_rights for other patrons, got {user_loans:?}"
    );
}

#[test]
fn loans_handlers_enforce_loans_rights_not_holds_rights() {
    assert!(
        !LOANS_API.contains("require_write_holds"),
        "loans.rs must not call require_write_holds(); circulation writes use loans_rights"
    );
    assert!(
        LOANS_API.contains("require_write_loans()"),
        "loans.rs must call require_write_loans()"
    );
    assert!(
        LOANS_API.contains("require_read_loans()"),
        "loans.rs must call require_read_loans()"
    );

    assert!(
        !BATCH_API.contains("require_write_holds"),
        "batch loan endpoints must not call require_write_holds()"
    );
    assert!(
        BATCH_API.contains("require_write_loans()"),
        "batch loan endpoints must call require_write_loans()"
    );

    let write_loans = helper_fn_src(USER_MODEL, "require_write_loans");
    assert!(
        write_loans.contains("loans_rights"),
        "require_write_loans must inspect loans_rights"
    );
    assert!(
        !write_loans.contains("holds_rights"),
        "require_write_loans must not inspect holds_rights"
    );

    let read_loans = helper_fn_src(USER_MODEL, "require_read_loans");
    assert!(
        read_loans.contains("loans_rights"),
        "require_read_loans must inspect loans_rights"
    );
}

#[tokio::test]
async fn admin_can_access_audit_log_reader_cannot() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };

    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (_, reader_token) = fixtures::create_reader(&app, &admin_token, "rbacreader").await;

    let (admin_status, _) = app.get_json_with_auth("/api/v1/audit", &admin_token).await;
    assert_eq!(
        admin_status,
        StatusCode::OK,
        "admin should access audit log"
    );

    let (reader_status, _) = app.get_json_with_auth("/api/v1/audit", &reader_token).await;
    assert_eq!(
        reader_status,
        StatusCode::FORBIDDEN,
        "reader must not access audit log"
    );
}

fn retarget_loan_holds_rights(token: &str, secret: &str, loans: Rights, holds: Rights) -> String {
    let mut claims = UserClaims::from_token(token, secret).expect("parse test token");
    claims.rights.loans_rights = loans;
    claims.rights.holds_rights = holds;
    claims.create_token(secret).expect("mint test token")
}

/// `ensure_first_setup` may fall through to login while `must_change_password` is still set
/// (parallel tests share one database). Clear that flag so the token is full-scope.
async fn full_admin_token(app: &TestApp) -> String {
    let token = fixtures::ensure_first_setup(app).await;
    let secret = app.state.config.users.jwt_secret.as_str();
    let claims = UserClaims::from_token(&token, secret).expect("parse admin token");
    if !claims.is_password_change_scope() {
        return token;
    }
    app.state
        .services
        .users
        .set_must_change_password(claims.user_id, false)
        .await
        .expect("clear must_change_password on test admin");
    let (status, body) = app
        .post_json(
            "/api/v1/auth/login",
            &json!({ "username": "testadmin", "password": "testadmin1234" }),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "admin re-login: {body}");
    body["token"]
        .as_str()
        .expect("token in login response")
        .to_string()
}

#[tokio::test]
async fn loans_writes_follow_loans_rights_not_holds_rights() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };

    let admin_token = full_admin_token(&app).await;
    let secret = app.state.config.users.jwt_secret.as_str();
    let admin = UserClaims::from_token(&admin_token, secret).expect("parse admin token");
    let loan_body = json!({
        "userId": admin.user_id.to_string(),
        "itemIdentification": "NO-SUCH-BARCODE"
    });
    let batch_body = json!({ "barcodes": ["NO-SUCH-BARCODE"] });

    let (admin_overdue, _) = app
        .get_json_with_auth("/api/v1/loans/overdue", &admin_token)
        .await;
    assert_eq!(
        admin_overdue,
        StatusCode::OK,
        "admin with loans_rights write must list overdue loans"
    );

    let below_write = retarget_loan_holds_rights(&admin_token, secret, Rights::Own, Rights::Write);
    let (own_create, _) = app
        .post_json("/api/v1/loans", &loan_body, Some(&below_write))
        .await;
    assert_eq!(
        own_create,
        StatusCode::FORBIDDEN,
        "loans_rights=own must not checkout"
    );
    let (own_overdue, _) = app
        .get_json_with_auth("/api/v1/loans/overdue", &below_write)
        .await;
    assert_eq!(
        own_overdue,
        StatusCode::FORBIDDEN,
        "loans_rights=own must not list overdue loans"
    );

    let loans_only = retarget_loan_holds_rights(&admin_token, secret, Rights::Write, Rights::None);
    let (loans_only_create, loans_only_body) = app
        .post_json("/api/v1/loans", &loan_body, Some(&loans_only))
        .await;
    assert_ne!(
        loans_only_create,
        StatusCode::FORBIDDEN,
        "loans_rights=write must pass checkout even when holds_rights=none: {loans_only_body}"
    );

    let (loans_only_batch, loans_only_batch_body) = app
        .post_json("/api/v1/loans/batch-return", &batch_body, Some(&loans_only))
        .await;
    assert_ne!(
        loans_only_batch,
        StatusCode::FORBIDDEN,
        "loans_rights=write must pass batch return even when holds_rights=none: {loans_only_batch_body}"
    );

    let holds_only = retarget_loan_holds_rights(&admin_token, secret, Rights::None, Rights::Write);
    let (holds_only_create, _) = app
        .post_json("/api/v1/loans", &loan_body, Some(&holds_only))
        .await;
    assert_eq!(
        holds_only_create,
        StatusCode::FORBIDDEN,
        "holds_rights=write must not checkout when loans_rights=none"
    );

    let (holds_only_batch, _) = app
        .post_json("/api/v1/loans/batch-return", &batch_body, Some(&holds_only))
        .await;
    assert_eq!(
        holds_only_batch,
        StatusCode::FORBIDDEN,
        "holds_rights=write must not batch-return when loans_rights=none"
    );

    let (types_status, reader_type) = app
        .get_json_with_auth("/api/v1/account-types/reader", &admin_token)
        .await;
    assert_eq!(types_status, StatusCode::OK);
    assert_eq!(
        reader_type["loansRights"], "o",
        "seeded reader loans_rights must stay own (below read/write)"
    );
    let (admin_type_status, admin_type) = app
        .get_json_with_auth("/api/v1/account-types/admin", &admin_token)
        .await;
    assert_eq!(admin_type_status, StatusCode::OK);
    assert_eq!(
        admin_type["loansRights"], "w",
        "seeded admin loans_rights must be write"
    );
}
