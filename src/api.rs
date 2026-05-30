use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use utoipa::{OpenApi, ToSchema};

use crate::client::{self, DepError};

pub const SERVICE: &str = "srvcs-isodd";
pub const CONCERN: &str = "parity: is the number odd";
pub const DEPENDS_ON: &[&str] = &["srvcs-iseven"];

/// Dependency endpoints, injected as router state so tests can point them at
/// mock services.
#[derive(Clone)]
pub struct Deps {
    pub iseven_url: String,
}

#[derive(Serialize, ToSchema)]
pub struct Info {
    pub service: &'static str,
    pub concern: &'static str,
    pub depends_on: Vec<&'static str>,
}

/// `GET /` — service identity (srvcs service standard).
#[utoipa::path(get, path = "/", responses((status = 200, body = Info)))]
pub async fn index() -> Json<Info> {
    Json(Info {
        service: SERVICE,
        concern: CONCERN,
        depends_on: DEPENDS_ON.to_vec(),
    })
}

#[derive(Deserialize, ToSchema)]
pub struct EvalRequest {
    #[schema(value_type = Object)]
    pub value: Value,
}

#[derive(Serialize, ToSchema)]
pub struct PredicateResponse {
    #[schema(value_type = Object)]
    pub value: Value,
    pub result: bool,
}

fn ok(value: Value, result: bool) -> Response {
    (
        StatusCode::OK,
        Json(json!({ "value": value, "result": result })),
    )
        .into_response()
}

fn degraded(dependency: &str) -> Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(json!({ "error": "dependency unavailable", "dependency": dependency })),
    )
        .into_response()
}

/// Forward a dependency's response verbatim (used to propagate `422` for invalid
/// input, so isodd reports the same rejection iseven did).
fn forward(status: u16, body: Value) -> Response {
    let code = StatusCode::from_u16(status).unwrap_or(StatusCode::BAD_GATEWAY);
    (code, Json(body)).into_response()
}

/// `POST /` — is `value` odd?
///
/// Odd is defined as "not even". Rather than reimplement parity, isodd delegates
/// to `srvcs-iseven` and negates it, keeping a single source of truth for parity.
#[utoipa::path(
    post,
    path = "/",
    request_body = EvalRequest,
    responses(
        (status = 200, body = PredicateResponse),
        (status = 422, description = "value is not a valid integer (forwarded from srvcs-iseven)"),
        (status = 503, description = "a dependency is unavailable")
    )
)]
pub async fn evaluate(State(deps): State<Deps>, Json(req): Json<EvalRequest>) -> Response {
    match client::evaluate_dep(&deps.iseven_url, &req.value).await {
        Err(DepError::Unreachable) => degraded("srvcs-iseven"),
        Ok((200, body)) => {
            let even = body.get("result").and_then(Value::as_bool).unwrap_or(false);
            ok(req.value, !even)
        }
        // Invalid input: forward iseven's rejection unchanged.
        Ok((422, body)) => forward(422, body),
        // iseven is itself degraded or behaving unexpectedly.
        Ok(_) => degraded("srvcs-iseven"),
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(index, evaluate),
    components(schemas(Info, EvalRequest, PredicateResponse))
)]
pub struct ApiDoc;

/// Serve OpenAPI document
pub async fn openapi_json() -> Json<utoipa::openapi::OpenApi> {
    Json(ApiDoc::openapi())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openapi_documents_routes() {
        let doc = ApiDoc::openapi();
        let root = doc.paths.paths.get("/").expect("path / present");
        assert!(root.get.is_some());
        assert!(root.post.is_some());
    }

    #[tokio::test]
    async fn index_reports_identity_and_dependency() {
        let Json(info) = index().await;
        assert_eq!(info.service, "srvcs-isodd");
        assert_eq!(info.depends_on, vec!["srvcs-iseven"]);
    }
}
