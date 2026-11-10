//! A remote MCP server at `/mcp` (Streamable HTTP, stateless: every POST is one JSON-RPC message
//! answered with plain JSON, and there are no sessions or server-sent events). Two tools, both
//! answering with `api::summary`:
//!
//! - `classify_url`: the server downloads the file (`remote::ASSET` limits).
//! - `classify_file`: the file comes inline as base64, up to `MAX_INLINE`.
//!
//! Like the rest of the public API, nothing is kept.

use axum::{
    body::Bytes,
    extract::State,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use base64::Engine as _;
use serde_json::{json, Value};

use crate::{api, AppState};

/// Newest first; an unknown requested version gets the newest.
const PROTOCOL_VERSIONS: [&str; 3] = ["2025-11-25", "2025-06-18", "2025-03-26"];
const MAX_INLINE: usize = 20 * 1024 * 1024;

const INSTRUCTIONS: &str = "Plug-Fest Bot reads a file's C2PA Content Credentials and says what they declare about \
generative AI: ai (everything is AI and the history is complete), includes-ai (some generative AI is declared in the \
current content), no-ai, or unknown (no readable or valid credentials, or the history is unclear or may be incomplete). \
The verdict describes the signed provenance, not an analysis of the pixels: a file without credentials is unknown, not \
no-ai. Prefer classify_url for files online; use classify_file only for small local files. Files are not stored.";

const INVALID_REQUEST: i64 = -32600;
const NOT_FOUND: i64 = -32601;
const PARAMS: i64 = -32602;

pub async fn post(State(app): State<AppState>, body: Bytes) -> Response {
    let msg: Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => return reply(Value::Null, Err((-32700, format!("parse error: {e}")))),
    };
    let id = msg.get("id").cloned();
    let Some(method) = msg["method"].as_str() else {
        // A client's response to a server request (this server never sends any), or junk.
        return match id {
            Some(_) if msg.get("result").is_some() || msg.get("error").is_some() => StatusCode::ACCEPTED.into_response(),
            _ => reply(id.unwrap_or(Value::Null), Err((INVALID_REQUEST, "not a JSON-RPC request (batches aren't supported)".into()))),
        };
    };
    // Notifications (no id) need no answer.
    let Some(id) = id else { return StatusCode::ACCEPTED.into_response() };
    let params = &msg["params"];
    let result = match method {
        "initialize" => Ok(initialize(params)),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": tools() })),
        "tools/call" => call(&app, params).await,
        _ => Err((NOT_FOUND, format!("method not found: {method}"))),
    };
    reply(id, result)
}

/// No server-to-client stream.
pub async fn get() -> Response {
    (StatusCode::METHOD_NOT_ALLOWED, [(header::ALLOW, "POST")]).into_response()
}

fn reply(id: Value, result: Result<Value, (i64, String)>) -> Response {
    Json(match result {
        Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        Err((code, message)) => json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } }),
    })
    .into_response()
}

fn initialize(params: &Value) -> Value {
    let asked = params["protocolVersion"].as_str().unwrap_or_default();
    let version = PROTOCOL_VERSIONS.into_iter().find(|v| *v == asked).unwrap_or(PROTOCOL_VERSIONS[0]);
    json!({
        "protocolVersion": version,
        "capabilities": { "tools": { "listChanged": false } },
        "serverInfo": { "name": "plug-fest-bot", "title": "Plug-Fest Bot", "version": env!("CARGO_PKG_VERSION") },
        "instructions": INSTRUCTIONS,
    })
}

fn tools() -> Value {
    let annotations = json!({ "readOnlyHint": true, "destructiveHint": false, "idempotentHint": true, "openWorldHint": true });
    let output = json!({
        "type": "object",
        "properties": {
            "verdict": { "type": "string", "enum": ["ai", "includes-ai", "no-ai", "unknown"] },
            "reason": { "type": "string", "description": "Why, step by step through the signed history" },
            "fileName": { "type": "string" },
            "size": { "type": "integer" },
            "remoteManifest": { "type": ["string", "null"], "description": "URL of a manifest the file links to, if it was fetched" },
            "manifest": { "type": ["object", "null"], "description": "The active manifest's generator, signer and validation state" },
        },
        "required": ["verdict", "reason"],
    });
    json!([
        {
            "name": "classify_url",
            "title": "Classify a file online",
            "description": "Download a file over HTTPS and say what its C2PA Content Credentials declare about generative AI. \
                            Up to 100 MB; public addresses only, no redirects.",
            "inputSchema": {
                "type": "object",
                "properties": { "url": { "type": "string", "description": "HTTPS URL of the image, video, audio or PDF" } },
                "required": ["url"],
            },
            "outputSchema": output,
            "annotations": annotations,
        },
        {
            "name": "classify_file",
            "title": "Classify a small file",
            "description": "Say what a file's C2PA Content Credentials declare about generative AI, with the file sent \
                            inline as base64. Up to 20 MB; use classify_url for anything online.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "fileName": { "type": "string", "description": "The file's name, with its extension (used to tell the format)" },
                    "contentBase64": { "type": "string", "description": "The file's bytes, base64-encoded" },
                },
                "required": ["fileName", "contentBase64"],
            },
            "outputSchema": output,
            "annotations": annotations,
        },
    ])
}

async fn call(app: &AppState, params: &Value) -> Result<Value, (i64, String)> {
    let args = &params["arguments"];
    let arg = |name: &str| args[name].as_str().ok_or((PARAMS, format!("`{name}` is required")));
    let checked = match params["name"].as_str() {
        Some("classify_url") => api::from_url(app, arg("url")?, false).await,
        Some("classify_file") => {
            let (name, b64) = (arg("fileName")?, arg("contentBase64")?);
            match decode(b64) {
                Ok(bytes) => Ok(api::from_bytes(app, name, "application/octet-stream", bytes.into(), false).await),
                Err(e) => Err(e),
            }
        }
        Some(other) => return Err((PARAMS, format!("unknown tool: {other}"))),
        None => return Err((PARAMS, "`name` is required".into())),
    };
    // Tool failures are results the model can read, not protocol errors.
    Ok(match checked {
        Ok(summary) => json!({
            "content": [{ "type": "text", "text": serde_json::to_string_pretty(&summary).unwrap_or_default() }],
            "structuredContent": summary,
            "isError": false,
        }),
        Err(e) => json!({ "content": [{ "type": "text", "text": e }], "isError": true }),
    })
}

fn decode(b64: &str) -> Result<Vec<u8>, String> {
    let b64: String = b64.chars().filter(|c| !c.is_ascii_whitespace()).collect();
    // Strip a data: URL prefix if one was pasted in.
    let b64 = b64.split_once(";base64,").map_or(b64.as_str(), |(_, rest)| rest);
    if b64.len() / 4 * 3 > MAX_INLINE {
        return Err(format!("file is larger than {} MB; put it online and use classify_url", MAX_INLINE / 1024 / 1024));
    }
    base64::engine::general_purpose::STANDARD.decode(b64).map_err(|e| format!("contentBase64 isn't valid base64: {e}"))
}
