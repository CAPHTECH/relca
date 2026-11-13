//! Closed evidence locations and configuration-owned conclusion links.
use serde_json::{Value, json};
use std::collections::BTreeMap;

fn allowed(path: &str, v: &str) -> bool {
    if path == "/agency/degradation/rules" {
        return v == "agency-degradation-v1";
    }
    if path == "/metrics/contests/rules" {
        return v == crate::metrics::contests::RULES;
    }
    if path.starts_with("/points/") && path.ends_with("/effective_inputs/rules") {
        return v == "sensitivity-inputs-p2-1";
    }
    if path.ends_with("/viewpoints/rules") {
        return matches!(v, "agency-viewpoints-p1-1" | "agency-viewpoints-p1-2");
    }
    if path.ends_with("/common_causes/rules") {
        return v == "common-cause-p1-1";
    }
    if path.ends_with("/model_uncertainties/rules") {
        return v == "wlin2-suppression-p1-1";
    }
    if path.ends_with("/differences/rules") {
        return v == "agency-diff-p2-1";
    }
    if path.starts_with("/metrics/") {
        return matches!(
            v,
            "business-metrics-p1-1" | "business-metrics-p1-resource-1"
        );
    }
    if path.starts_with("/responsibilities/") {
        return matches!(v, "responsibility-p1-1" | "responsibility-p1-2");
    }
    if path.starts_with("/contest_paths/") {
        return matches!(v, "contest-p1-1" | "contest-p1-2");
    }
    match path {
        "/agency/rules" | "/agency/from/rules" | "/agency/to/rules" => matches!(
            v,
            "agency-p0-1"
                | "agency-p1-state-1"
                | "agency-p1-requires-1"
                | "agency-p1-rule-1"
                | "agency-p1-reads-1"
                | "agency-p2-response-1"
                | "agency-p2-loop-1"
        ),
        "/via_model_paths/rules" => {
            matches!(v, "via-model-read-paths-1" | "via-model-read-paths-reads-1")
        }
        "/quality/rules" => v == "quality-cohort-p1-1",
        "/evaluation/rules" => v == "compare-evaluation-p1-1",
        "/response_analysis/rules" => matches!(v, "response-p2-1" | "response-p3a-recovery-1"),
        "/response_recovery/rules" => v == crate::recovery::RULES,
        "/resource_time/rules" => v == "resource-time-v1",
        "/repetition/rules" => v == "bounded-repetition-1",
        "/priority_policy/rules" => v == "priority-queues-v1",
        "/work_profiles/rules" => v == crate::report::work_profiles::RULES,
        "/uncertainty/rules" => v == "uncertainty-p2-1",
        "/explanation/version" => matches!(
            v,
            "invoice-counterexample-v1" | "invoice-counterexample-p3a-recovery-1"
        ),
        "/schema" => matches!(
            v,
            "relca-conformance/v1"
                | "relca-conformance-approval/v1"
                | "relca-sensitivity-result/v1"
        ),
        "/scope/kind" => v == "enumerated_models",
        "/scope" => matches!(
            v,
            "observable_event_contracts_v1" | "observable_event_contracts_with_approval_burden_v1"
        ),
        "/approval_burden/schema" => v == "relca-approval-burden/v1",
        "/approval_burden/rules" => v == "approval-burden-observed-p2-1",
        _ => false,
    }
}
pub(super) fn components(host: &Value) -> Result<Value, String> {
    let sensitivity = host["schema"] == "relca-sensitivity-result/v1";
    if sensitivity
        && (!matches!(
            host["mode"].as_str(),
            Some("sensitivity_exhaustive" | "sensitivity_simulation")
        ) || host["scope"]["kind"] != "enumerated_models")
    {
        return Err("unsupported_version: sensitivity schema/mode/scope".into());
    }
    if (host.get("schema").is_some() || host.get("scope").is_some())
        && !sensitivity
        && !matches!(
            (host["schema"].as_str(), host["scope"].as_str()),
            (
                Some("relca-conformance/v1"),
                Some("observable_event_contracts_v1")
            ) | (
                Some("relca-conformance-approval/v1"),
                Some("observable_event_contracts_with_approval_burden_v1")
            )
        )
    {
        return Err("unsupported_version: conformance schema/scope pair".into());
    }
    let mut paths = Vec::new();
    for root in ["/agency", "/agency/from", "/agency/to"] {
        for suffix in [
            "/rules",
            "/viewpoints/rules",
            "/common_causes/rules",
            "/model_uncertainties/rules",
        ] {
            paths.push(format!("{root}{suffix}"));
        }
    }
    paths.extend(
        [
            "/agency/differences/rules",
            "/agency/degradation/rules",
            "/via_model_paths/rules",
            "/quality/rules",
            "/evaluation/rules",
            "/response_analysis/rules",
            "/response_recovery/rules",
            "/resource_time/rules",
            "/repetition/rules",
            "/priority_policy/rules",
            "/work_profiles/rules",
            "/uncertainty/rules",
            "/explanation/version",
            "/schema",
            "/scope",
            "/approval_burden/schema",
            "/approval_burden/rules",
        ]
        .map(String::from),
    );
    if sensitivity {
        paths.retain(|p| p != "/scope");
        paths.push("/scope/kind".into());
        if let Some(points) = host["points"].as_array() {
            paths.extend(
                points
                    .iter()
                    .enumerate()
                    .filter(|(_, p)| !p["effective_inputs"].is_null())
                    .map(|(i, _)| format!("/points/{i}/effective_inputs/rules")),
            );
        }
    }
    for field in ["responsibilities", "contest_paths"] {
        if let Some(rows) = host[field].as_array() {
            paths.extend((0..rows.len()).map(|i| format!("/{field}/{i}/rules")));
        }
    }
    if let Some(rows) = host["tables"]["agency"].as_array() {
        paths.extend((0..rows.len()).map(|i| format!("/tables/agency/{i}/differences/rules")));
    }
    fn metrics(v: &Value, path: &str, paths: &mut Vec<String>) {
        match v {
            Value::Object(fields) => {
                for (key, v) in fields {
                    let p = format!("{path}/{}", key.replace('~', "~0").replace('/', "~1"));
                    if key == "rules" {
                        paths.push(p);
                    } else {
                        metrics(v, &p, paths);
                    }
                }
            }
            Value::Array(rows) => {
                for (i, v) in rows.iter().enumerate() {
                    metrics(v, &format!("{path}/{i}"), paths);
                }
            }
            _ => {}
        }
    }
    if let Some(m) = host.get("metrics") {
        metrics(m, "/metrics", &mut paths);
    }
    paths.sort();
    paths.dedup();
    let mut out = Vec::new();
    for path in paths {
        if let Some(v) = host.pointer(&path) {
            let version = v
                .as_str()
                .ok_or("version_unavailable: local version type")?;
            if !allowed(&path, version) {
                return Err(format!("unsupported_version: {path}"));
            }
            out.push(json!({"name":path,"version":version,"at":path}));
        }
    }
    Ok(json!(out))
}

pub(super) fn sources(host: &Value, mode: &str) -> Vec<String> {
    match mode {
        "sensitivity_exhaustive" | "sensitivity_simulation" => host["points"]
            .as_array()
            .into_iter()
            .flatten()
            .enumerate()
            .filter(|(_, p)| p.get("report").is_some_and(|r| !r.is_null()))
            .map(|(i, _)| format!("/points/{i}/report/ledger"))
            .collect(),
        "diff" => ["/from/report", "/to/report"]
            .into_iter()
            .filter(|p| host.pointer(p).is_some())
            .map(|p| format!("{p}/ledger"))
            .collect(),
        "compare" => host["plans"]
            .as_array()
            .into_iter()
            .flatten()
            .enumerate()
            .map(|(i, _)| format!("/plans/{i}/report/ledger"))
            .collect(),
        "mutation" => {
            let mut out = Vec::new();
            if host.get("original").is_some_and(|v| !v.is_null()) {
                out.push("/original/ledger".into());
            }
            if let Some(kinds) = host["mutations"].as_array() {
                for (k, kind) in kinds.iter().enumerate() {
                    if let Some(mutants) = kind["mutants"].as_array() {
                        for (i, m) in mutants.iter().enumerate() {
                            if m.get("report").is_some_and(|v| !v.is_null()) {
                                out.push(format!("/mutations/{k}/mutants/{i}/report/ledger"));
                            }
                        }
                    }
                }
            }
            out
        }
        _ => vec![],
    }
}
pub(super) fn subjects(host: &Value, mode: &str) -> Result<Value, String> {
    let mut rows = BTreeMap::new();
    let all = sources(host, mode);
    let fields: &[&str] = match mode {
        "sensitivity_exhaustive" | "sensitivity_simulation" => {
            &["verdict", "scope", "summaries", "diagnostics"]
        }
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
            "response_recovery",
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
            "approval_burden",
        ],
    };
    for field in fields {
        if host.get(*field).is_some() {
            rows.insert(format!("/{field}"), all.clone());
        }
    }
    if host.get("uncertainty").is_some() {
        rows.insert("/uncertainty".into(), all.clone());
    }
    if matches!(mode, "sensitivity_exhaustive" | "sensitivity_simulation") {
        if let Some(points) = host["points"].as_array() {
            for (i, p) in points.iter().enumerate() {
                rows.insert(
                    format!("/points/{i}"),
                    if p.get("report").is_some_and(|r| !r.is_null()) {
                        vec![format!("/points/{i}/report/ledger")]
                    } else {
                        vec![]
                    },
                );
            }
        }
        if let Some(summaries) = host["summaries"].as_array() {
            for (i, _) in summaries.iter().enumerate() {
                rows.insert(format!("/summaries/{i}"), all.clone());
            }
        }
    }
    if mode == "compare" {
        let plans = host["plans"].as_array().ok_or("context_mismatch: plans")?;
        let source = |configuration: &Value| -> Result<String, String> {
            let found: Vec<_> = plans
                .iter()
                .enumerate()
                .filter(|(_, p)| p["configuration"] == *configuration)
                .collect();
            if found.len() != 1 {
                return Err("context_mismatch: configuration ownership".into());
            }
            Ok(format!("/plans/{}/report/ledger", found[0].0))
        };
        for table in ["properties", "terminals", "metrics", "quality"] {
            if let Some(table_rows) = host["tables"][table].as_array() {
                for (i, row) in table_rows.iter().enumerate() {
                    if let Some(cells) = row["values"].as_array() {
                        for (j, cell) in cells.iter().enumerate() {
                            rows.insert(
                                format!("/tables/{table}/{i}/values/{j}"),
                                vec![source(&cell["configuration"])?],
                            );
                        }
                    }
                }
            }
        }
        if let Some(pairs) = host["tables"]["agency"].as_array() {
            for (i, pair) in pairs.iter().enumerate() {
                rows.insert(
                    format!("/tables/agency/{i}"),
                    vec![source(&pair["from"])?, source(&pair["to"])?],
                );
            }
        }
    }
    if mode == "mutation" {
        let original: Vec<_> = all
            .iter()
            .filter(|p| *p == "/original/ledger")
            .cloned()
            .collect();
        if let Some(kinds) = host["mutations"].as_array() {
            for (k, kind) in kinds.iter().enumerate() {
                rows.insert(format!("/mutations/{k}/status"), original.clone());
                if let Some(mutants) = kind["mutants"].as_array() {
                    for (i, m) in mutants.iter().enumerate() {
                        let mut pair = original.clone();
                        if m.get("report").is_some_and(|v| !v.is_null()) {
                            pair.push(format!("/mutations/{k}/mutants/{i}/report/ledger"));
                        }
                        rows.insert(format!("/mutations/{k}/mutants/{i}"), pair);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn close_conformance_schema_and_scope_are_a_pair_not_independent_versions() {
        let pairs = [
            ("relca-conformance/v1", "observable_event_contracts_v1"),
            (
                "relca-conformance-approval/v1",
                "observable_event_contracts_with_approval_burden_v1",
            ),
        ];
        for (schema, scope) in pairs {
            assert!(components(&json!({"schema":schema,"scope":scope})).is_ok());
        }
        for (schema, scope) in [(pairs[0].0, pairs[1].1), (pairs[1].0, pairs[0].1)] {
            assert!(
                components(&json!({"schema":schema,"scope":scope})).is_err(),
                "{schema}: {scope}"
            );
        }
    }
}
