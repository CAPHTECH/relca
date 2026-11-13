//! Fail-closed argument/file boundaries and the three ADR 0058 conform jobs.
#[path = "support/ledger_harness.rs"]
mod h;
use h::{Spec, known, oracle};
use serde_json::{Value, json};

const VERIFY_REASON: &str =
    "relca verify: not implemented; outside Phase 1; external backend is not connected\n";
fn conform(log: &str) -> h::Output {
    h::cli(&[
        "conform",
        h::CONFORM_MODEL,
        "--log",
        log,
        "--scope",
        "event-contracts",
    ])
}
fn ok() -> h::Output {
    conform("crates/relca-cli/tests/snapshots/inputs/q3-conform-ok.jsonl")
}
fn unknown() -> h::Output {
    conform("crates/relca-cli/tests/snapshots/inputs/q3-conform-unknown.jsonl")
}
#[test]
fn green_three_conform_jobs_have_independent_hand_classifications() {
    let a = ok();
    assert_eq!(
        (
            a.exit,
            a.value["verdict"].as_str(),
            a.value["conformance"].as_str()
        ),
        (0, Some("satisfied"), Some("conformant"))
    );
    let b = unknown();
    assert_eq!(
        (
            b.exit,
            b.value["verdict"].as_str(),
            b.value["conformance"].as_str()
        ),
        (2, Some("inconclusive"), Some("unknown"))
    );
    assert!(
        b.value["checks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["reason"] == "missing_start")
    );
    let c = h::cli(&["conform"]);
    assert_eq!((c.exit, c.value["verdict"].as_str()), (3, Some("invalid")));
    assert_eq!(c.value["diagnostics"][0]["code"], "E-LOG-0");
    let a_log = std::fs::read_to_string(
        h::root().join("crates/relca-cli/tests/snapshots/inputs/q3-conform-ok.jsonl"),
    )
    .unwrap();
    let b_log = std::fs::read_to_string(
        h::root().join("crates/relca-cli/tests/snapshots/inputs/q3-conform-unknown.jsonl"),
    )
    .unwrap();
    let lines = a_log
        .lines()
        .enumerate()
        .filter(|(n, _)| *n != 1)
        .map(|(_, s)| s)
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    assert_eq!(lines, b_log, "exactly one start removed, no other fault");
}
#[test]
fn conform_success_ledger_connects_model_log_scope_and_three_limits() {
    let v = ok();
    h::ledger(&v.value, &Spec::conform());
    assert_eq!(v.value["scope"], "observable_event_contracts_v1");
    assert_eq!(v.value["log"]["clock"], "shared-test-clock");
    assert_eq!(v.value["log"]["window"], json!({"from":0,"to":20000}));
    assert_eq!(
        v.value["ledger"]["execution"]["limits"]["max_events"],
        known(10000)
    );
}
#[test]
fn conform_unknown_retains_settings_without_promoting_missing_start() {
    let v = unknown();
    assert_eq!(v.exit, 2);
    h::ledger(&v.value, &Spec::conform());
    assert_eq!(
        v.value["ledger"]["execution"]["seed"],
        oracle::na("no_analysis")
    );
    assert!(
        v.value["not_checked"]
            .as_array()
            .unwrap()
            .iter()
            .any(|x| x["item"] == "process_path")
    );
}
#[test]
fn conform_argument_failure_knows_limits_but_not_model_bounds_or_actor_versions() {
    let v = h::cli(&["conform"]);
    let mut s = Spec::prelower("log_conformance", true);
    s.builtin = "builtin_event_contracts";
    s.seed = oracle::na("no_analysis");
    s.replications = s.seed.clone();
    s.confidence = s.seed.clone();
    s.random = s.seed.clone();
    s.limits = Spec::conform().limits;
    h::ledger(&v.value, &s);
}
#[test]
fn conform_core_entry_has_same_ledger_and_does_not_rewrite_log_or_model() {
    let model = std::fs::read_to_string(h::root().join(h::CONFORM_MODEL)).unwrap();
    let path = h::root().join("crates/relca-cli/tests/snapshots/inputs/q3-conform-ok.jsonl");
    let log = std::fs::read(&path).unwrap();
    let r = relca_core::conformance::conform(&model, log.as_slice(), None, None);
    let core = serde_json::to_value(r).unwrap();
    h::ledger(&core, &Spec::conform());
    assert_eq!(core, ok().value);
    assert_eq!(std::fs::read(path).unwrap(), log);
    assert_eq!(
        std::fs::read_to_string(h::root().join(h::CONFORM_MODEL)).unwrap(),
        model
    );
}
#[test]
fn verify_three_probes_have_identical_ledger_and_never_parse_input() {
    let absent = ".tmp/definitely-absent-q3-model.relca";
    let probes = [
        vec!["verify"],
        vec!["verify", "fixtures/serial-time/model.relca"],
        vec!["verify", absent],
    ];
    let mut previous = None;
    let mut errors = Vec::new();
    for args in probes {
        let v = h::cli(&args);
        assert_eq!(v.exit, 4);
        assert_eq!(v.stderr, VERIFY_REASON);
        assert_eq!(v.value["verdict"], "unsupported");
        h::record(&v.value, &Spec::minimal("verify"), &mut errors);
        if v.value.get("ledger").is_some() {
            assert_eq!(
                v.value.as_object().unwrap().len(),
                3,
                "only ledger, uncertainty and verdict"
            );
        }
        if let Some(p) = previous {
            assert_eq!(v.stdout, p);
        }
        previous = Some(v.stdout);
    }
    h::finish(errors);
}
#[test]
fn common_refusal_no_command_unknown_command_and_bad_diff_include_ledger() {
    let mut errors = Vec::new();
    for args in [vec![], vec!["no-such-command"], vec!["diff"]] {
        let v = h::cli(&args);
        assert_eq!(v.exit, 3);
        assert_eq!(v.value["verdict"], "invalid");
        h::record(&v.value, &Spec::minimal("cli_refusal"), &mut errors);
        if v.value.get("ledger").is_some() {
            assert_eq!(v.value.as_object().unwrap().len(), 3);
            assert_eq!(
                v.value["uncertainty"]["estimation"]["reason"],
                "no_analysis"
            );
        }
    }
    h::finish(errors);
}
#[test]
fn unreadable_check_and_simulate_have_no_empty_source_hash_or_fake_bounds() {
    let absent = ".tmp/definitely-absent-q3-model.relca";
    for command in ["check", "simulate"] {
        let v = h::cli(&[command, absent]);
        assert_eq!(v.exit, 3);
        assert_eq!(v.value["model"]["sha256"], Value::Null);
        assert_eq!(v.value["bounds"]["clock"], Value::Null);
        h::ledger(
            &v.value,
            &Spec::prelower(
                if command == "check" {
                    "exhaustive"
                } else {
                    "simulation"
                },
                true,
            ),
        );
    }
}
#[test]
fn all_structured_argument_failure_factories_add_ledger() {
    let mut errors = Vec::new();
    let check = h::cli(&["check"]);
    h::record(
        &check.value,
        &Spec::prelower("exhaustive", true),
        &mut errors,
    );
    let sim = h::cli(&["simulate"]);
    h::record(&sim.value, &Spec::prelower("simulation", true), &mut errors);
    let explanation = h::cli(&["explain"]);
    let mut s = Spec::prelower("exhaustive", true);
    s.limits["capture_limit"] = known(200000);
    h::record(&explanation.value, &s, &mut errors);
    let compare = h::cli(&["compare"]);
    h::record(&compare.value, &Spec::aggregate("compare"), &mut errors);
    let mutation = h::cli(&["mutate"]);
    let mut rejected = Spec::aggregate("mutation");
    rejected.limits["mutant_limit"] =
        serde_json::json!({"status":"not_applicable","value":null,"reason":"no_analysis"});
    h::record(&mutation.value, &rejected, &mut errors);
    h::finish(errors);
}
#[test]
fn green_snapshot_extra_job_contract_is_exactly_three_conform_inputs() {
    let rows: Value =
        serde_json::from_str(include_str!("../../../scripts/snapshot-extra-jobs.json")).unwrap();
    let selected = rows
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["id"].as_str().unwrap().starts_with("q3-conform-"))
        .collect::<Vec<_>>();
    assert_eq!(selected.len(), 3);
    for (id, exit, verdict) in [
        ("q3-conform-ok", 0, "satisfied"),
        ("q3-conform-unknown", 2, "inconclusive"),
        ("q3-conform-arguments", 3, "invalid"),
    ] {
        let row = selected.iter().find(|r| r["id"] == id).unwrap();
        assert_eq!(row["args"][0], "conform");
        assert_eq!(row["expected_exit"], exit);
        assert_eq!(row["expected_verdict"], verdict);
    }
    assert!(include_str!("../../../scripts/snapshots.py").contains("\"verify\", \"conform\""));
}
#[test]
fn diff_and_compare_unreadable_children_keep_requested_selection_and_unknown_bounds() {
    let absent = ".tmp/definitely-absent-q3-model.relca";
    let v = h::cli(&["diff", absent, "--from", "AsIs", "--to", "ToBeP"]);
    assert_eq!(v.exit, 3);
    for (side, name) in [("from", "AsIs"), ("to", "ToBeP")] {
        let mut s = Spec::prelower("exhaustive", true);
        s.selection["requested"]["configuration"] = json!(name);
        assert!(v.value[side]["report"]["model"]["sha256"].is_null());
        h::ledger(&v.value[side]["report"], &s);
    }
    h::ledger(&v.value, &Spec::aggregate("diff"));
    let v = h::cli(&[
        "compare",
        absent,
        "--plans",
        "AsIs,ToBeP",
        "--mode",
        "simulation",
    ]);
    assert_eq!(v.exit, 3);
    for (n, name) in [(0, "AsIs"), (1, "ToBeP")] {
        let mut s = Spec::prelower("simulation", true);
        s.selection["requested"]["configuration"] = json!(name);
        h::ledger(&v.value["plans"][n]["report"], &s);
    }
    h::ledger(&v.value, &Spec::aggregate("compare"));
}
