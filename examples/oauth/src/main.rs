use axum::{
    Json, Router,
    extract::Query,
    response::{Html, IntoResponse},
    routing::get,
};
use lineapi::{
    LineOptions,
    line_login::{Scope, get_v2_profile, oauth_url, post_oauth2_v2_1_token},
};
use std::collections::HashMap;
use tower_cookies::{Cookie, CookieManagerLayer, Cookies};
use uuid::Uuid;

pub const PKCE_VERIFIER: &str = "pkce_verifier";
pub const STATE: &str = "state";

// LINE_CLIENT_ID=xxx LINE_CLIENT_SECRET=xxx LINE_REDIRECT_URI=xxx cargo run

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/oauth-line", get(oauth))
        .route("/", get(root))
        .layer(CookieManagerLayer::new());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:5173").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

fn credentials() -> (String, String, String) {
    (
        std::env::var("LINE_CLIENT_ID").expect("LINE_CLIENT_ID not set"),
        std::env::var("LINE_CLIENT_SECRET").expect("LINE_CLIENT_SECRET not set"),
        std::env::var("LINE_REDIRECT_URI").expect("LINE_REDIRECT_URI not set"),
    )
}

// リクエスト/レスポンスをログ出力する LineOptions を組み立てる。
// コールバックが受け取るログは未マスク(client_secret や access_token を生で含む)なので、
// 出力前に必ず `*_redacted()` でマスクする(`Debug` も同様にマスクして出力される)。
// method() / path() でどのエンドポイントを叩いたかが分かる。query() は生のクエリ文字列
// (GET verify の access_token 等の秘匿情報を含み得る)なので query_redacted() を使う。
fn logging_options() -> LineOptions {
    LineOptions::builder()
        .with_on_request(|log| {
            println!(
                "[LINE request] {:?} {:?} query={:?} body={}",
                log.method(),
                log.path(),
                log.query_redacted(),
                log.body_redacted(),
            );
        })
        .with_on_response(|_req, res| {
            println!(
                "[LINE response] status={} body={}",
                res.status_code(),
                res.body_redacted()
            );
        })
        .build()
}

async fn root(cookies: Cookies) -> impl IntoResponse {
    let (client_id, _client_secret, redirect_uri) = credentials();
    let code_verifier = Uuid::new_v4().to_string() + "1234567"; // UUIDは36文字なので7文字追加して長さを調整
    let state = Uuid::new_v4().to_string();
    let url = oauth_url(
        client_id,
        redirect_uri,
        vec![Scope::Profile],
        &state,
        Some(&code_verifier),
    )
    .unwrap();
    cookies.add(Cookie::new(PKCE_VERIFIER, code_verifier));
    cookies.add(Cookie::new(STATE, state));
    Html(format!("<a href='{url}'>oauth<a>")).into_response()
}

async fn oauth(
    Query(params): Query<HashMap<String, String>>,
    cookies: Cookies,
) -> impl IntoResponse {
    let (client_id, client_secret, redirect_uri) = credentials();
    let code_verifier = cookies.get(PKCE_VERIFIER).unwrap();
    let state = cookies.get(STATE).unwrap();
    let code = params.get("code").unwrap();
    let params_state = params.get("state").unwrap();
    if params_state != state.value() {
        return Html("State mismatch").into_response();
    }
    let request_body = post_oauth2_v2_1_token::RequestBody::AuthorizationCode {
        code: code.to_string(),
        redirect_uri,
        client_id,
        client_secret,
        code_verifier: Some(code_verifier.value().to_string()),
    };
    let res = post_oauth2_v2_1_token::execute(&request_body, &logging_options())
        .await
        .unwrap();
    let res = get_v2_profile::execute(&res.0.access_token, &logging_options())
        .await
        .unwrap();
    Json(res.0).into_response()
}
