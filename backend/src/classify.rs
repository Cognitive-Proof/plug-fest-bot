//! Copied unchanged (below this header) from c2pa-ai-examples/src/classify.rs at commit ae87c72.
//! Keep it in sync with that file rather than editing it here.
//!
//! Reference classifier: reads a C2PA manifest store (as `Reader::json()` output, plus
//! `Reader::detailed_json()` to tell update manifests apart) and answers "does the signed
//! provenance declare generative AI in the current content?" with one of four verdicts.
//!
//! The four categories are an application policy, not an algorithm the C2PA specification
//! prescribes. The policy is documented in the README; this file is its executable form. It
//! concerns *generative* AI, following the IPTC split between generative and non-generative
//! tools: `computationalCapture` ("non-generative AI") and `digitalCreation` ("non-generative
//! tools") count as no generative AI.
//!
//! Signing-credential trust is deliberately ignored here: every file in this set is signed
//! with a public test certificate, so trust is a property of the reader's trust list, not of
//! the file.

use std::{
    collections::{HashMap, HashSet},
    fmt,
};

use serde_json::{Map, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Ai,
    IncludesAi,
    NoAi,
    Unknown,
}

impl Verdict {
    pub fn dir(self) -> &'static str {
        match self {
            Verdict::Ai => "ai",
            Verdict::IncludesAi => "includes-ai",
            Verdict::NoAi => "no-ai",
            Verdict::Unknown => "unknown",
        }
    }
}

impl fmt::Display for Verdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.dir())
    }
}

pub const IPTC: &str = "http://cv.iptc.org/newscodes/digitalsourcetype/";
pub const C2PA: &str = "http://c2pa.org/digitalsourcetype/";

/// IPTC terms as of the 2025 vocabulary (including the retired `minorHumanEdits`), plus the two
/// C2PA-specific values.
pub fn is_known_source_type(uri: &str) -> bool {
    const IPTC_TERMS: &[&str] = &[
        "digitalCapture", "computationalCapture", "negativeFilm", "positiveFilm", "print",
        "humanEdits", "minorHumanEdits", "compositeCapture", "algorithmicallyEnhanced",
        "dataDrivenMedia", "digitalCreation", "trainedAlgorithmicMedia",
        "compositeWithTrainedAlgorithmicMedia", "algorithmicMedia", "screenCapture",
        "virtualRecording", "composite", "compositeSynthetic",
    ];
    if let Some(term) = uri.strip_prefix(IPTC) {
        IPTC_TERMS.contains(&term)
    } else {
        matches!(uri.strip_prefix(C2PA), Some("empty" | "trainedAlgorithmicData"))
    }
}

/// What a source type says about the content it describes.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Source {
    Ai,
    IncludesAi,
    NoAi,
    /// `empty`: the content comes entirely from the ingredients.
    Empty,
    /// Edits made by people with non-generative tools; says nothing about the origin.
    HumanEdits,
    /// Neither confirms nor rules out generative AI, or is not a recognised term.
    Unclear,
}

fn source_of(uri: &str) -> Source {
    // trainedAlgorithmicData is data produced with a trained model (CSV, pickle): that model
    // may as well be a classifier as a generator, so it doesn't establish generative AI.
    if uri == format!("{C2PA}trainedAlgorithmicData") {
        return Source::Unclear;
    }
    if uri == format!("{C2PA}empty") {
        return Source::Empty;
    }
    match uri.strip_prefix(IPTC) {
        Some("trainedAlgorithmicMedia") => Source::Ai,
        // compositeSynthetic: "at least one of which is Generative AI".
        Some("compositeWithTrainedAlgorithmicMedia" | "compositeSynthetic") => Source::IncludesAi,
        // computationalCapture merges frames with "non-generative AI" at most.
        Some("digitalCapture" | "computationalCapture" | "negativeFilm" | "positiveFilm"
            | "print" | "digitalCreation" | "compositeCapture" | "algorithmicMedia") => Source::NoAi,
        Some("humanEdits" | "minorHumanEdits") => Source::HumanEdits,
        // screenCapture says how a screen was captured, not where its content came from;
        // virtualRecording may be "based on Generative AI and/or captured elements";
        // algorithmicallyEnhanced doesn't say whether the algorithm is generative.
        _ => Source::Unclear,
    }
}

/// How an action from the specification's list (`actions.yaml`) affects the content.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Effect {
    /// Removes, rearranges or repackages existing content, or touches none of it.
    Preserves,
    /// Resamples or reprocesses existing content (resizing, colour, enhancement). It adds no new
    /// material, but the action name doesn't say whether a generative model did the processing,
    /// as in AI upscaling.
    Processes,
    /// May add content of its own, such as drawing, text, a filter or style, or a generic edit.
    Adds,
    /// Not a specification action, or `c2pa.unknown`: what it did can't be known.
    Unknown,
}

fn effect_of(action: &str) -> Effect {
    match action {
        "c2pa.converted" | "c2pa.cropped" | "c2pa.deleted" | "c2pa.edited.metadata"
        | "c2pa.orientation" | "c2pa.published" | "c2pa.redacted" | "c2pa.removed"
        | "c2pa.repackaged" | "c2pa.reviewed" | "c2pa.trimmed" | "c2pa.watermarked"
        | "c2pa.watermarked.bound" | "c2pa.watermarked.unbound" => Effect::Preserves,
        "c2pa.adjustedColor" | "c2pa.changedSpeed" | "c2pa.color_adjustments" | "c2pa.enhanced"
        | "c2pa.mastered" | "c2pa.resized" | "c2pa.resized.proportional" | "c2pa.transcoded" => {
            Effect::Processes
        }
        "c2pa.unknown" => Effect::Unknown,
        a if a.starts_with("c2pa.") || a.starts_with("font.") => Effect::Adds,
        _ => Effect::Unknown,
    }
}

/// Source types that declare any AI involvement, generative or not.
fn declares_ai_use(uri: &str) -> bool {
    matches!(source_of(uri), Source::Ai | Source::IncludesAi)
        || uri == format!("{C2PA}trainedAlgorithmicData")
        || uri == format!("{IPTC}computationalCapture")
        || uri == format!("{IPTC}algorithmicallyEnhanced")
}

fn has_disclosure(m: &Value) -> bool {
    m["assertions"].as_array().into_iter().flatten().any(|a| a["label"] == "c2pa.ai-disclosure")
}

fn short(uri: &str) -> &str {
    uri.rsplit('/').next().unwrap_or(uri)
}

/// Failure codes other than an untrusted signing credential (see module docs).
pub fn failures(results: &Value) -> Vec<String> {
    results["activeManifest"]["failure"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|s| s["code"].as_str())
        .filter(|c| *c != "signingCredential.untrusted")
        .map(str::to_owned)
        .collect()
}

/// The actions assertions of a manifest, as (label, data) pairs.
pub fn actions_assertions(m: &Value) -> Vec<(&str, &Value)> {
    m["assertions"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|a| Some((a["label"].as_str()?, &a["data"])))
        .filter(|(l, _)| *l == "c2pa.actions" || l.starts_with("c2pa.actions."))
        .collect()
}

/// Overlays `layer` onto `out`. `parameters` merge key by key; in a template, the deprecated
/// `templateParameters` is treated as `parameters`.
fn overlay(out: &mut Map<String, Value>, layer: &Value, template: bool) {
    for (k, v) in layer.as_object().into_iter().flatten() {
        let key = if template && k == "templateParameters" { "parameters" } else { k.as_str() };
        match (key, out.get_mut(key)) {
            ("parameters", Some(Value::Object(p))) => {
                p.extend(v.as_object().into_iter().flatten().map(|(k, v)| (k.clone(), v.clone())));
            }
            _ => {
                out.insert(key.to_owned(), v.clone());
            }
        }
    }
}

/// `action` followed by each of its `related` actions merged onto it, recursively.
fn with_related(action: Map<String, Value>, out: &mut Vec<Value>) {
    let mut primary = action;
    let related = primary.remove("related");
    out.push(Value::Object(primary.clone()));
    for r in related.as_ref().and_then(Value::as_array).into_iter().flatten() {
        let mut merged = primary.clone();
        overlay(&mut merged, r, false);
        with_related(merged, out);
    }
}

/// Every action as a consumer must see it: the `*` template first, then same-named templates
/// in order, then the action's own fields; then each `related` action merged onto its primary.
pub fn merged_actions(data: &Value) -> Vec<Value> {
    let templates: Vec<&Value> = data["templates"].as_array().into_iter().flatten().collect();
    let mut out = Vec::new();
    for action in data["actions"].as_array().into_iter().flatten() {
        let name = &action["action"];
        let mut merged = Map::new();
        for t in templates.iter().filter(|t| t["action"] == "*") {
            overlay(&mut merged, t, true);
        }
        for t in templates.iter().filter(|t| &t["action"] == name) {
            overlay(&mut merged, t, true);
        }
        overlay(&mut merged, action, false);
        merged.insert("action".into(), name.clone());
        with_related(merged, &mut out);
    }
    out
}

/// The ingredient assertion labels a (merged) action's `ingredients` parameter references.
fn placed_refs(action: &Value) -> Vec<&str> {
    action["parameters"]["ingredients"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|r| r["url"].as_str()?.rsplit_once("c2pa.assertions/").map(|(_, l)| l))
        .collect()
}

/// (manifest label, ingredient assertion label) of a hashed JUMBF URI.
fn target(url: &str) -> Option<(String, String)> {
    let rest = url.split("/c2pa/").nth(1)?;
    let (manifest, assertion) = rest.split_once("/c2pa.assertions/")?;
    Some((manifest.to_owned(), assertion.to_owned()))
}

/// The `Reader::json()` and `Reader::detailed_json()` views of one manifest store.
#[derive(Clone, Copy)]
pub struct Store<'a> {
    pub json: &'a Value,
    pub detailed: &'a Value,
}

impl Store<'_> {
    /// An update manifest has no hard-binding (`c2pa.hash.*`) assertion.
    fn is_update(&self, label: &str) -> bool {
        self.detailed["manifests"][label]["assertion_store"]
            .as_object()
            .is_some_and(|a| !a.keys().any(|k| k.starts_with("c2pa.hash.")))
    }
}

/// Classifies a manifest store. `None` means no manifest could be read.
pub fn classify(store: Option<Store>) -> (Verdict, String) {
    let Some(store) = store else {
        return (Verdict::Unknown, "no readable C2PA manifest".into());
    };
    let json = store.json;
    let bad = failures(&json["validation_results"]);
    if !bad.is_empty() {
        return (Verdict::Unknown, format!("active manifest failed validation: {}", bad.join(", ")));
    }
    match json["active_manifest"].as_str() {
        Some(label) => manifest(store, label, &HashSet::new()),
        None => (Verdict::Unknown, "no active manifest".into()),
    }
}

/// `declared` is a `digitalSourceType` the action that placed this ingredient gave it.
fn ingredient(
    store: Store,
    ing: &Value,
    deleted: &HashSet<(String, String)>,
    declared: Option<&str>,
) -> (Verdict, String) {
    let title = ing["title"].as_str().unwrap_or("untitled ingredient");
    let bad = failures(&ing["validation_results"]);
    if !bad.is_empty() {
        return (Verdict::Unknown, format!("ingredient '{title}' failed validation: {}", bad.join(", ")));
    }
    if let Some(label) = ing["active_manifest"].as_str() {
        let (v, why) = manifest(store, label, deleted);
        return (v, format!("ingredient '{title}' -> {why}"));
    }
    // An ingredient without a manifest may still declare its own source type, or the action
    // that placed it may.
    match ing["digital_source_type"].as_str().or(declared) {
        Some(uri) => {
            let v = match source_of(uri) {
                Source::Ai => Verdict::Ai,
                Source::IncludesAi => Verdict::IncludesAi,
                Source::NoAi => Verdict::NoAi,
                _ => Verdict::Unknown,
            };
            (v, format!("ingredient '{title}' declares {} (no manifest)", short(uri)))
        }
        None => (Verdict::Unknown, format!("ingredient '{title}' has no manifest or source type")),
    }
}

fn manifest(store: Store, label: &str, deleted: &HashSet<(String, String)>) -> (Verdict, String) {
    let m = &store.json["manifests"][label];
    let ingredients: Vec<&Value> = m["ingredients"].as_array().into_iter().flatten().collect();
    let parent = ingredients.iter().find(|i| i["relationship"] == "parentOf");

    // An update manifest can't change the content, whatever actions it records (opened,
    // published, edited.metadata, redacted), so the content's provenance is the parent's.
    if store.is_update(label) {
        return match parent {
            Some(p) => {
                let (v, why) = ingredient(store, p, deleted, None);
                // It can't change the content, but it can add what was not recorded before.
                if v == Verdict::NoAi && has_disclosure(m) {
                    let why = format!("update manifest adds c2pa.ai-disclosure over {why}");
                    return (Verdict::Unknown, why);
                }
                (v, format!("update manifest over {why}"))
            }
            None => (Verdict::Unknown, "update manifest with no parent ingredient".into()),
        };
    }

    let assertions = actions_assertions(m);
    let actions: Vec<Value> = assertions.iter().flat_map(|(_, d)| merged_actions(d)).collect();
    // Only a v2 assertion saying allActionsIncluded: true rules out unrecorded actions.
    let complete = !assertions.is_empty()
        && assertions.iter().all(|(l, d)| *l == "c2pa.actions.v2" && d["allActionsIncluded"] == true);

    // Components removed by c2pa.deleted, here or in a later manifest, are not current content.
    let mut deleted = deleted.clone();
    for a in actions.iter().filter(|a| a["action"] == "c2pa.deleted") {
        for r in a["parameters"]["ingredients"].as_array().into_iter().flatten() {
            deleted.extend(r["url"].as_str().and_then(target));
        }
    }

    // Source types declared on the c2pa.placed action, by the ingredient assertion they placed.
    let mut placed_as: HashMap<&str, &str> = HashMap::new();
    for a in actions.iter().filter(|a| a["action"] == "c2pa.placed") {
        if let Some(uri) = a["digitalSourceType"].as_str() {
            for l in placed_refs(a) {
                placed_as.insert(l, uri);
            }
        }
    }

    let components: Vec<(Verdict, String)> = ingredients
        .iter()
        .filter(|i| i["relationship"] == "componentOf")
        .filter(|i| {
            let key = (label.to_owned(), i["label"].as_str().unwrap_or_default().to_owned());
            !deleted.contains(&key)
        })
        .map(|i| {
            let declared = i["label"].as_str().and_then(|l| placed_as.get(l).copied());
            ingredient(store, i, &deleted, declared)
        })
        .collect();

    let mut notes = Vec::new();
    let mut ai_signal = false;
    let mut unclear = false;
    // A later action may have added content that isn't AI, so an AI origin is no longer all.
    let mut mixed = false;
    // Processing with no source type: can't rule out a generative model, but it doesn't add
    // material, so it only stands in the way of a no-ai verdict.
    let mut undeclared_processing = false;
    let mut ai_use_declared = false;
    let mut origin = None;
    let mut empty_origin = false;

    for a in &actions {
        let name = a["action"].as_str().unwrap_or_default();
        match (name, a["digitalSourceType"].as_str()) {
            ("c2pa.created", Some(uri)) => {
                notes.push(format!("created as {}", short(uri)));
                origin = Some(match source_of(uri) {
                    Source::Ai => Verdict::Ai,
                    Source::IncludesAi => Verdict::IncludesAi,
                    Source::NoAi => Verdict::NoAi,
                    Source::Empty => {
                        empty_origin = true;
                        Verdict::NoAi
                    }
                    Source::HumanEdits | Source::Unclear => Verdict::Unknown,
                });
            }
            ("c2pa.created", None) => {
                notes.push("created with no digitalSourceType".into());
                origin = Some(Verdict::Unknown);
            }
            ("c2pa.opened", _) => {
                let (v, why) = match parent {
                    Some(p) => ingredient(store, p, &deleted, None),
                    None => (Verdict::Unknown, "opened with no parent ingredient".into()),
                };
                notes.push(format!("opened parent: {why}"));
                origin = Some(v);
            }
            ("c2pa.deleted", _) if a["parameters"]["ingredients"].is_array() => {
                notes.push("c2pa.deleted removes a whole component".into())
            }
            ("c2pa.deleted", _) => notes.push("c2pa.deleted of part of the content".into()),
            // Placed content is judged by its componentOf ingredient below, with any source
            // type declared here filling in for an ingredient that declares none.
            ("c2pa.placed", None) => {}
            // Evidence about a component that a later c2pa.deleted removed no longer applies.
            ("c2pa.placed", Some(_))
                if !placed_refs(a).is_empty()
                    && placed_refs(a).iter().all(|l| deleted.contains(&(label.to_owned(), l.to_string()))) => {}
            ("c2pa.placed", Some(uri)) => {
                notes.push(format!("c2pa.placed as {}", short(uri)));
                match source_of(uri) {
                    Source::Ai | Source::IncludesAi => ai_signal = true,
                    Source::Unclear => unclear = true,
                    Source::NoAi | Source::Empty | Source::HumanEdits => mixed = true,
                }
            }
            (_, Some(uri)) => {
                notes.push(format!("{name} as {}", short(uri)));
                ai_use_declared |= declares_ai_use(uri);
                let adds = matches!(effect_of(name), Effect::Adds | Effect::Unknown);
                match source_of(uri) {
                    // Generative augmentation of AI content keeps it AI...
                    Source::Ai => ai_signal = true,
                    Source::IncludesAi if uri == format!("{IPTC}compositeWithTrainedAlgorithmicMedia") => {
                        ai_signal = true
                    }
                    // ...but compositeSynthetic declares a mix with other elements.
                    Source::IncludesAi => {
                        ai_signal = true;
                        mixed = true;
                    }
                    Source::Unclear => unclear = true,
                    Source::NoAi | Source::Empty | Source::HumanEdits => mixed |= adds,
                }
            }
            (_, None) => match effect_of(name) {
                Effect::Preserves => {}
                Effect::Processes => {
                    notes.push(format!("{name} has no digitalSourceType"));
                    undeclared_processing = true;
                }
                // The spec doesn't require a generative edit to declare a source type, so an
                // edit that may add content and declares none could be either.
                Effect::Adds => {
                    notes.push(format!("{name} may add content and has no digitalSourceType"));
                    unclear = true;
                    mixed = true;
                }
                Effect::Unknown => {
                    notes.push(format!("{name} has unknown effect and no digitalSourceType"));
                    unclear = true;
                }
            },
        }
    }

    let origin = match origin {
        Some(v) => v,
        None => {
            notes.push("no created or opened action".into());
            Verdict::Unknown
        }
    };

    // Redacting an ingredient or actions assertion hides provenance, not content: whatever it
    // recorded can no longer be seen, so it can't count as absent.
    for r in m["redactions"].as_array().into_iter().flatten().filter_map(Value::as_str) {
        let assertion = r.rsplit_once("c2pa.assertions/").map_or("", |(_, a)| a);
        if assertion.starts_with("c2pa.ingredient") || assertion.starts_with("c2pa.actions") {
            notes.push(format!("redacts {r}"));
            unclear = true;
        }
    }

    // The disclosure names AI/ML models but not that they generated content. With a source
    // type that already declares AI use it adds nothing; without one, a model was used in an
    // undeclared way.
    for a in &actions {
        if let Some(uri) = a["digitalSourceType"].as_str() {
            ai_use_declared |= declares_ai_use(uri);
        }
    }
    if has_disclosure(m) {
        if ai_use_declared || matches!(origin, Verdict::Ai | Verdict::IncludesAi) || ai_signal {
            notes.push("c2pa.ai-disclosure present".into());
        } else {
            notes.push("c2pa.ai-disclosure present but no source type declares AI use".into());
            unclear = true;
        }
    }
    for (v, why) in &components {
        notes.push(format!("component: {why}"));
        match v {
            Verdict::Ai | Verdict::IncludesAi => ai_signal = true,
            Verdict::Unknown => unclear = true,
            Verdict::NoAi => {}
        }
    }
    if !complete {
        notes.push("history may be incomplete (allActionsIncluded not true)".into());
    }

    let components_all_ai = components.iter().all(|(v, _)| *v == Verdict::Ai);
    let all_ai_inputs = components_all_ai
        && (origin == Verdict::Ai || (empty_origin && !components.is_empty()));
    if mixed && all_ai_inputs {
        notes.push("a later action may have added non-AI content".into());
    }
    // "Entirely AI" needs the whole history: an unrecorded, unclear or content-adding later
    // change could have added content that isn't AI. Without it, AI is still known to be in the
    // provenance, and nothing shows it was removed.
    let verdict = if all_ai_inputs && complete && !unclear && !mixed {
        Verdict::Ai
    } else if all_ai_inputs || origin == Verdict::Ai || origin == Verdict::IncludesAi || ai_signal {
        Verdict::IncludesAi
    } else if origin == Verdict::Unknown || unclear || undeclared_processing || !complete {
        Verdict::Unknown
    } else {
        Verdict::NoAi
    };
    (verdict, notes.join("; "))
}

/// Cases c2pa-rs can't build as files: its Builder always records `c2pa.opened` in an update
/// manifest. The stores here carry only the fields the classifier reads.
#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    const AI: &str = "http://cv.iptc.org/newscodes/digitalsourcetype/trainedAlgorithmicMedia";
    const CAPTURE: &str = "http://cv.iptc.org/newscodes/digitalsourcetype/digitalCapture";
    const FILL: &str = "http://cv.iptc.org/newscodes/digitalsourcetype/compositeWithTrainedAlgorithmicMedia";

    /// (label, actions, ingredients, is an update manifest)
    type Manifest = (&'static str, Value, Value, bool);

    fn classify_store(manifests: &[Manifest]) -> Verdict {
        let mut json = json!({ "active_manifest": manifests[0].0, "manifests": {} });
        let mut detailed = json!({ "manifests": {} });
        for (label, actions, ingredients, update) in manifests {
            json["manifests"][label] = json!({
                "assertions": [{ "label": "c2pa.actions.v2", "data": actions }],
                "ingredients": ingredients,
            });
            let mut store = json!({ "c2pa.actions.v2": actions });
            if !update {
                store["c2pa.hash.data"] = json!({});
            }
            detailed["manifests"][label] = json!({ "assertion_store": store });
        }
        classify(Some(Store { json: &json, detailed: &detailed })).0
    }

    fn ai_parent() -> Manifest {
        let actions = json!({ "actions": [{ "action": "c2pa.created", "digitalSourceType": AI }], "allActionsIncluded": true });
        ("parent", actions, json!([]), false)
    }

    fn update_over_parent(action: &str) -> Manifest {
        let actions = json!({ "actions": [{ "action": action }] });
        let parent = json!([{ "relationship": "parentOf", "active_manifest": "parent", "title": "parent" }]);
        ("update", actions, parent, true)
    }

    #[test]
    fn published_only_update_manifest_inherits_parent() {
        assert_eq!(classify_store(&[update_over_parent("c2pa.published"), ai_parent()]), Verdict::Ai);
    }

    #[test]
    fn metadata_only_update_manifest_inherits_parent() {
        assert_eq!(classify_store(&[update_over_parent("c2pa.edited.metadata"), ai_parent()]), Verdict::Ai);
    }

    #[test]
    fn standard_manifest_without_inception_action_is_unknown() {
        let (label, actions, ingredients, _) = update_over_parent("c2pa.published");
        assert_eq!(classify_store(&[(label, actions, ingredients, false), ai_parent()]), Verdict::Unknown);
    }

    #[test]
    fn generative_edit_in_nested_related_action() {
        let actions = json!({
            "actions": [
                { "action": "c2pa.created", "digitalSourceType": CAPTURE },
                { "action": "c2pa.edited", "related": [
                    { "action": "c2pa.color_adjustments", "related": [
                        { "action": "c2pa.edited", "digitalSourceType": FILL },
                    ] },
                ] },
            ],
            "allActionsIncluded": true,
        });
        assert_eq!(classify_store(&[("m", actions, json!([]), false)]), Verdict::IncludesAi);
    }

    #[test]
    fn related_action_inherits_primary_fields() {
        // The related action only lists what differs, so it keeps the primary's source type.
        let actions = json!({
            "actions": [
                { "action": "c2pa.created", "digitalSourceType": CAPTURE },
                { "action": "c2pa.edited", "digitalSourceType": FILL, "related": [{ "action": "c2pa.edited", "description": "second pass" }] },
            ],
            "allActionsIncluded": true,
        });
        let merged = merged_actions(&actions);
        assert_eq!(merged.len(), 3);
        assert_eq!(merged[2]["digitalSourceType"], FILL);
        assert!(merged[1].get("related").is_none());
    }

    fn single(actions: Value) -> Verdict {
        classify_store(&[("m", actions, json!([]), false)])
    }

    const HUMAN_EDITS: &str = "http://cv.iptc.org/newscodes/digitalsourcetype/humanEdits";
    const SYNTHETIC: &str = "http://cv.iptc.org/newscodes/digitalsourcetype/compositeSynthetic";

    #[test]
    fn later_edits_decide_whether_ai_origin_stays_entirely_ai() {
        let after_ai = |edit: Value| {
            single(json!({ "actions": [{ "action": "c2pa.created", "digitalSourceType": AI }, edit], "allActionsIncluded": true }))
        };
        // Preserving edits and generative augmentation keep it AI.
        assert_eq!(after_ai(json!({ "action": "c2pa.cropped" })), Verdict::Ai);
        assert_eq!(after_ai(json!({ "action": "c2pa.edited", "digitalSourceType": FILL })), Verdict::Ai);
        // Human drawing, an explicitly mixed edit, or a generic edit may add non-AI content.
        assert_eq!(after_ai(json!({ "action": "c2pa.drawing", "digitalSourceType": HUMAN_EDITS })), Verdict::IncludesAi);
        assert_eq!(after_ai(json!({ "action": "c2pa.edited", "digitalSourceType": SYNTHETIC })), Verdict::IncludesAi);
        assert_eq!(after_ai(json!({ "action": "c2pa.edited" })), Verdict::IncludesAi);
    }

    #[test]
    fn unrecognised_action_keeps_complete_history_uncertain() {
        let actions = json!({
            "actions": [{ "action": "c2pa.created", "digitalSourceType": CAPTURE }, { "action": "com.fabrikam.stylize" }],
            "allActionsIncluded": true,
        });
        assert_eq!(single(actions), Verdict::Unknown);
    }

    #[test]
    fn omitted_all_actions_included_means_possibly_incomplete() {
        // Older v2 assertions could omit the field; consumers read that as "maybe incomplete".
        let actions = json!({ "actions": [{ "action": "c2pa.created", "digitalSourceType": CAPTURE }] });
        assert_eq!(single(actions), Verdict::Unknown);
    }

    #[test]
    fn template_precedence_is_wildcard_then_named_then_action() {
        let actions = json!({
            "actions": [{ "action": "c2pa.created", "description": "own" }],
            "templates": [
                { "action": "c2pa.created", "digitalSourceType": AI, "description": "named" },
                { "action": "*", "digitalSourceType": CAPTURE, "description": "wildcard", "softwareAgent": { "name": "w" } },
            ],
        });
        let merged = &merged_actions(&actions)[0];
        assert_eq!(merged["digitalSourceType"], AI);
        assert_eq!(merged["description"], "own");
        assert_eq!(merged["softwareAgent"]["name"], "w");
    }

    #[test]
    fn edit_without_source_type_is_not_assumed_non_generative() {
        let after_capture = |edit: Value| {
            single(json!({ "actions": [{ "action": "c2pa.created", "digitalSourceType": CAPTURE }, edit], "allActionsIncluded": true }))
        };
        assert_eq!(after_capture(json!({ "action": "c2pa.edited" })), Verdict::Unknown);
        assert_eq!(after_capture(json!({ "action": "c2pa.filtered" })), Verdict::Unknown);
        // An explicit non-generative declaration settles it.
        assert_eq!(after_capture(json!({ "action": "c2pa.edited", "digitalSourceType": HUMAN_EDITS })), Verdict::NoAi);
        assert_eq!(after_capture(json!({ "action": "c2pa.cropped" })), Verdict::NoAi);
    }

    #[test]
    fn undeclared_processing_rules_out_no_ai_only() {
        let after = |origin: &str, edit: Value| {
            single(json!({ "actions": [{ "action": "c2pa.created", "digitalSourceType": origin }, edit], "allActionsIncluded": true }))
        };
        // Enhancement or resizing could be a generative model (AI upscaling)...
        assert_eq!(after(CAPTURE, json!({ "action": "c2pa.enhanced" })), Verdict::Unknown);
        assert_eq!(after(CAPTURE, json!({ "action": "c2pa.resized" })), Verdict::Unknown);
        assert_eq!(after(CAPTURE, json!({ "action": "c2pa.resized", "digitalSourceType": HUMAN_EDITS })), Verdict::NoAi);
        // ...but it adds no material, so AI content stays entirely AI.
        assert_eq!(after(AI, json!({ "action": "c2pa.resized" })), Verdict::Ai);
    }

    #[test]
    fn source_type_on_placed_action_counts() {
        let actions = json!({
            "actions": [
                { "action": "c2pa.created", "digitalSourceType": CAPTURE },
                { "action": "c2pa.placed", "digitalSourceType": AI,
                  "parameters": { "ingredients": [{ "url": "self#jumbf=c2pa.assertions/c2pa.ingredient.v3" }] } },
            ],
            "allActionsIncluded": true,
        });
        let unsigned = json!([{ "relationship": "componentOf", "label": "c2pa.ingredient.v3", "title": "raw.jpg" }]);
        assert_eq!(classify_store(&[("m", actions, unsigned, false)]), Verdict::IncludesAi);
    }

    #[test]
    fn trained_algorithmic_data_is_not_assumed_generative() {
        let data = "http://c2pa.org/digitalsourcetype/trainedAlgorithmicData";
        let actions = json!({ "actions": [{ "action": "c2pa.created", "digitalSourceType": data }], "allActionsIncluded": true });
        assert_eq!(single(actions), Verdict::Unknown);
    }

    #[test]
    fn template_parameters_merge_into_parameters() {
        let actions = json!({
            "actions": [{ "action": "c2pa.deleted", "parameters": { "description": "own" } }],
            "templates": [{ "action": "c2pa.deleted", "templateParameters": { "ingredients": [1], "description": "template" } }],
        });
        let merged = merged_actions(&actions);
        assert_eq!(merged[0]["parameters"], json!({ "ingredients": [1], "description": "own" }));
    }
}
