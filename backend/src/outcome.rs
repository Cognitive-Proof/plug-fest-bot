//! Turns c2pa-rs `Reader` JSON into the `VerificationOutcome` shape that
//! `c2pa-react-component` and `c2pa-react-ai-disclaimer` render.
//!
//! A port of `readStore` and `toOutcome` in c2pa-react-ai-disclaimer/scripts/build-fixtures.mjs,
//! so the frontend sees the same data those libraries are tested against.

use serde_json::{json, Map, Value};

/// `json` and `detailed` are `Reader::json()` and `Reader::detailed_json()` of one store.
pub fn outcome(json: &Value, detailed: &Value) -> Value {
    let Some(manifests) = json["manifests"].as_object() else {
        return json!({ "state": false, "manifests": [], "error": "no manifests" });
    };

    let entries: Vec<(String, Value)> = manifests
        .iter()
        .map(|(label, m)| {
            let raw = &detailed["manifests"][label]["assertion_store"];
            let mut assertions = m["assertions"].as_array().cloned().unwrap_or_default();
            // Reader::json() omits hard-binding assertions, which are what tell a standard
            // manifest from an update manifest. Keep them, labels only.
            if let Some(raw) = raw.as_object() {
                assertions.extend(
                    raw.keys().filter(|k| k.starts_with("c2pa.hash.")).map(|k| json!({ "label": k, "data": {} })),
                );
            }
            let mut ingredients = m["ingredients"].as_array().cloned().unwrap_or_default();
            for ing in &mut ingredients {
                if ing.get("digital_source_type").is_none() {
                    if let Some(dst) = ing["label"].as_str().and_then(|l| raw[l].get("digitalSourceType")) {
                        ing["digital_source_type"] = dst.clone();
                    }
                }
            }
            let mut entry = json!({
                "label": label,
                "claim_version": m["claim_version"],
                "title": m["title"],
                "instanceId": m["instance_id"],
                "claimGenerator": m.get("claim_generator").cloned().unwrap_or(Value::Null),
                "claimGeneratorInfo": m["claim_generator_info"],
                "signatureInfo": m["signature_info"],
                "assertions": assertion_record(&assertions),
                "ingredients": ingredients,
                "credentials": [],
                "thumbnail": null,
            });
            if let Some(r) = m.get("redactions") {
                entry["redactions"] = r.clone();
            }
            (label.clone(), entry)
        })
        .collect();

    let list: Vec<Value> = entries
        .iter()
        .map(|(label, e)| {
            let mut e = e.clone();
            let o = e.as_object_mut().unwrap();
            o.remove("label");
            let mut out = Map::new();
            out.insert("id".into(), json!(label));
            out.extend(o.clone());
            Value::Object(out)
        })
        .collect();

    json!({
        "state": failures(&json["validation_results"]).is_empty(),
        "manifests": list,
        "manifestStore": {
            "activeManifest": json["active_manifest"],
            "manifests": Map::from_iter(entries),
            "validation_status": json["validation_status"],
            "validation_results": json["validation_results"],
            "validation_state": json["validation_state"],
        },
    })
}

/// Assertions as an object keyed by label; a repeated label gets `__<instance>`.
fn assertion_record(assertions: &[Value]) -> Value {
    let mut record = Map::new();
    for a in assertions {
        let Some(label) = a["label"].as_str() else { continue };
        let key = if record.contains_key(label) {
            let instance = a.get("instance").map_or_else(|| record.len().to_string(), |i| i.to_string());
            format!("{label}__{instance}")
        } else {
            label.to_string()
        };
        record.insert(key, a["data"].clone());
    }
    Value::Object(record)
}

/// Active-manifest failures other than an untrusted signer, which says nothing about AI use.
fn failures(results: &Value) -> Vec<&Value> {
    results["activeManifest"]["failure"]
        .as_array()
        .map(|f| f.iter().filter(|f| f["code"] != "signingCredential.untrusted").collect())
        .unwrap_or_default()
}
