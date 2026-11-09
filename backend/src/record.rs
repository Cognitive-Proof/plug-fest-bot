//! Keeps every upload and what people say about its verdict: the file goes to a Cloud Storage
//! bucket and a document in the Firestore `uploads` collection records it. Feedback is added to
//! that document later. Talks to the REST APIs directly with Application Default Credentials
//! (the Cloud Run service account, or `gcloud auth application-default login` locally).

use std::sync::Arc;

use anyhow::{Context as _, Result};
use gcp_auth::TokenProvider;
use serde_json::{json, Map};
use sha2::{Digest, Sha256};

const SCOPE: &str = "https://www.googleapis.com/auth/cloud-platform";
const COLLECTION: &str = "uploads";

pub const VERDICTS: [&str; 4] = ["ai", "includes-ai", "no-ai", "unknown"];

pub struct Recorder {
    bucket: String,
    project: String,
    auth: Arc<dyn TokenProvider>,
    http: reqwest::Client,
}

/// What `eat` learned about one upload.
pub struct Upload<'a> {
    pub name: &'a str,
    pub content_type: &'a str,
    pub bytes: &'a [u8],
    pub verdict: &'a str,
    pub reason: &'a str,
    /// The remote manifest the file links to, if any.
    pub remote_manifest: Option<&'a str>,
    pub sample: bool,
}

impl Recorder {
    /// A recorder when `UPLOAD_BUCKET` and `GOOGLE_CLOUD_PROJECT` are set and credentials are
    /// found; otherwise `None`, and uploads aren't kept.
    pub async fn from_env() -> Option<Self> {
        let (Ok(bucket), Ok(project)) = (std::env::var("UPLOAD_BUCKET"), std::env::var("GOOGLE_CLOUD_PROJECT")) else {
            println!("recording off: set UPLOAD_BUCKET and GOOGLE_CLOUD_PROJECT to keep uploads");
            return None;
        };
        match gcp_auth::provider().await {
            Ok(auth) => {
                println!("recording uploads to gs://{bucket} and Firestore in {project}");
                Some(Self { bucket, project, auth, http: reqwest::Client::new() })
            }
            Err(e) => {
                eprintln!("recording off: no Google credentials ({e})");
                None
            }
        }
    }

    async fn token(&self) -> Result<String> {
        Ok(self.auth.token(&[SCOPE]).await?.as_str().to_string())
    }

    fn document_url(&self, id: &str) -> String {
        format!(
            "https://firestore.googleapis.com/v1/projects/{}/databases/(default)/documents/{COLLECTION}/{id}",
            self.project
        )
    }

    /// Stores the file and creates its Firestore document. Returns the document id.
    pub async fn record(&self, u: Upload<'_>) -> Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        let object = format!("uploads/{id}/{}", safe_name(u.name));
        let token = self.token().await?;

        let res = self
            .http
            .post(format!("https://storage.googleapis.com/upload/storage/v1/b/{}/o", self.bucket))
            .query(&[("uploadType", "media"), ("name", object.as_str())])
            .bearer_auth(&token)
            .header(reqwest::header::CONTENT_TYPE, u.content_type)
            .body(u.bytes.to_vec())
            .send()
            .await?;
        check(res, "storing the upload").await?;

        let mut fields = json!({
            "fileName": { "stringValue": u.name },
            "size": { "integerValue": u.bytes.len().to_string() },
            "contentType": { "stringValue": u.content_type },
            "sha256": { "stringValue": hex(&Sha256::digest(u.bytes)) },
            "uploadedAt": { "timestampValue": now() },
            "storagePath": { "stringValue": format!("gs://{}/{object}", self.bucket) },
            "verdict": { "stringValue": u.verdict },
            "reason": { "stringValue": u.reason },
            "sample": { "booleanValue": u.sample },
        });
        if let Some(url) = u.remote_manifest {
            fields["remoteManifest"] = json!({ "stringValue": url });
        }
        let res = self
            .http
            .post(format!(
                "https://firestore.googleapis.com/v1/projects/{}/databases/(default)/documents/{COLLECTION}",
                self.project
            ))
            .query(&[("documentId", id.as_str())])
            .bearer_auth(&token)
            .json(&json!({ "fields": fields }))
            .send()
            .await?;
        check(res, "creating the Firestore record").await?;
        Ok(id)
    }

    /// Records whether the verdict was right, and if not, what it should have been. `Ok(false)`
    /// means there is no upload with that id.
    pub async fn feedback(&self, id: &str, correct: bool, expected: Option<&str>) -> Result<bool> {
        if uuid::Uuid::parse_str(id).is_err() {
            return Ok(false);
        }
        let mut feedback = Map::new();
        feedback.insert("correct".into(), json!({ "booleanValue": correct }));
        if let Some(expected) = expected {
            feedback.insert("expected".into(), json!({ "stringValue": expected }));
        }
        feedback.insert("submittedAt".into(), json!({ "timestampValue": now() }));

        let res = self
            .http
            .patch(self.document_url(id))
            // Only touch `feedback`, and only on an existing upload.
            .query(&[("updateMask.fieldPaths", "feedback"), ("currentDocument.exists", "true")])
            .bearer_auth(self.token().await?)
            .json(&json!({ "fields": { "feedback": { "mapValue": { "fields": feedback } } } }))
            .send()
            .await?;
        if res.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(false);
        }
        check(res, "saving feedback").await?;
        Ok(true)
    }
}

async fn check(res: reqwest::Response, what: &str) -> Result<()> {
    if res.status().is_success() {
        return Ok(());
    }
    let status = res.status();
    let body = res.text().await.unwrap_or_default();
    Err(anyhow::anyhow!("{status}: {body}")).with_context(|| what.to_string())
}

/// The last path segment of an upload's name, limited to characters safe in an object name.
fn safe_name(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let safe: String = base
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || "._-".contains(c) { c } else { '_' })
        .take(120)
        .collect();
    if safe.trim_matches(['.', '_']).is_empty() { "upload".into() } else { safe }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Micros, true)
}

#[cfg(test)]
mod tests {
    use super::safe_name;

    #[test]
    fn safe_names() {
        assert_eq!(safe_name("photo 1.jpg"), "photo_1.jpg");
        assert_eq!(safe_name("../../etc/passwd"), "passwd");
        assert_eq!(safe_name("C:\\x\\y.png"), "y.png");
        assert_eq!(safe_name(".."), "upload");
        assert_eq!(safe_name("é.jpg"), "_.jpg");
    }
}
