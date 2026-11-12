//! Closed hand classification of the R2 raw jobs; no engine/reference calls.
//! Values are the interval sums in snapshots/inputs/r2-raw-notes.md.
use serde_json::{Value, json};
pub const IDS: [&str; 14] = [
    "r2-check-proceed",
    "r2-check-clock",
    "r2-check-horizon",
    "r2-simulate",
    "r2-explain",
    "r2-check-resolved",
    "r2-check-unresolved",
    "r2-compare-exhaustive",
    "r2-compare-simulation",
    "r2-diff",
    "r2-sensitivity",
    "r2-mutate-refusal",
    "r2-conform-refusal",
    "r2-verify-refusal",
];
const SINGLE: &str = "crates/relca-cli/tests/snapshots/inputs/r2-recovery.relca";
const PAIRED: &str = "crates/relca-cli/tests/snapshots/inputs/r2-comparison.relca";
fn require(ok: bool, why: &str) -> Result<(), String> {
    if ok { Ok(()) } else { Err(why.into()) }
}
pub fn args(id: &str) -> Vec<String> {
    let a = match id {
        "r2-check-proceed" => vec!["check", SINGLE, "--scenario", "Full"],
        "r2-check-clock" => vec!["check", SINGLE, "--scenario", "Clock"],
        "r2-check-horizon" => vec!["check", SINGLE, "--scenario", "Horizon"],
        "r2-simulate" => vec!["simulate", SINGLE, "--scenario", "Full"],
        "r2-explain" => vec!["explain", SINGLE, "--scenario", "Full"],
        "r2-check-resolved" => vec!["check", PAIRED, "--configuration", "A"],
        "r2-check-unresolved" => vec!["check", PAIRED, "--configuration", "B"],
        "r2-compare-exhaustive" => {
            vec!["compare", PAIRED, "--plans", "A,B", "--mode", "exhaustive"]
        }
        "r2-compare-simulation" => {
            vec!["compare", PAIRED, "--plans", "A,B", "--mode", "simulation"]
        }
        "r2-diff" => vec!["diff", PAIRED, "--from", "A", "--to", "B"],
        "r2-sensitivity" => vec![
            "sensitivity",
            "crates/relca-cli/tests/snapshots/inputs/r2-sensitivity.json",
            "--mode",
            "exhaustive",
        ],
        "r2-mutate-refusal" => vec!["mutate", SINGLE, "--scenario", "Full"],
        "r2-conform-refusal" => vec![
            "conform",
            SINGLE,
            "--scenario",
            "Full",
            "--log",
            "crates/relca-cli/tests/snapshots/inputs/r2-conform.jsonl",
            "--scope",
            "event-contracts",
        ],
        "r2-verify-refusal" => vec!["verify", SINGLE],
        _ => panic!("unclassified recovery raw job: {id}"),
    };
    a.into_iter().map(String::from).collect()
}
pub fn validate_set(jobs: &[Value]) -> Result<(), String> {
    let rows: Vec<_> = jobs
        .iter()
        .filter(|j| j["id"].as_str().is_some_and(|id| id.starts_with("r2-")))
        .collect();
    let ids: std::collections::BTreeSet<_> = rows.iter().filter_map(|j| j["id"].as_str()).collect();
    require(
        rows.len() == IDS.len() && ids == IDS.into_iter().collect(),
        "recovery raw job population changed",
    )?;
    for j in rows {
        let id = j["id"].as_str().unwrap();
        require(
            j["args"] == json!(args(id)),
            "recovery raw ordered argv changed",
        )?;
    }
    Ok(())
}
fn automatic(v: &Value) -> Option<&Value> {
    v["properties"]
        .as_array()?
        .iter()
        .find(|p| p["id"] == "receipt_resolution")
        .map(|p| &p["verdict"])
}
fn report(v: &Value, stop: &str, sampled: bool) -> Result<(), String> {
    require(
        v["ledger"]["derivation"]["bundle"] == "relca-conclusions-p3a-recovery-1",
        "recovery child bundle",
    )?;
    let r = &v["response_recovery"];
    require(r["rules"] == "response-recovery-p3a-1", "recovery rules")?;
    require(
        r["paths"].as_array().is_some_and(|p| p.len() == 1),
        "one deterministic joint path",
    )?;
    let commands = r["paths"][0]["commands"]
        .as_array()
        .ok_or("commands missing")?;
    require(commands.len() == 1, "one original command")?;
    let c = &commands[0];
    require(
        c["case_id"] == 1
            && c["saved_key"] == 1
            && c["command"] == "pay"
            && c["provider"] == "Psp"
            && c["capability"] == "Psp.charge_keyed",
        "command identity",
    )?;
    require(
        c["stop_reason"] == stop && c["unresolved"] == (stop != "end"),
        "recovery end state",
    )?;
    let queries = c["queries"].as_array().ok_or("queries missing")?;
    let resolutions = c["resolutions"].as_array().ok_or("resolutions missing")?;
    if matches!(stop, "truncated" | "cut_off") {
        require(
            queries.is_empty() && resolutions.is_empty(),
            "cut before query evidence",
        )?;
    } else {
        require(
            queries.len() == 1 && queries[0]["at"] == 3 && queries[0]["status"] == "committed_once",
            "query fact at3",
        )?;
        require(
            queries[0]["evidence"] == queries[0]["id"] && queries[0]["id"].is_string(),
            "query provenance",
        )?;
        if stop == "end" {
            require(
                resolutions.len() == 1
                    && resolutions[0]["at"] == 6
                    && resolutions[0]["query_ref"] == queries[0]["id"],
                "resolve same evidence at6",
            )?;
            let tasks = c["tasks"].as_array().ok_or("tasks missing")?;
            require(
                tasks.len() == 3,
                "three real handler tasks, not resolve or original charge",
            )?;
            let decision = tasks
                .iter()
                .find(|t| t["binding"] == "decision")
                .ok_or("decision missing")?;
            require(
                decision["finished_at"] == 6 && decision["result"] == "proceed",
                "decision finishes at6",
            )?;
        } else {
            require(resolutions.is_empty(), "unknown stop did not resolve")?;
        }
    }
    let a = &v["response_analysis"];
    require(a["rules"] == "response-p3a-recovery-1", "response rules")?;
    if sampled {
        require(
            a["coverage"] == "sampled" && r["coverage"] == "sampled",
            "sampled, not complete",
        )?;
        let wanted = [
            "sampled_retry_bounded",
            "sampled_receipt_channel",
            "sampled_receipt_not_success",
            "sampled_g_query",
            "sampled_recovery_evidence_matching",
            "sampled_recovery_incomplete_case",
        ];
        require(a["checked"] == json!(wanted), "sampled checked scope")?;
        serde_json::from_value::<relca_core::report::SimulationReport>(v.clone())
            .map_err(|e| e.to_string())?;
    } else {
        require(
            a["coverage"]
                == if stop == "truncated" {
                    "truncated"
                } else {
                    "complete"
                },
            "exhaustive coverage",
        )?;
        if v.get("explanation").is_some() {
            // Explain is flattened, and its ledger links the explanation. Do
            // not erase that evidence and then pretend it is a plain Report.
            require(
                v["explanation"]["version"] == "invoice-counterexample-p3a-recovery-1",
                "explain local version",
            )?;
            let ledger: relca_core::ledger::Ledger =
                serde_json::from_value(v["ledger"].clone()).map_err(|e| e.to_string())?;
            ledger.validate(v)?;
        } else {
            serde_json::from_value::<relca_core::report::Report>(v.clone())
                .map_err(|e| e.to_string())?;
        }
        let receipt = &v["response_receipts"][0];
        require(
            receipt["receipt"] == "unknown"
                && receipt["observed_result"].is_null()
                && receipt["resolved_effect"] == false,
            "old unknown receipt is not rewritten",
        )?;
        if stop == "end" {
            require(
                v["terminals"][0]["elapsed"] == json!({"min":7,"max":7})
                    && v["terminals"][0]["work"] == json!({"min":7,"max":7}),
                "end/work interval sums",
            )?;
        } else if stop == "receipt_unknown" {
            let t = &v["case_outcomes"][0]["terminals"][0];
            require(
                t["kind"] == "receipt_unknown"
                    && t["end"].is_null()
                    && t["work"].is_null()
                    && t["observed_work"] == json!({"min":3,"max":3}),
                "unfinished observed work3",
            )?;
        }
    }
    require(
        automatic(v)
            == Some(&json!(if sampled || stop != "end" {
                "inconclusive"
            } else {
                "satisfied"
            })),
        "receipt resolution fold",
    )?;
    Ok(())
}
pub fn check(id: &str, argv: &[String], v: &Value, exit: i32) -> Result<(), String> {
    require(
        IDS.contains(&id) && argv == args(id),
        "unclassified recovery invocation",
    )?;
    let (code, verdict) = match id {
        "r2-check-proceed" | "r2-explain" | "r2-simulate" => (1, "violated"),
        "r2-check-resolved" | "r2-sensitivity" => (0, "satisfied"),
        "r2-mutate-refusal" | "r2-conform-refusal" | "r2-verify-refusal" => (4, "unsupported"),
        _ => (2, "inconclusive"),
    };
    require(
        exit == code && v["verdict"] == verdict,
        "recovery hand root fold",
    )?;
    match id {
        "r2-check-proceed" | "r2-check-resolved" => report(v, "end", false)?,
        "r2-check-clock" => {
            report(v, "truncated", false)?;
            require(v["bounds"]["truncated"] == true, "clock cut")?;
        }
        "r2-check-horizon" => {
            report(v, "cut_off", false)?;
            require(
                v["bounds"]["truncated"] == false,
                "horizon is not a computational cut",
            )?;
        }
        "r2-check-unresolved" => report(v, "receipt_unknown", false)?,
        "r2-simulate" => report(v, "end", true)?,
        "r2-explain" => {
            report(v, "end", false)?;
            let examples = v["explanation"]["counterexamples"]
                .as_array()
                .ok_or("missing explanations")?;
            let c = examples
                .iter()
                .find(|c| c["id"].as_str().is_some_and(|id| id.contains("Deadline")))
                .ok_or("missing deadline example")?;
            require(c["replay"]["status"] == "verified", "independent replay")?;
            let steps = c["steps"].as_array().ok_or("missing replay steps")?;
            for (kind, time) in [("receipt_query", 3), ("resolve_effect", 6)] {
                require(
                    steps
                        .iter()
                        .any(|s| s["kind"] == kind && s["time"] == time && s["command"] == "pay"),
                    "distinct evidence and resolution events",
                )?;
            }
        }
        "r2-compare-exhaustive" | "r2-compare-simulation" => {
            let plans = v["plans"].as_array().ok_or("plans missing")?;
            require(plans.len() == 2, "two plans")?;
            require(
                plans[0]["configuration"] == "A" && plans[1]["configuration"] == "B",
                "plan identities",
            )?;
            let sampled = id == "r2-compare-simulation";
            report(&plans[0]["report"], "end", sampled)?;
            report(&plans[1]["report"], "receipt_unknown", sampled)?;
        }
        "r2-diff" => {
            report(&v["from"]["report"], "end", false)?;
            report(&v["to"]["report"], "receipt_unknown", false)?;
        }
        "r2-sensitivity" => {
            let points = v["points"].as_array().ok_or("points missing")?;
            require(points.len() == 2, "two finite controls")?;
            for p in points {
                report(&p["report"], "end", false)?;
            }
        }
        "r2-mutate-refusal" => {
            report(&v["original"], "end", false)?;
            require(
                v["diagnostics"].as_array().is_some_and(|ds| {
                    ds.iter().any(|d| {
                        d["code"] == "U-SEM-0" && d["message"] == "recovery_feature_combination"
                    })
                }),
                "dedicated recovery mutation refusal",
            )?;
            require(
                v["mutations"].as_array().is_some_and(|ms| {
                    ms.iter()
                        .all(|m| m["mutants"].as_array().is_some_and(Vec::is_empty))
                }),
                "no mutant execution",
            )?;
        }
        "r2-conform-refusal" => require(
            v.to_string().contains("recovery_feature_combination"),
            "dedicated model-feature refusal",
        )?,
        "r2-verify-refusal" => require(
            v["ledger"]["backends"]["external"]["status"] == "not_connected",
            "external backend not connected",
        )?,
        _ => unreachable!(),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn r2_population_rejects_loss_extra_and_argv_change() {
        let rows: Vec<_> = IDS
            .iter()
            .map(|id| json!({"id":id,"args":args(id)}))
            .collect();
        validate_set(&rows).unwrap();
        let mut bad = rows.clone();
        bad.pop();
        assert!(validate_set(&bad).is_err());
        let mut bad = rows.clone();
        bad.push(json!({"id":"r2-future","args":["conform","future"]}));
        assert!(validate_set(&bad).is_err());
        let mut bad = rows;
        bad[0]["args"][0] = json!("verify");
        assert!(validate_set(&bad).is_err());
    }
}
