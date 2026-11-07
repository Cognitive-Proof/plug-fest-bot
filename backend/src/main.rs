//! Nom's stomach. `POST /api/eat` takes one multipart `file`, reads its C2PA manifest store with
//! c2pa-rs, and answers with the reference classifier's verdict (ai / includes-ai / no-ai /
//! unknown), its reason, and the store as a `VerificationOutcome` for the React components.
//! Uploads are read in memory and never written to disk.

#[allow(dead_code)] // copied verbatim; the lint-only helpers are unused here
mod classify;
mod outcome;

use std::{io::Cursor, net::SocketAddr, path::Path, sync::Arc};

use anyhow::{Context as _, Result};
use axum::{
    extract::{DefaultBodyLimit, Multipart, State},
    http::StatusCode,
    routing::post,
    Json, Router,
};
use c2pa::{Context, Reader, Settings};
use classify::Store;
use serde_json::{json, Value};
use tower_http::{cors::CorsLayer, services::ServeDir};

const MAX_UPLOAD: usize = 100 * 1024 * 1024;

/// Reader settings: the c2pa-ai-examples test trust anchors, and no network access.
fn settings() -> Result<Settings> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("settings/c2pa-test-settings.json");
    let mut s: Value = serde_json::from_str(&std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?)?;
    // A server shouldn't fetch URLs named by an uploaded file.
    s["verify"]["remote_manifest_fetch"] = json!(false);
    Ok(Settings::new().with_json(&s.to_string())?)
}

fn context() -> Result<Arc<Context>> {
    Ok(Context::new().with_settings(settings()?)?.into_shared())
}

/// The `Reader::json()` and `Reader::detailed_json()` views of `bytes`, or `None` if no
/// manifest store could be read. `format` is a hint; c2pa-rs detects the format from the bytes.
fn views(ctx: &Arc<Context>, format: &str, bytes: &[u8]) -> Option<(Value, Value)> {
    let r = Reader::from_shared_context(ctx).with_stream(format, Cursor::new(bytes)).ok()?;
    Some((serde_json::from_str(&r.json()).ok()?, serde_json::from_str(&r.detailed_json()).ok()?))
}

/// Classifies one file and builds the response body.
fn eat(ctx: &Arc<Context>, name: &str, format: &str, bytes: &[u8]) -> Value {
    let views = views(ctx, format, bytes);
    let (verdict, reason) = classify::classify(views.as_ref().map(|(j, d)| Store { json: j, detailed: d }));
    let outcome = match &views {
        Some((j, d)) => outcome::outcome(j, d),
        None => json!({ "state": false, "manifests": [], "error": "no readable C2PA manifest" }),
    };
    json!({
        "verdict": verdict.dir(),
        "reason": reason,
        "fileName": name,
        "size": bytes.len(),
        "outcome": outcome,
    })
}

/// A format hint from the upload's MIME type, else its file extension.
fn format_hint(content_type: Option<&str>, name: &str) -> String {
    match content_type {
        Some(t) if t != "application/octet-stream" => t.to_string(),
        _ => Path::new(name).extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase(),
    }
}

async fn eat_upload(State(ctx): State<Arc<Context>>, mut form: Multipart) -> Result<Json<Value>, (StatusCode, String)> {
    let bad = |e: &dyn std::fmt::Display| (StatusCode::BAD_REQUEST, e.to_string());
    while let Some(field) = form.next_field().await.map_err(|e| bad(&e))? {
        if field.name() != Some("file") {
            continue;
        }
        let name = field.file_name().unwrap_or("upload").to_string();
        let format = format_hint(field.content_type(), &name);
        let bytes = field.bytes().await.map_err(|e| bad(&e))?;
        let body = tokio::task::spawn_blocking(move || eat(&ctx, &name, &format, &bytes))
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        return Ok(Json(body));
    }
    Err((StatusCode::BAD_REQUEST, "no `file` field in the upload".into()))
}

#[tokio::main]
async fn main() -> Result<()> {
    let dist = Path::new(env!("CARGO_MANIFEST_DIR")).join("../dist");
    let app = Router::new()
        .route("/api/eat", post(eat_upload))
        .layer(DefaultBodyLimit::max(MAX_UPLOAD))
        .layer(CorsLayer::permissive())
        .with_state(context()?)
        // The built frontend, when there is one (`npm run build`).
        .fallback_service(ServeDir::new(dist));

    let port = std::env::var("PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(8080);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    println!("nom-server listening on http://localhost:{port}");
    axum::serve(tokio::net::TcpListener::bind(addr).await?, app).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Removes failure status `code` everywhere in a store, as c2pa-ai-examples does for files
    /// with a known c2pa-rs issue.
    fn drop_status(v: &mut Value, code: &str) {
        match v {
            Value::Object(o) => {
                for key in ["failure", "validation_status"] {
                    if let Some(Value::Array(f)) = o.get_mut(key) {
                        f.retain(|x| x["code"] != code);
                    }
                }
                o.values_mut().for_each(|v| drop_status(v, code));
            }
            Value::Array(a) => a.iter_mut().for_each(|v| drop_status(v, code)),
            _ => {}
        }
    }

    /// Every labelled file in c2pa-ai-examples gets its expected verdict through `eat`.
    #[test]
    fn matches_reference_labels() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../c2pa-ai-examples");
        let labels: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(root.join("labels.json")).unwrap()).unwrap();
        let ctx = context().unwrap();
        let mut wrong = Vec::new();
        for l in &labels {
            let file = l["file"].as_str().unwrap();
            let bytes = std::fs::read(root.join(file)).unwrap();
            let got = &eat(&ctx, file, &format_hint(None, file), &bytes)["verdict"];
            let got = match (&l["knownSdkIssue"]["code"], views(&ctx, &format_hint(None, file), &bytes)) {
                (Value::String(code), Some((mut j, d))) => {
                    assert_eq!(got, &l["knownSdkIssue"]["sdkVerdict"], "{file}: SDK verdict");
                    drop_status(&mut j, code);
                    json!(classify::classify(Some(Store { json: &j, detailed: &d })).0.dir())
                }
                _ => got.clone(),
            };
            if got != l["expected"] {
                wrong.push(format!("{file}: got {got}, expected {}", l["expected"]));
            }
        }
        assert!(wrong.is_empty(), "{} of {} wrong:\n{}", wrong.len(), labels.len(), wrong.join("\n"));
    }
}
