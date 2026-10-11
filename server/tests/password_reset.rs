//! Password-reset links are single-use and bound to a nonce.

mod common;

use common::fixtures;
use common::TestApp;
use elidune_server::error::AppError;
use elidune_server::models::secret::plaintext_password;

#[tokio::test]
async fn reset_link_is_single_use_and_a_newer_link_supersedes_the_older() {
    let Some(app) = TestApp::spawn().await else {
        return;
    };
    let admin_token = fixtures::ensure_first_setup(&app).await;
    let (reader_id, _) = fixtures::create_reader(&app, &admin_token, "resetonce").await;
    let user = app
        .state
        .services
        .users
        .get_by_id(reader_id)
        .await
        .expect("reader");
    let login = user.login.expect("login");

    let users = &app.state.services.users;
    let (_, older, _, _) = users
        .request_password_reset(&login)
        .await
        .expect("first reset link");
    let (_, newer, _, _) = users
        .request_password_reset(&login)
        .await
        .expect("second reset link");

    let stale = users
        .reset_password(&older, &plaintext_password("older-link-pass"))
        .await
        .expect_err("older link must be rejected once a newer one exists");
    assert!(
        matches!(stale, AppError::Authentication(_)),
        "stale link: {stale:?}"
    );

    users
        .reset_password(&newer, &plaintext_password("fresh-link-pass"))
        .await
        .expect("newer link resets the password");

    let reused = users
        .reset_password(&newer, &plaintext_password("second-use-pass"))
        .await
        .expect_err("a used link must be rejected");
    assert!(
        matches!(reused, AppError::Authentication(_)),
        "reused link: {reused:?}"
    );
}
