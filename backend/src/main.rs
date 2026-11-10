//! Nom's stomach. `POST /api/eat` takes one multipart `file`, reads its C2PA manifest store with
//! c2pa-rs, and answers with the reference classifier's verdict (ai / includes-ai / no-ai /
//! unknown), its reason, and the store as a `VerificationOutcome` for the React components.
//! Uploads are never written to local disk. When recording is configured (see `record`), each
//! upload is kept in Cloud Storage and Firestore, and `POST /api/feedback` records whether
//! people think its verdict was right. A file whose manifest is only linked (a remote manifest)
//! has it fetched with the limits in `remote`.

#[allow(dead_code)] // copied verbatim; the lint-only helpers are unused here
mod classify;
mod outcome;
mod record;
mod remote;
mod spec;

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
use record::{Recorder, Upload, VERDICTS};
use serde::Deserialize;
use serde_json::{json, Value};
use tower_http::{cors::CorsLayer, services::ServeDir};

const MAX_UPLOAD: usize = 100 * 1024 * 1024;

/// Reader settings: the c2pa-ai-examples test trust anchors plus the official C2PA signer and
/// time-stamping trust lists (or the file `C2PA_SETTINGS` names), and no network access of its own.
fn settings() -> Result<Settings> {
    let path = match std::env::var_os("C2PA_SETTINGS") {
        Some(p) => p.into(),
        None => Path::new(env!("CARGO_MANIFEST_DIR")).join("settings/c2pa-test-settings.json"),
    };
    let mut s: Value = serde_json::from_str(&std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?)?;
    // c2pa-rs would fetch any URL a file names. Instead it reports the URL, and `remote::fetch`
    // gets it with limits.
    s["verify"]["remote_manifest_fetch"] = json!(false);
    Ok(Settings::new().with_json(&s.to_string())?)
}

fn context() -> Result<Arc<Context>> {
    Ok(Context::new().with_settings(settings()?)?.into_shared())
}

/// `Reader::json()` and `Reader::detailed_json()` for the classifier, and crJSON for the signals
/// rubric the frontend evaluates.
type Views = (Value, Value, Value);

/// The views of `bytes`, read from the manifest embedded in it, or from `manifest` (a fetched
/// remote manifest) validated against it. `format` is a hint; c2pa-rs detects the format from the bytes.
fn views(ctx: &Arc<Context>, format: &str, bytes: &[u8], manifest: Option<&[u8]>) -> c2pa::Result<Views> {
    let reader = Reader::from_shared_context(ctx);
    let r = match manifest {
        Some(m) => reader.with_manifest_data_and_stream(m, format, Cursor::new(bytes))?,
        None => reader.with_stream(format, Cursor::new(bytes))?,
    };
    let parse = |s: String| serde_json::from_str(&s).map_err(c2pa::Error::JsonError);
    Ok((parse(r.json())?, parse(r.detailed_json())?, r.to_crjson_value()?))
}

/// What reading an upload found: its views, the remote manifest URL if it had one, and why it
/// couldn't be read when that's worth telling the person.
struct Read {
    views: Option<Views>,
    remote: Option<String>,
    problem: Option<String>,
}

async fn read_upload(ctx: Arc<Context>, format: String, bytes: axum::body::Bytes) -> Read {
    let blocking = |manifest: Option<Vec<u8>>| {
        let (ctx, format, bytes) = (ctx.clone(), format.clone(), bytes.clone());
        tokio::task::spawn_blocking(move || views(&ctx, &format, &bytes, manifest.as_deref()))
    };
    let first = blocking(None).await.unwrap_or_else(|e| Err(c2pa::Error::OtherError(e.into())));
    let url = match first {
        Ok(v) => return Read { views: Some(v), remote: None, problem: None },
        Err(c2pa::Error::RemoteManifestUrl(url)) => url,
        Err(_) => return Read { views: None, remote: None, problem: None },
    };
    let problem = match remote::fetch(&url).await {
        Ok(manifest) => match blocking(Some(manifest)).await {
            Ok(Ok(v)) => return Read { views: Some(v), remote: Some(url), problem: None },
            Ok(Err(e)) => format!("remote manifest at {url} couldn't be read: {e}"),
            Err(e) => format!("remote manifest at {url} couldn't be read: {e}"),
        },
        Err(e) => format!("remote manifest at {url} couldn't be fetched: {e:#}"),
    };
    Read { views: None, remote: Some(url), problem: Some(problem) }
}

/// Classifies one file and builds the response body.
fn eat(name: &str, size: usize, views: Option<&Views>) -> Value {
    let (verdict, reason) = match views {
        Some((j, d, _)) => spec::classify(j, d),
        None => classify::classify(None),
    };
    let outcome = match views {
        Some((j, d, _)) => outcome::outcome(j, d),
        None => json!({ "state": false, "manifests": [], "error": "no readable C2PA manifest" }),
    };
    json!({
        "verdict": verdict.dir(),
        "reason": reason,
        "fileName": name,
        "size": size,
        "outcome": outcome,
        "crjson": views.map(|(_, _, c)| c),
    })
}

/// A format hint from the upload's MIME type, else its file extension.
fn format_hint(content_type: Option<&str>, name: &str) -> String {
    match content_type {
        Some(t) if t != "application/octet-stream" => t.to_string(),
        _ => Path::new(name).extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase(),
    }
}

#[derive(Clone)]
struct AppState {
    ctx: Arc<Context>,
    recorder: Option<Arc<Recorder>>,
}

type ApiError = (StatusCode, String);

/// `file` is the upload; `sample` is "true" when it is one of the site's own sample files.
async fn eat_upload(State(app): State<AppState>, mut form: Multipart) -> Result<Json<Value>, ApiError> {
    let bad = |e: &dyn std::fmt::Display| (StatusCode::BAD_REQUEST, e.to_string());
    let (mut file, mut sample) = (None, false);
    while let Some(field) = form.next_field().await.map_err(|e| bad(&e))? {
        match field.name() {
            Some("file") => {
                let name = field.file_name().unwrap_or("upload").to_string();
                let content_type = field.content_type().unwrap_or("application/octet-stream").to_string();
                file = Some((name, content_type, field.bytes().await.map_err(|e| bad(&e))?));
            }
            Some("sample") => sample = field.text().await.map_err(|e| bad(&e))? == "true",
            _ => {}
        }
    }
    let Some((name, content_type, bytes)) = file else {
        return Err((StatusCode::BAD_REQUEST, "no `file` field in the upload".into()));
    };

    let read = read_upload(app.ctx.clone(), format_hint(Some(&content_type), &name), bytes.clone()).await;
    let mut body = eat(&name, bytes.len(), read.views.as_ref());
    body["remoteManifest"] = json!(read.remote);
    if let Some(problem) = &read.problem {
        body["reason"] = json!(problem);
    }

    // Recording is best effort: a storage failure shouldn't cost the person their verdict.
    body["id"] = Value::Null;
    if let Some(recorder) = &app.recorder {
        let upload = Upload {
            name: &name,
            content_type: &content_type,
            bytes: &bytes,
            verdict: body["verdict"].as_str().unwrap_or_default(),
            reason: body["reason"].as_str().unwrap_or_default(),
            remote_manifest: read.remote.as_deref(),
            sample,
        };
        match recorder.record(upload).await {
            Ok(id) => body["id"] = json!(id),
            Err(e) => eprintln!("recording {name}: {e:#}"),
        }
    }
    Ok(Json(body))
}

#[derive(Deserialize)]
struct Feedback {
    id: String,
    correct: bool,
    /// What the verdict should have been; required when `correct` is false.
    expected: Option<String>,
}

async fn feedback(State(app): State<AppState>, Json(f): Json<Feedback>) -> Result<StatusCode, ApiError> {
    let Some(recorder) = &app.recorder else {
        return Err((StatusCode::SERVICE_UNAVAILABLE, "feedback isn't being recorded on this server".into()));
    };
    let expected = match (f.correct, f.expected.as_deref()) {
        (true, _) => None,
        (false, Some(v)) if VERDICTS.contains(&v) => Some(v),
        (false, _) => return Err((StatusCode::BAD_REQUEST, format!("`expected` must be one of {VERDICTS:?}"))),
    };
    let found = recorder.feedback(&f.id, f.correct, expected).await.map_err(|e| {
        eprintln!("feedback for {}: {e:#}", f.id);
        (StatusCode::BAD_GATEWAY, "couldn't save feedback".into())
    })?;
    if !found {
        return Err((StatusCode::NOT_FOUND, "no upload with that id".into()));
    }
    Ok(StatusCode::NO_CONTENT)
}

#[tokio::main]
async fn main() -> Result<()> {
    let dist = Path::new(env!("CARGO_MANIFEST_DIR")).join("../dist");
    let state = AppState { ctx: context()?, recorder: Recorder::from_env().await.map(Arc::new) };
    let app = Router::new()
        .route("/api/eat", post(eat_upload))
        .route("/api/feedback", post(feedback))
        .layer(DefaultBodyLimit::max(MAX_UPLOAD))
        .layer(CorsLayer::permissive())
        .with_state(state)
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

    /// Every labelled file in c2pa-ai-examples gets its expected verdict from the reference
    /// classifier, read the way the bot reads it. (`eat` then reads omitted
    /// `allActionsIncluded` by spec version, see `spec`.)
    #[test]
    fn matches_reference_labels() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../c2pa-ai-examples");
        let labels: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(root.join("labels.json")).unwrap()).unwrap();
        let ctx = context().unwrap();
        let mut wrong = Vec::new();
        for l in &labels {
            let file = l["file"].as_str().unwrap();
            let bytes = std::fs::read(root.join(file)).unwrap();
            // Without fetching: u06's remote manifest was never published, so it's unknown either way.
            let read = views(&ctx, &format_hint(None, file), &bytes, None).ok();
            let got = &json!(classify::classify(read.as_ref().map(|(j, d, _)| Store { json: j, detailed: d })).0.dir());
            let got = match (&l["knownSdkIssue"]["code"], read.clone()) {
                (Value::String(code), Some((mut j, d, _))) => {
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
