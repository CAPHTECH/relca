//! ADR 0058's three jobs plus ADR 0061's three approval observation jobs.
//! This closed addition does not broaden/exclude any pre-existing IR population.
use serde_json::{Value, json};
#[path = "approval_snapshot_jobs.rs"]
mod approval;
#[path = "log_conformance_wire.rs"]
mod wire;

pub const IDS: [&str; 6] = [
    "q3-conform-ok",
    "q3-conform-unknown",
    "q3-conform-arguments",
    "q8-approval-baseline",
    "q8-approval-ineligible",
    "q8-approval-person-union",
];
pub const REASON: &str = "observable event contracts and approval burden are not state-execution Reports; their closed jobs use separate wire and independent hand classifications";
pub fn arguments(id: &str) -> Vec<String> {
    if approval::IDS.contains(&id) {
        return approval::arguments(id);
    }
    let log = match id {
        "q3-conform-ok" => "crates/relca-cli/tests/snapshots/inputs/q3-conform-ok.jsonl",
        "q3-conform-unknown" => "crates/relca-cli/tests/snapshots/inputs/q3-conform-unknown.jsonl",
        "q3-conform-arguments" => return vec!["conform".into()],
        _ => panic!("unclassified conform job {id}"),
    };
    [
        "conform",
        "crates/relca-cli/tests/snapshots/inputs/q3-conform-model.relca",
        "--log",
        log,
        "--scope",
        "event-contracts",
    ]
    .map(str::to_string)
    .to_vec()
}
pub fn validate_set(jobs: &[Value]) -> Result<(), String> {
    let rows: Vec<_> = jobs.iter().filter(|j| j["args"][0] == "conform").collect();
    if rows.len() != IDS.len() {
        return Err("exactly six separately checked conform jobs are required".into());
    }
    let ids: std::collections::BTreeSet<_> = rows.iter().filter_map(|j| j["id"].as_str()).collect();
    if ids != IDS.into_iter().collect() {
        return Err("unknown/duplicate conform job id".into());
    }
    for row in rows {
        let args = arguments(row["id"].as_str().unwrap());
        if row["args"] != json!(args) {
            return Err("conform job ordered arguments changed".into());
        }
    }
    Ok(())
}
pub fn classified_args(args: &[String]) -> bool {
    IDS.iter().any(|id| arguments(id) == args)
}
pub fn check_output(id: &str, args: &[String], doc: &Value, exit: i32) -> Result<(), String> {
    if !IDS.contains(&id) || arguments(id) != args {
        return Err("unclassified conform job".into());
    }
    if approval::IDS.contains(&id) {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        approval::check_inputs(id, &root);
        return approval::check(id, doc, exit);
    }
    wire::validate(doc, exit)?;
    let (wanted_exit, verdict, conformance, events) = match id {
        "q3-conform-ok" => (0, "satisfied", "conformant", 6),
        "q3-conform-unknown" => (2, "inconclusive", "unknown", 5),
        "q3-conform-arguments" => (3, "invalid", "unknown", 0),
        _ => unreachable!(),
    };
    if exit != wanted_exit
        || doc["verdict"] != verdict
        || doc["conformance"] != conformance
        || doc["log"]["events"] != events
    {
        return Err(format!("independent conform classification differs: {id}"));
    }
    if id == "q3-conform-unknown" && !wire::has_reason(doc, "missing_start") {
        return Err("missing start must remain unknown".into());
    }
    let mut spec = wire::ledger_shape::Spec::conform();
    if id == "q3-conform-arguments" {
        spec = wire::ledger_shape::Spec::prelower("log_conformance", true);
        spec.builtin = "builtin_event_contracts";
        for v in [
            &mut spec.seed,
            &mut spec.replications,
            &mut spec.confidence,
            &mut spec.random,
        ] {
            *v = json!({"status":"not_applicable","value":null,"reason":"no_analysis"});
        }
        spec.limits = wire::ledger_shape::Spec::conform().limits;
        if !wire::has_code(doc, "E-LOG-0") || !doc["model"]["sha256"].is_null() {
            return Err("conform arguments must not read the model".into());
        }
    }
    wire::ledger_shape::validate_expected(doc, &spec)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn conform_population_is_exactly_six_and_a_seventh_is_rejected() {
        let jobs: Vec<_> = IDS
            .iter()
            .map(|id| json!({"id":id,"args":arguments(id)}))
            .collect();
        validate_set(&jobs).unwrap();
        let mut fourth = jobs.clone();
        fourth.push(json!({"id":"future-conform","args":["conform","future.relca"]}));
        assert!(validate_set(&fourth).is_err());
        assert!(validate_set(&jobs[1..]).is_err());
    }
    #[test]
    fn conform_all_six_run_the_wire_and_hand_checks() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut checked = Vec::new();
        for id in IDS {
            let args = arguments(id);
            let out = std::process::Command::new(env!("CARGO_BIN_EXE_relca"))
                .args(&args)
                .current_dir(&root)
                .output()
                .unwrap();
            let doc: Value = serde_json::from_slice(&out.stdout).unwrap();
            check_output(id, &args, &doc, out.status.code().unwrap()).unwrap();
            let mut bad_hand = doc.clone();
            bad_hand["log"]["events"] = json!(doc["log"]["events"].as_u64().unwrap() + 1);
            if !approval::IDS.contains(&id) {
                wire::validate(&bad_hand, out.status.code().unwrap()).unwrap();
            }
            assert!(
                check_output(id, &args, &bad_hand, out.status.code().unwrap()).is_err(),
                "separate hand oracle must run for {id}"
            );
            checked.push(id);
            eprintln!("conform wire+hand checked: {id}");
        }
        assert_eq!(checked, IDS);
    }
}
