//! The public API, for scripts and the MCP server. Nothing it reads is kept, whatever the
//! recording settings.
//!
//! - `POST /api/v1/classify`: multipart `file`.
//! - `POST /api/v1/classify-url`: JSON `{ "url": "https://…" }`; the server downloads the file
//!   with the limits in `remote::ASSET`.
//!
//! Both answer with `summary`; `?detail=true` adds `outcome` and `crjson` as `/api/eat` sends them.

use axum::{
    body::Bytes,
    extract::{Multipart, Query, State},
    http::StatusCode,
    Json,
};
use reqwest::Url;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{check, remote, ApiError, AppState, Views};

#[derive(Deserialize)]
pub struct Detail {
    #[serde(default)]
    detail: bool,
}

/// The verdict and what signed the file, without the views the website renders.
pub fn summary(body: &Value, views: Option<&Views>, detail: bool) -> Value {
    let mut s = json!({
        "verdict": body["verdict"],
        "reason": body["reason"],
        "fileName": body["fileName"],
        "size": body["size"],
        "remoteManifest": body["remoteManifest"],
        "manifest": null,
    });
    if let Some((j, _, _)) = views {
        let active = &j["manifests"][j["active_manifest"].as_str().unwrap_or_default()];
        let generator = &active["claim_generator_info"][0];
        s["manifest"] = json!({
            "label": j["active_manifest"],
            "claimGenerator": generator["name"],
            "claimGeneratorVersion": generator["version"],
            "signer": active["signature_info"]["common_name"],
            "issuer": active["signature_info"]["issuer"],
            "signedAt": active["signature_info"]["time"],
            "validationState": j["validation_state"],
        });
    }
    if detail {
        s["outcome"] = body["outcome"].clone();
        s["crjson"] = body["crjson"].clone();
    }
    s
}

/// Downloads `url` and classifies it.
pub async fn from_url(app: &AppState, url: &str, detail: bool) -> Result<Value, String> {
    let fetched = remote::fetch(url, &remote::ASSET).await.map_err(|e| format!("couldn't fetch {url}: {e:#}"))?;
    let name = Url::parse(url)
        .ok()
        .and_then(|u| u.path_segments()?.next_back().filter(|s| !s.is_empty()).map(str::to_string))
        .unwrap_or_else(|| "download".into());
    let content_type = fetched.content_type.unwrap_or_else(|| "application/octet-stream".into());
    Ok(from_bytes(app, &name, &content_type, fetched.bytes.into(), detail).await)
}

/// Classifies `bytes` without keeping them.
pub async fn from_bytes(app: &AppState, name: &str, content_type: &str, bytes: Bytes, detail: bool) -> Value {
    let (body, read) = check(app.ctx.clone(), name, content_type, bytes).await;
    summary(&body, read.views.as_ref(), detail)
}

pub async fn classify(State(app): State<AppState>, Query(q): Query<Detail>, mut form: Multipart) -> Result<Json<Value>, ApiError> {
    let bad = |e: &dyn std::fmt::Display| (StatusCode::BAD_REQUEST, e.to_string());
    while let Some(field) = form.next_field().await.map_err(|e| bad(&e))? {
        if field.name() == Some("file") {
            let name = field.file_name().unwrap_or("upload").to_string();
            let content_type = field.content_type().unwrap_or("application/octet-stream").to_string();
            let bytes = field.bytes().await.map_err(|e| bad(&e))?;
            return Ok(Json(from_bytes(&app, &name, &content_type, bytes, q.detail).await));
        }
    }
    Err((StatusCode::BAD_REQUEST, "no `file` field in the upload".into()))
}

#[derive(Deserialize)]
pub struct UrlRequest {
    url: String,
}

pub async fn classify_url(State(app): State<AppState>, Query(q): Query<Detail>, Json(r): Json<UrlRequest>) -> Result<Json<Value>, ApiError> {
    from_url(&app, &r.url, q.detail).await.map(Json).map_err(|e| (StatusCode::BAD_GATEWAY, e))
}
