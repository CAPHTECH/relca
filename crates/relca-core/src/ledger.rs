//! ADR 0058: reporting provenance, never an input to analysis or random keys.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

mod links;
mod settings;
pub use settings::{Refusal, refusal, unsupported};

pub(crate) fn source_reports(host: &Value, mode: &str) -> Vec<String> {
    links::sources(host, mode)
        .into_iter()
        .map(|p| {
            p.strip_suffix("/ledger")
                .expect("child ledger path")
                .to_owned()
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ledger {
    pub schema: String,
    pub producer: Value,
    pub backends: Value,
    pub derivation: Value,
    pub execution: Value,
    pub actor_versions: Value,
    pub context: Value,
    pub subjects: Value,
}

pub(crate) fn known(value: impl Into<Value>) -> Value {
    json!({"status":"known","value":value.into(),"reason":null})
}
pub(crate) fn absent(status: &str, reason: &str) -> Value {
    json!({"status":status,"value":null,"reason":reason})
}
pub(crate) fn na(reason: &str) -> Value {
    absent("not_applicable", reason)
}
pub(crate) fn unavailable(reason: &str) -> Value {
    absent("unavailable", reason)
}
pub(crate) fn undeclared() -> Value {
    absent("not_declared", "not_declared")
}
pub(crate) fn complete(
    ledger: &mut Ledger,
    host: &Value,
    verdict: &mut crate::Verdict,
    diagnostics: &mut Vec<crate::diag::Diagnostic>,
) {
    if let Err(message) = ledger.connect(host).and_then(|()| ledger.validate(host)) {
        if !diagnostics
            .iter()
            .any(|d| d.code == crate::diag::U_LEDGER_0)
        {
            diagnostics.push(crate::diag::Diagnostic {
                code: crate::diag::U_LEDGER_0.into(),
                severity: crate::diag::Severity::Error,
                message,
                span: None,
            });
        }
        if !matches!(verdict, crate::Verdict::Invalid | crate::Verdict::Violated) {
            *verdict = crate::Verdict::Unsupported;
        }
    }
}
fn ref_at(host: &Value, path: &str) -> Value {
    if host.pointer(path).is_some() {
        known(path)
    } else {
        na("no_analysis")
    }
}
fn refs(host: &Value, paths: &[&str]) -> Value {
    json!(
        paths
            .iter()
            .filter(|p| host.pointer(p).is_some())
            .map(|p| known(*p))
            .collect::<Vec<_>>()
    )
}

impl Ledger {
    /// Provenance for a request whose input could not be obtained.
    pub fn input_failure(mode: &str) -> Self {
        Self::prelower(mode, true)
    }

    pub(crate) fn prelower(mode: &str, input_unavailable: bool) -> Self {
        let reason = if input_unavailable {
            "input_unavailable"
        } else {
            "not_lowered"
        };
        let sampling = mode == "simulation";
        let no_sample = if sampling {
            unavailable(reason)
        } else {
            na("exhaustive_analysis")
        };
        let mut limits = json!({});
        for key in [
            "state_budget",
            "capture_limit",
            "mutant_limit",
            "max_events",
            "max_line_bytes",
            "max_order_states",
        ] {
            limits[key] = na("no_analysis");
        }
        if !sampling {
            limits["state_budget"] = unavailable("not_lowered");
        }
        Self {
            schema: "relca-conclusion-ledger/v1".into(),
            producer: json!({"name":"relca","version":env!("CARGO_PKG_VERSION"),"version_source":"cargo_package",
                "build_identity":absent("not_recorded","package_version_only")}),
            backends: json!({"builtin":{"name":if sampling {"builtin_simulation"} else {"builtin_finite"},
                "version":known(env!("CARGO_PKG_VERSION")),"status":"not_run"},
                "external":{"status":"not_connected","name":null,"version":null,"reason":"external_backend_not_connected"}}),
            derivation: json!({"bundle":"relca-conclusions-p2-uncertainty-2","components":[]}),
            execution: json!({"mode":mode,"status":"not_run","selection":{"requested":{"configuration":null,"scenario":null},
                "effective":{"configuration":unavailable(reason),"scenario":unavailable(reason),"process":unavailable(reason)}},
                "seed":no_sample,"replications":no_sample,"confidence":no_sample,"random_method":no_sample,
                "bounds":unavailable(reason),"limits":limits}),
            actor_versions: json!({"status":"unavailable","entries":[],"reason":"not_lowered"}),
            context: json!({}),
            subjects: json!([]),
        }
    }

    /// Resolve only the closed, type-owned evidence locations. No analysis runs.
    pub(crate) fn connect(&mut self, host: &Value) -> Result<(), String> {
        self.derivation["bundle"] = serde_json::json!(conclusion_bundle(host));
        self.derivation["components"] = links::components(host)?;
        self.context = json!({"model":ref_at(host,"/model"),"mode":ref_at(host,"/mode"),
            "assumptions":refs(host,&["/assumptions","/explanation/assumptions"]),
            "cut":ref_at(host,"/cut"),"boundaries":refs(host,&["/bounds","/cut_context","/limits","/log/window"]),
            "scope":refs(host,&["/unchecked","/checked","/scope","/simulation","/not_checked","/mismatch","/explanation"]),
            "log":ref_at(host,"/log"),"mutation":if host.get("original").is_some_and(|v| !v.is_null()) {known("/original/model")} else {na("no_analysis")}});
        self.subjects = links::subjects(
            host,
            self.execution["mode"]
                .as_str()
                .ok_or("context_mismatch: mode")?,
        )?;
        Ok(())
    }

    pub fn validate(&self, host: &Value) -> Result<(), String> {
        crate::uncertainty::validate_host(host)?;
        if self.schema != "relca-conclusion-ledger/v1" {
            return Err("unsupported_version: ledger schema".into());
        }
        let canonical = Self::prelower("exhaustive", false);
        if self.producer != canonical.producer
            || self.backends["external"] != canonical.backends["external"]
            || self.derivation["bundle"] != conclusion_bundle(host)
        {
            return Err("version_unavailable: producer, external backend or bundle".into());
        }
        keys(&self.backends, &["builtin", "external"])?;
        keys(&self.backends["builtin"], &["name", "version", "status"])?;
        keys(&self.derivation, &["bundle", "components"])?;
        let builtin = &self.backends["builtin"];
        let version = match builtin["name"].as_str() {
            Some("builtin_finite" | "builtin_simulation" | "builtin_event_contracts") => {
                known(env!("CARGO_PKG_VERSION"))
            }
            Some("none") => na("no_analysis"),
            _ => return Err("unsupported_version: builtin".into()),
        };
        if builtin["version"] != version
            || builtin["status"] != self.execution["status"]
            || !matches!(
                builtin["status"].as_str(),
                Some("ran" | "not_run" | "not_applicable" | "unavailable")
            )
        {
            return Err("context_mismatch: builtin execution".into());
        }
        validate_execution(&self.execution, host)?;
        validate_actors(&self.actor_versions, host)?;
        validate_settings(self, host)?;
        let mut expected = self.clone();
        expected.connect(host)?;
        if self.context != expected.context
            || self.subjects != expected.subjects
            || self.derivation != expected.derivation
        {
            return Err("context_mismatch: evidence or local version".into());
        }
        Ok(())
    }
}

fn conclusion_bundle(host: &Value) -> &'static str {
    fn recovery(v: &Value) -> bool {
        v.get("response_recovery").is_some_and(Value::is_object)
            || v["explanation"]["version"] == "invoice-counterexample-p3a-recovery-1"
            || ["from", "to"].iter().any(|s| {
                v.get(*s)
                    .and_then(|s| s.get("report"))
                    .is_some_and(recovery)
            })
            || ["plans", "points"].iter().any(|s| {
                v.get(*s)
                    .and_then(Value::as_array)
                    .is_some_and(|rows| rows.iter().filter_map(|r| r.get("report")).any(recovery))
            })
    }
    if recovery(host) {
        "relca-conclusions-p3a-recovery-1"
    } else {
        "relca-conclusions-p2-uncertainty-2"
    }
}

fn validate_settings(ledger: &Ledger, host: &Value) -> Result<(), String> {
    let e = &ledger.execution;
    let mode = e["mode"].as_str().ok_or("context_mismatch: mode type")?;
    if mode == "diff" {
        for side in ["from", "to"] {
            let report = &host[side]["report"];
            let child = &report["ledger"];
            let selection = &child["execution"]["selection"];
            let configuration = &host[side]["configuration"];
            let requested = &selection["requested"]["configuration"];
            let effective = &selection["effective"]["configuration"];
            let lowering_evidence =
                report["actors"].is_array() || child["execution"]["status"] == "ran";
            let unlowered = child["actor_versions"]["status"] == "unavailable"
                && child["actor_versions"]["reason"] == "not_lowered"
                && !lowering_evidence;
            let lowered_or_known = lowering_evidence
                || child["actor_versions"]["status"] == "known"
                || effective["status"] == "known";
            // A structural prelower wire can have no requested name. Do not
            // invent one, but known selection or Report evidence of lowering
            // cannot use that escape merely by claiming not_lowered.
            if ((!requested.is_null() || !unlowered || effective["status"] == "known")
                && requested != configuration)
                || (lowered_or_known && *effective != known(configuration.clone()))
            {
                return Err("context_mismatch: diff child configuration ownership".into());
            }
        }
    }
    let sample_reason = match mode {
        "exhaustive" => "exhaustive_analysis",
        "diff" | "compare" | "mutation" | "sensitivity_exhaustive" | "sensitivity_simulation" => {
            "aggregate_per_source"
        }
        "log_conformance" | "verify" | "cli_refusal" | "simulation" => "no_analysis",
        _ => return Err("context_mismatch: unknown mode".into()),
    };
    for key in ["seed", "replications", "confidence", "random_method"] {
        if mode != "simulation" && e[key] != na(sample_reason) {
            return Err("settings_unavailable: inapplicable sample setting".into());
        }
    }
    if mode == "simulation" {
        let configured = e["seed"]["status"] == "known";
        if configured {
            if e["seed"]["value"].as_u64().is_none()
                || e["replications"]["value"].as_u64().is_none()
                || e["confidence"]["value"].as_str().is_none_or(str::is_empty)
                || e["random_method"] != known("sha256-counter-v1")
            {
                return Err("settings_unavailable: sample types".into());
            }
            for key in ["seed", "replications", "confidence"] {
                if let Some(value) = host["simulation"].get(key).filter(|v| !v.is_null())
                    && &e[key]["value"] != value
                {
                    return Err("context_mismatch: requested sample setting".into());
                }
            }
        } else if ["seed", "replications", "confidence", "random_method"]
            .iter()
            .any(|k| e[*k]["status"] != "unavailable")
        {
            return Err("settings_unavailable: mixed sample availability".into());
        }
    }
    for key in ["seed", "replications"] {
        if e[key]["status"] == "known" && e[key]["value"].as_u64().is_none() {
            return Err("settings_unavailable: integer setting".into());
        }
    }
    let bounds = &e["bounds"];
    if bounds["status"] == "known" && bounds["value"].is_object() {
        keys(&bounds["value"], &["cases", "live_tasks", "clock"])?;
        if mode != "log_conformance"
            || bounds["value"]
                .as_object()
                .unwrap()
                .values()
                .any(|v| v.as_u64().is_none())
        {
            return Err("context_mismatch: direct bounds".into());
        }
    }
    if e["status"] == "ran"
        && mode == "exhaustive"
        && e["limits"]["state_budget"]["status"] != "known"
    {
        return Err("settings_unavailable: unrecorded running budget".into());
    }
    Ok(())
}

fn keys(value: &Value, expected: &[&str]) -> Result<(), String> {
    let fields = value
        .as_object()
        .ok_or("context_mismatch: object required")?;
    if fields.len() != expected.len() || expected.iter().any(|k| !fields.contains_key(*k)) {
        return Err("context_mismatch: closed keys".into());
    }
    Ok(())
}
fn state(value: &Value) -> Result<(), String> {
    keys(value, &["status", "value", "reason"])?;
    // Text can be a real source identifier (including a process named unknown).
    // Producer/component versions have their own closed registration checks.
    let valid = match value["status"].as_str() {
        Some("known") => {
            !value["value"].is_null()
                && value["reason"].is_null()
                && value["value"].as_str().is_none_or(|s| !s.is_empty())
        }
        Some("not_declared") => value == &undeclared(),
        Some("not_applicable") => {
            value["value"].is_null()
                && matches!(
                    value["reason"].as_str(),
                    Some("no_analysis" | "aggregate_per_source" | "exhaustive_analysis")
                )
        }
        Some("unavailable") => {
            value["value"].is_null()
                && matches!(
                    value["reason"].as_str(),
                    Some("input_unavailable" | "not_lowered" | "execution_settings_unavailable")
                )
        }
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err("settings_unavailable: invalid state".into())
    }
}
fn validate_execution(e: &Value, host: &Value) -> Result<(), String> {
    keys(
        e,
        &[
            "mode",
            "status",
            "selection",
            "seed",
            "replications",
            "confidence",
            "random_method",
            "bounds",
            "limits",
        ],
    )?;
    if let Some(mode) = host.get("mode")
        && mode != &e["mode"]
    {
        return Err("context_mismatch: mode".into());
    }
    keys(&e["selection"], &["requested", "effective"])?;
    keys(&e["selection"]["requested"], &["configuration", "scenario"])?;
    keys(
        &e["selection"]["effective"],
        &["configuration", "scenario", "process"],
    )?;
    for key in ["configuration", "scenario"] {
        let v = &e["selection"]["requested"][key];
        if !v.is_null() && !v.is_string() {
            return Err("context_mismatch: requested selection".into());
        }
    }
    for v in e["selection"]["effective"].as_object().unwrap().values() {
        state(v)?;
        if v["status"] == "known" && v["value"].as_str().is_none_or(str::is_empty) {
            return Err("context_mismatch: selection value type".into());
        }
    }
    for key in [
        "seed",
        "replications",
        "confidence",
        "random_method",
        "bounds",
    ] {
        state(&e[key])?;
    }
    if e["bounds"]["status"] == "known"
        && !e["bounds"]["value"].is_string()
        && !e["bounds"]["value"].is_object()
    {
        return Err("context_mismatch: bounds value type".into());
    }
    keys(
        &e["limits"],
        &[
            "state_budget",
            "capture_limit",
            "mutant_limit",
            "max_events",
            "max_line_bytes",
            "max_order_states",
        ],
    )?;
    for v in e["limits"].as_object().unwrap().values() {
        state(v)?;
        if v["status"] == "known" && v["value"].as_u64().is_none() {
            return Err("settings_unavailable: limit type".into());
        }
    }
    if e["bounds"]["status"] == "known"
        && e["bounds"]["value"].is_string()
        && (e["bounds"]["value"] != "/bounds"
            || host.pointer("/bounds/cases").is_none_or(Value::is_null))
    {
        return Err("context_mismatch: bounds".into());
    }
    Ok(())
}
fn validate_actors(a: &Value, host: &Value) -> Result<(), String> {
    keys(a, &["status", "entries", "reason"])?;
    let entries = a["entries"]
        .as_array()
        .ok_or("context_mismatch: actor entries")?;
    if a["status"] != "known" {
        if !entries.is_empty()
            || !matches!(
                (a["status"].as_str(), a["reason"].as_str()),
                (Some("unavailable"), Some("not_lowered"))
                    | (Some("not_applicable"), Some("aggregate_per_source"))
            )
        {
            return Err("context_mismatch: actor status".into());
        }
        return Ok(());
    }
    if !a["reason"].is_null() {
        return Err("context_mismatch: actor reason".into());
    }
    for entry in entries {
        keys(entry, &["actor", "via_model", "rule"])?;
        if entry["actor"].as_str().is_none_or(str::is_empty) {
            return Err("context_mismatch: actor name".into());
        }
        state(&entry["via_model"])?;
        state(&entry["rule"])?;
        if !matches!(
            entry["via_model"]["status"].as_str(),
            Some("known" | "not_declared")
        ) || !matches!(
            entry["rule"]["status"].as_str(),
            Some("known" | "not_declared")
        ) || (entry["via_model"]["status"] == "known"
            && entry["via_model"]["value"]
                .as_str()
                .is_none_or(str::is_empty))
        {
            return Err("context_mismatch: actor version type".into());
        }
        if entry["rule"]["status"] == "known" {
            keys(&entry["rule"]["value"], &["declared", "effective"])?;
            for field in ["declared", "effective"] {
                if entry["rule"]["value"][field]
                    .as_str()
                    .is_none_or(str::is_empty)
                {
                    return Err("version_unavailable: rule version".into());
                }
            }
        }
    }
    // Conformance has no Actor-table field. An explicitly null table on a
    // Report, unlike an absent field, witnesses that lowering did not finish.
    let Some(actors) = host.get("actors") else {
        return Ok(());
    };
    let actors = actors
        .as_array()
        .ok_or("context_mismatch: known actors without actor table")?;
    if actors.len() != entries.len()
        || actors
            .iter()
            .zip(entries)
            .any(|(a, e)| a["name"] != e["actor"])
    {
        return Err("context_mismatch: actor ownership".into());
    }
    for (actor, entry) in actors.iter().zip(entries) {
        let via = if actor["via_model"]["status"] == "declared" {
            known(actor["via_model"]["value"].clone())
        } else {
            undeclared()
        };
        let rule = actor
            .get("rule")
            .map_or_else(undeclared, |r| known(r["version"].clone()));
        if entry["via_model"] != via || entry["rule"] != rule {
            return Err("context_mismatch: actor declaration".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_known_effective_configuration_rejects_missing_requested_owner() {
        let source = include_str!("../../../fixtures/config-diff-parallel/model.relca");
        let control = serde_json::to_value(crate::diff::diff(source, "AsIs", "ToBeP")).unwrap();
        serde_json::from_value::<crate::diff::DiffReport>(control.clone()).unwrap();
        for side in ["from", "to"] {
            let mut bad = control.clone();
            bad[side]["report"]["ledger"]["execution"]["selection"]["requested"]["configuration"] =
                Value::Null;
            assert!(serde_json::from_value::<crate::diff::DiffReport>(bad).is_err());
        }
    }
}
