//! Real core and CLI acceptance. Expected values precede implementation (ADR 0058).
#[path = "support/ledger_harness.rs"]
mod h;
use h::{Spec, known, oracle};
use serde_json::{Value, json};

#[test]
fn green_legacy_serial_cut_and_simulation_hand_values_remain_fixed() {
    let serial = h::invoke("check", h::SERIAL, &[]);
    assert_eq!(
        (serial.exit, serial.value["verdict"].as_str()),
        (0, Some("satisfied"))
    );
    h::leaf_numbers(&serial.value, 8, 8);
    assert_eq!(
        serial.value["bounds"],
        json!({"cases":1,"live_tasks":2,"clock":8,"truncated":false})
    );
    let cut = h::invoke("check", &h::SERIAL.replace("clock 8min", "clock 7min"), &[]);
    assert_eq!(
        (cut.exit, cut.value["verdict"].as_str()),
        (2, Some("inconclusive"))
    );
    assert_eq!(cut.value["bounds"]["truncated"], true);
    assert_eq!(cut.value["terminals"], json!([]));
    let sim = h::invoke("simulate", h::KEYS, &[]);
    assert_eq!(sim.exit, 0);
    assert_eq!(sim.value["simulation"]["completed_replications"], 4);
    assert_eq!(sim.value["simulation"]["seed"], 0);
    assert_eq!(sim.value["properties"][0]["verdict"], "satisfied");
    assert_eq!(sim.value["properties"][1]["verdict"], "satisfied");
}
#[test]
fn check_core_api_and_cli_own_the_same_complete_ledger() {
    let core = serde_json::to_value(relca_core::report::check(h::SERIAL)).unwrap();
    h::leaf_numbers(&core, 8, 8);
    h::ledger(&core, &h::serial_spec());
    let cli = h::invoke("check", h::SERIAL, &[]);
    h::ledger(&cli.value, &h::serial_spec());
    assert_eq!(cli.value, core);
    assert!(cli.stdout.starts_with(b"{\"ledger\":"));
    h::no_new_diagnostics(&core);
}
#[test]
fn check_explicit_cut_bounds_and_default_actor_version_survive_clock_cut() {
    let source = h::SERIAL.replace("clock 8min", "clock 7min");
    let v = h::invoke("check", &source, &[]);
    assert_eq!(v.exit, 2);
    assert_eq!(v.value["bounds"]["truncated"], true);
    h::ledger(&v.value, &h::serial_spec());
    assert_eq!(
        v.value["ledger"]["actor_versions"]["entries"][0]["via_model"],
        oracle::undeclared()
    );
}
#[test]
fn parser_and_lower_failure_do_not_invent_settings_or_actors() {
    let mut errors = Vec::new();
    for source in [
        "not a model",
        &h::SERIAL.replace("Office.long()", "Missing.long()"),
    ] {
        let v = h::invoke("check", source, &[]);
        assert_eq!(
            v.exit, 3,
            "source={source}; diagnostics={}",
            v.value["diagnostics"]
        );
        assert!(v.value["model"]["sha256"].is_string());
        assert!(v.value["bounds"]["clock"].is_null());
        h::record(&v.value, &Spec::prelower("exhaustive", false), &mut errors);
    }
    h::finish(errors);
}
#[test]
fn simulation_seed_zero_replications_confidence_and_no_finite_state_budget() {
    let v = h::invoke("simulate", h::KEYS, &[]);
    assert_eq!(v.exit, 0);
    h::ledger(&v.value, &Spec::simulation("P", &["A"], 0, 4, "3/4"));
    assert_eq!(v.value["simulation"]["interval_method"], "clopper-pearson");
    let core = serde_json::to_value(relca_core::report::simulate(h::KEYS)).unwrap();
    assert_eq!(v.value, core);
}
#[test]
fn exhaustive_does_not_claim_it_used_a_declared_simulation_seed() {
    let v = h::invoke("check", h::KEYS, &[]);
    h::ledger(&v.value, &Spec::finite("P", &["A"]));
    assert_eq!(
        v.value["ledger"]["execution"]["seed"],
        oracle::na("exhaustive_analysis")
    );
}
#[test]
fn simulation_parse_failure_and_missing_clause_do_not_default_seed() {
    let v = h::invoke("simulate", "not a model", &[]);
    assert_eq!(v.exit, 3);
    h::ledger(&v.value, &Spec::prelower("simulation", false));
    let v = h::invoke("simulate", h::SERIAL, &[]);
    assert_eq!(v.exit, 3);
    assert_eq!(v.value["diagnostics"][0]["code"], "E-SIM-0");
    h::ledger(&v.value, &Spec::prelower("simulation", false));
}
#[test]
fn simulation_late_abort_keeps_requested_four_not_completed_two() {
    let source =
        include_str!("../../../fixtures/qual-late-abort.broken-nondeterminism/model.relca");
    let v = h::invoke("simulate", source, &[]);
    assert_eq!(v.exit, 4);
    assert_eq!(v.value["simulation"]["attempted_replications"], 3);
    assert_eq!(v.value["simulation"]["completed_replications"], 2);
    assert_eq!(v.value["simulation"]["status"], "unsupported");
    h::ledger(&v.value, &Spec::simulation("P", &["A", "B"], 0, 4, "3/4"));
}
#[test]
fn via_actor_declaration_is_preserved_verbatim_not_split_at_at_sign() {
    let v = h::invoke("check", h::VIA, &[]);
    let mut s = Spec::finite("Flow", &["Invoice", "Extract", "Reader", "Judge", "Ledger"]);
    s.actors["entries"][1]["via_model"] = known("doc@v3");
    h::ledger(&v.value, &s);
    assert_eq!(v.value["actors"][1]["via_model"]["value"], "doc@v3");
}
#[test]
fn rule_versions_distinguish_declared_and_effective_configuration() {
    let mut errors = Vec::new();
    for (config, version, exit) in [("AsIs", "r1", 0), ("ToBeB", "r2", 1)] {
        let v = h::invoke("check", h::RULE, &["--configuration", config]);
        assert_eq!(v.exit, exit);
        assert_eq!(
            v.value["actors"][0]["rule"]["version"],
            json!({"declared":"r1","effective":version})
        );
        h::record(&v.value, &h::rule_spec(config, version), &mut errors);
    }
    h::finish(errors);
}
#[test]
fn explain_is_flattened_once_and_links_replay_scope() {
    let v = h::invoke("explain", h::SERIAL, &[]);
    assert_eq!(v.exit, 0);
    h::leaf_numbers(&v.value, 8, 8);
    assert_eq!(
        v.value["explanation"]["version"],
        "invoice-counterexample-v1"
    );
    let mut s = h::serial_spec();
    s.limits["capture_limit"] = known(200_000);
    h::ledger(&v.value, &s);
    assert!(v.value["explanation"].get("ledger").is_none());
    let core = serde_json::to_value(relca_core::explain::explain(h::SERIAL)).unwrap();
    assert_eq!(core, v.value);
}
#[test]
fn deterministic_repeat_has_no_timestamp_pid_or_absolute_build_identity() {
    let a = h::invoke("check", h::SERIAL, &[]);
    let b = h::invoke("check", h::SERIAL, &[]);
    assert_eq!((a.stdout, a.stderr, a.exit), (b.stdout, b.stderr, b.exit));
    h::ledger(&a.value, &h::serial_spec());
    assert_eq!(
        a.value["ledger"]["producer"]["build_identity"]["value"],
        Value::Null
    );
}
#[test]
fn scenario_selection_records_requested_and_effective_names_without_changing_bounds() {
    let source = include_str!("../../../fixtures/scn-static-config-scenario/model.relca");
    let v = h::invoke(
        "check",
        source,
        &["--configuration", "Fast", "--scenario", "Light"],
    );
    assert_eq!(v.exit, 0);
    assert_eq!(v.value["bounds"]["cases"], 3);
    assert_eq!(v.value["bounds"]["clock"], 30);
    let mut s = Spec::finite("Q", &["A"]);
    s.selection = oracle::selection(Some("Fast"), Some("Light"), "Q");
    h::ledger(&v.value, &s);
}
#[test]
fn responsibility_and_contest_local_versions_are_referenced_not_silently_omitted() {
    for (source, names) in [
        (
            include_str!("../../../fixtures/resp-origin/model.relca"),
            vec!["Invoice", "Reader", "Approver", "Ledger"],
        ),
        (
            include_str!("../../../fixtures/resp-contest/model.relca"),
            vec!["Invoice", "Reader", "Clerk", "Approver", "Vendor"],
        ),
    ] {
        let v = h::invoke("check", source, &[]);
        let s = Spec::finite("Flow", &names);
        assert!(v.value["responsibilities"].is_array() || v.value["contest_paths"].is_array());
        h::ledger(&v.value, &s);
    }
}
#[test]
fn green_hand_actor_order_and_version_expectations_match_input_declarations() {
    let cases = [
        (h::SERIAL, h::serial_spec()),
        (h::VIA, {
            let mut s = Spec::finite("Flow", &["Invoice", "Extract", "Reader", "Judge", "Ledger"]);
            s.actors["entries"][1]["via_model"] = known("doc@v3");
            s
        }),
    ];
    for (src, s) in cases {
        let v = serde_json::to_value(relca_core::report::check(src)).unwrap();
        let entries = s.actors["entries"].as_array().unwrap();
        assert_eq!(v["actors"].as_array().unwrap().len(), entries.len());
        for (row, hand) in v["actors"].as_array().unwrap().iter().zip(entries) {
            assert_eq!(row["name"], hand["actor"]);
            if hand["via_model"]["status"] == "known" {
                assert_eq!(row["via_model"]["value"], hand["via_model"]["value"]);
            }
        }
        oracle::expected(&v,&s).expect("hand input and closed local version registry must be usable before production ledger exists");
    }
}
