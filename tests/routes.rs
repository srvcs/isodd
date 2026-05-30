use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::routing::post;
use axum::{Json, Router as AxumRouter};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use srvcs_isodd::{api::Deps, health, router, telemetry};
use tower::ServiceExt;

/// Mock dependency answering `POST /` with a fixed status + body.
async fn spawn_mock(status: StatusCode, body: Value) -> String {
    let app = AxumRouter::new().route(
        "/",
        post(move || {
            let body = body.clone();
            async move { (status, Json(body)) }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    format!("http://{addr}")
}

fn app(iseven_url: &str) -> axum::Router {
    router(
        telemetry::metrics_handle_for_tests(),
        Deps {
            iseven_url: iseven_url.to_string(),
        },
    )
}

async fn eval(iseven_url: &str, value: Value) -> (StatusCode, Value) {
    let res = app(iseven_url)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/")
                .header("content-type", "application/json")
                .body(Body::from(json!({ "value": value }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

const DEAD_URL: &str = "http://127.0.0.1:1";

async fn status_of(uri: &str) -> StatusCode {
    app(DEAD_URL)
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap()
        .status()
}

#[tokio::test]
async fn index_ok() {
    assert_eq!(status_of("/").await, StatusCode::OK);
}

#[tokio::test]
async fn healthz_ok() {
    assert_eq!(status_of("/healthz").await, StatusCode::OK);
}

#[tokio::test]
async fn readyz_reflects_state() {
    health::set_ready(true);
    assert_eq!(status_of("/readyz").await, StatusCode::OK);
}

#[tokio::test]
async fn openapi_ok() {
    assert_eq!(status_of("/openapi.json").await, StatusCode::OK);
}

#[tokio::test]
async fn odd_is_the_negation_of_even() {
    // iseven says false -> isodd is true.
    let iseven = spawn_mock(StatusCode::OK, json!({ "result": false })).await;
    let (status, body) = eval(&iseven, json!(7)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["result"], true);

    // iseven says true -> isodd is false.
    let iseven = spawn_mock(StatusCode::OK, json!({ "result": true })).await;
    let (_, body) = eval(&iseven, json!(8)).await;
    assert_eq!(body["result"], false);
}

#[tokio::test]
async fn forwards_invalid_input_from_iseven() {
    let iseven = spawn_mock(
        StatusCode::UNPROCESSABLE_ENTITY,
        json!({ "error": "value is not an integer" }),
    )
    .await;
    let (status, _) = eval(&iseven, json!(4.5)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn degrades_when_iseven_is_unreachable() {
    let (status, body) = eval(DEAD_URL, json!(7)).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["dependency"], "srvcs-iseven");
}
