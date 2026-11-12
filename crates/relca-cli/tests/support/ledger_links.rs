//! Closed locations and ownership. Never search arbitrary payloads for a rules key.
use serde_json::{Value, json};
use std::collections::BTreeMap;

fn allowed(at: &str, version: &str) -> bool {
    if at.ends_with("/viewpoints/rules") {
        return matches!(version, "agency-viewpoints-p1-1" | "agency-viewpoints-p1-2");
    }
    if at.ends_with("/common_causes/rules") {
        return version == "common-cause-p1-1";
    }
    if at.ends_with("/model_uncertainties/rules") {
        return version == "wlin2-suppression-p1-1";
    }
    if at.ends_with("/differences/rules") {
        return version == "agency-diff-p2-1";
    }
    if at.starts_with("/metrics/") {
        return matches!(
            version,
            "business-metrics-p1-1" | "business-metrics-p1-resource-1"
        );
    }
    if at.starts_with("/responsibilities/") {
        return matches!(version, "responsibility-p1-1" | "responsibility-p1-2");
    }
    if at.starts_with("/contest_paths/") {
        return matches!(version, "contest-p1-1" | "contest-p1-2");
    }
    match at {
        "/agency/rules" | "/agency/from/rules" | "/agency/to/rules" => matches!(
            version,
            "agency-p0-1"
                | "agency-p1-state-1"
                | "agency-p1-requires-1"
                | "agency-p1-rule-1"
                | "agency-p1-reads-1"
                | "agency-p2-response-1"
        ),
        "/via_model_paths/rules" => matches!(
            version,
            "via-model-read-paths-1" | "via-model-read-paths-reads-1"
        ),
        "/quality/rules" => version == "quality-cohort-p1-1",
        "/evaluation/rules" => version == "compare-evaluation-p1-1",
        "/response_analysis/rules" => version == "response-p2-1",
        "/uncertainty/rules" => version == "uncertainty-p2-1",
        "/resource_time/rules" => version == "resource-time-v1",
        "/priority_policy/rules" => version == "priority-queues-v1",
        "/explanation/version" => version == "invoice-counterexample-v1",
        "/schema" => version == "relca-conformance/v1",
        "/scope" => version == "observable_event_contracts_v1",
        _ => false,
    }
}
pub fn components(report: &Value) -> Result<Value, String> {
    let mut paths = Vec::new();
    for base in ["/agency", "/agency/from", "/agency/to"] {
        for tail in [
            "/rules",
            "/viewpoints/rules",
            "/common_causes/rules",
            "/model_uncertainties/rules",
        ] {
            paths.push(format!("{base}{tail}"));
        }
    }
    paths.extend(
        [
            "/agency/differences/rules",
            "/via_model_paths/rules",
            "/quality/rules",
            "/evaluation/rules",
            "/response_analysis/rules",
            "/uncertainty/rules",
            "/resource_time/rules",
            "/priority_policy/rules",
            "/explanation/version",
            "/schema",
            "/scope",
        ]
        .map(String::from),
    );
    for (base, tail) in [("responsibilities", "rules"), ("contest_paths", "rules")] {
        if let Some(rows) = report[base].as_array() {
            for (n, _) in rows.iter().enumerate() {
                paths.push(format!("/{base}/{n}/{tail}"));
            }
        }
    }
    if let Some(rows) = report["tables"]["agency"].as_array() {
        for (n, _) in rows.iter().enumerate() {
            paths.push(format!("/tables/agency/{n}/differences/rules"));
        }
    }
    fn metrics(v: &Value, at: &str, out: &mut Vec<String>) {
        match v {
            Value::Object(fields) => {
                for (k, v) in fields {
                    let p = format!("{at}/{}", k.replace('~', "~0").replace('/', "~1"));
                    if k == "rules" {
                        out.push(p);
                    } else {
                        metrics(v, &p, out);
                    }
                }
            }
            Value::Array(rows) => {
                for (n, v) in rows.iter().enumerate() {
                    metrics(v, &format!("{at}/{n}"), out);
                }
            }
            _ => {}
        }
    }
    if let Some(m) = report.get("metrics") {
        metrics(m, "/metrics", &mut paths);
    }
    paths.sort();
    paths.dedup();
    let mut out = Vec::new();
    for at in paths {
        if let Some(version) = report.pointer(&at).and_then(Value::as_str) {
            if !allowed(&at, version) {
                return Err(format!("UNSUPPORTED_LOCAL_VERSION at {at}: {version}"));
            }
            out.push(json!({"name":at,"at":at,"version":version}));
        }
    }
    Ok(json!(out))
}
pub fn sources(report: &Value, mode: &str) -> Vec<String> {
    match mode {
        "diff" => ["/from/report/ledger", "/to/report/ledger"]
            .iter()
            .filter(|p| report.pointer(p.trim_end_matches("/ledger")).is_some())
            .map(|s| s.to_string())
            .collect(),
        "compare" => report["plans"]
            .as_array()
            .into_iter()
            .flatten()
            .enumerate()
            .map(|(n, _)| format!("/plans/{n}/report/ledger"))
            .collect(),
        "mutation" => {
            let mut out = Vec::new();
            if report.get("original").is_some_and(|v| !v.is_null()) {
                out.push("/original/ledger".into());
            }
            if let Some(kinds) = report["mutations"].as_array() {
                for (k, kind) in kinds.iter().enumerate() {
                    if let Some(mutants) = kind["mutants"].as_array() {
                        for (n, m) in mutants.iter().enumerate() {
                            if m.get("report").is_some_and(|v| !v.is_null()) {
                                out.push(format!("/mutations/{k}/mutants/{n}/report/ledger"));
                            }
                        }
                    }
                }
            }
            out
        }
        _ => Vec::new(),
    }
}
pub fn subjects(report: &Value, mode: &str) -> Result<Value, String> {
    let mut rows: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let all = sources(report, mode);
    let fields: &[&str] = match mode {
        "diff" => &[
            "verdict",
            "mismatch",
            "differences",
            "terminals",
            "properties",
            "metrics",
            "quality",
            "case_outcomes",
            "preservation",
            "diagnostics",
            "agency",
        ],
        "compare" => &["verdict", "mismatch", "diagnostics", "evaluation"],
        "mutation" => &["verdict", "run", "diagnostics"],
        _ => &[
            "verdict",
            "properties",
            "terminals",
            "case_outcomes",
            "agency",
            "quality",
            "metrics",
            "responsibilities",
            "contest_paths",
            "contest_runtime",
            "contest_summary",
            "obligations",
            "via_model_paths",
            "response_receipts",
            "response_analysis",
            "admission",
            "state_range_violations",
            "capability_refusals",
            "diagnostics",
            "actors",
            "explanation",
            "conformance",
            "checks",
            "cases",
            "summary",
            "unmapped_actor",
            "unmapped_operation",
        ],
    };
    for key in fields {
        if report.get(*key).is_some() {
            rows.insert(format!("/{key}"), all.clone());
        }
    }
    if report.get("uncertainty").is_some() {
        rows.insert("/uncertainty".into(), all.clone());
    }
    if mode == "compare" {
        let plans = report["plans"].as_array().ok_or("plans must be array")?;
        let source = |configuration: &Value| -> Result<String, String> {
            let matching: Vec<_> = plans
                .iter()
                .enumerate()
                .filter(|(_, p)| p["configuration"] == *configuration)
                .collect();
            if matching.len() != 1 {
                return Err("configuration has no unique source ledger".into());
            }
            Ok(format!("/plans/{}/report/ledger", matching[0].0))
        };
        for table in ["properties", "terminals", "metrics", "quality"] {
            if let Some(values) = report["tables"][table].as_array() {
                for (n, row) in values.iter().enumerate() {
                    if let Some(cells) = row["values"].as_array() {
                        for (c, cell) in cells.iter().enumerate() {
                            rows.insert(
                                format!("/tables/{table}/{n}/values/{c}"),
                                vec![source(&cell["configuration"])?],
                            );
                        }
                    }
                }
            }
        }
        if let Some(pairs) = report["tables"]["agency"].as_array() {
            for (n, pair) in pairs.iter().enumerate() {
                rows.insert(
                    format!("/tables/agency/{n}"),
                    vec![source(&pair["from"])?, source(&pair["to"])?],
                );
            }
        }
    }
    if mode == "mutation" {
        let original = all
            .iter()
            .filter(|p| p.as_str() == "/original/ledger")
            .cloned()
            .collect::<Vec<_>>();
        if let Some(kinds) = report["mutations"].as_array() {
            for (k, kind) in kinds.iter().enumerate() {
                rows.insert(format!("/mutations/{k}/status"), original.clone());
                if let Some(mutants) = kind["mutants"].as_array() {
                    for (n, m) in mutants.iter().enumerate() {
                        let mut pair = original.clone();
                        if m.get("report").is_some_and(|v| !v.is_null()) {
                            pair.push(format!("/mutations/{k}/mutants/{n}/report/ledger"));
                        }
                        rows.insert(format!("/mutations/{k}/mutants/{n}"), pair);
                    }
                }
            }
        }
    }
    Ok(json!(
        rows.into_iter()
            .map(
                |(target, source_ledgers)| json!({"target":target,"source_ledgers":source_ledgers})
            )
            .collect::<Vec<_>>()
    ))
}
