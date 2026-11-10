//! Reads an omitted `allActionsIncluded` by the spec version the claim declares. The field was
//! optional in C2PA 2.2 and recommended in 2.3 and 2.4, so a claim made under those versions
//! (or declaring none) that leaves it out says nothing about unrecorded actions; the bot reads
//! it as complete. From 2.5 the field is required, so leaving it out is non-conformant and the
//! history stays possibly incomplete. An explicit `false` always means possibly incomplete.
//!
//! This is the bot's one departure from the reference classifier (`classify`), which reads
//! every omission as possibly incomplete. It's applied by filling in the field before
//! classifying, so `classify` stays a verbatim copy.

use serde_json::{json, Value};

use crate::classify::{self, Store, Verdict};

/// The version from which `allActionsIncluded` is required.
const REQUIRED_FROM: (u32, u32) = (2, 5);

/// The spec version a manifest's claim declares: `specVersion` in `claim_generator_info`
/// (2.4 and later) or on the claim itself (2.3).
fn declared(detailed: &Value, label: &str) -> Option<String> {
    let claim = &detailed["manifests"][label]["claim"];
    let info = &claim["claim_generator_info"];
    let info = if info.is_array() { &info[0] } else { info };
    info["specVersion"].as_str().or(claim["specVersion"].as_str()).map(str::to_string)
}

/// `"2.4.0"` → `(2, 4)`.
fn major_minor(version: &str) -> Option<(u32, u32)> {
    let mut parts = version.split('.').map(|p| p.parse().ok());
    Some((parts.next()??, parts.next()??))
}

/// Classifies `json` (with `detailed`) as `classify::classify` does, after reading omitted
/// `allActionsIncluded` fields by each claim's declared spec version.
pub fn classify(json: &Value, detailed: &Value) -> (Verdict, String) {
    let mut json = json.clone();
    // Declared versions of the claims whose omission was allowed, and of those where it wasn't.
    let (mut allowed, mut required) = (Vec::new(), Vec::new());
    if let Some(manifests) = json["manifests"].as_object_mut() {
        for (label, m) in manifests {
            let version = declared(detailed, label);
            let requires = version.as_deref().and_then(major_minor).is_some_and(|v| v >= REQUIRED_FROM);
            for a in m["assertions"].as_array_mut().into_iter().flatten() {
                let omitted = a["label"].as_str().is_some_and(|l| l.starts_with("c2pa.actions.v2"))
                    && a["data"].get("allActionsIncluded").is_none();
                if !omitted {
                    continue;
                }
                let version = version.clone().unwrap_or_else(|| "no spec version".into());
                if requires {
                    required.push(version);
                } else {
                    a["data"]["allActionsIncluded"] = json!(true);
                    allowed.push(version);
                }
            }
        }
    }

    let (verdict, mut why) = classify::classify(Some(Store { json: &json, detailed }));
    // Validation failures decide the verdict before any action is read.
    if !classify::failures(&json["validation_results"]).is_empty() {
        return (verdict, why);
    }
    let list = |v: &mut Vec<String>| {
        v.sort();
        v.dedup();
        v.join(", ")
    };
    if !allowed.is_empty() {
        why.push_str(&format!(
            "; allActionsIncluded omitted where it was optional (before spec 2.5; declares {}), read as complete",
            list(&mut allowed)
        ));
    }
    if !required.is_empty() {
        why.push_str(&format!(
            "; allActionsIncluded omitted though spec {} requires it",
            list(&mut required)
        ));
    }
    (verdict, why)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CAPTURE: &str = "http://cv.iptc.org/newscodes/digitalsourcetype/digitalCapture";

    /// A one-manifest store: a capture with the given actions-assertion extras, declaring
    /// `spec` in `claim_generator_info`.
    fn capture(extra: Value, spec: Option<&str>) -> (Verdict, String) {
        let mut data = json!({ "actions": [{ "action": "c2pa.created", "digitalSourceType": CAPTURE }] });
        data.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        let json = json!({
            "active_manifest": "m",
            "manifests": { "m": { "assertions": [{ "label": "c2pa.actions.v2", "data": data }] } },
        });
        let info = match spec {
            Some(s) => json!({ "name": "test", "specVersion": s }),
            None => json!({ "name": "test" }),
        };
        let detailed = json!({ "manifests": { "m": {
            "claim": { "claim_generator_info": info },
            "assertion_store": { "c2pa.hash.data": {} },
        } } });
        classify(&json, &detailed)
    }

    #[test]
    fn omission_before_2_5_reads_as_complete() {
        for spec in [None, Some("2.2.0"), Some("2.4.0")] {
            let (v, why) = capture(json!({}), spec);
            assert_eq!(v, Verdict::NoAi, "{spec:?}: {why}");
            assert!(why.contains("read as complete"), "{why}");
        }
    }

    #[test]
    fn omission_from_2_5_stays_possibly_incomplete() {
        let (v, why) = capture(json!({}), Some("2.5.0"));
        assert_eq!(v, Verdict::Unknown);
        assert!(why.contains("spec 2.5.0 requires it"), "{why}");
    }

    #[test]
    fn explicit_false_is_always_possibly_incomplete() {
        for spec in [None, Some("2.4.0"), Some("2.5.0")] {
            let (v, why) = capture(json!({ "allActionsIncluded": false }), spec);
            assert_eq!(v, Verdict::Unknown, "{spec:?}: {why}");
            assert!(!why.contains("allActionsIncluded omitted"), "{why}");
        }
    }

    #[test]
    fn versions() {
        assert_eq!(major_minor("2.4.0"), Some((2, 4)));
        assert_eq!(major_minor("2.10"), Some((2, 10)));
        assert_eq!(major_minor("two"), None);
    }
}
