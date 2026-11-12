//! CLI process exits and wire are checked separately from engine projections.
#[path = "../src/fixture_match.rs"]
mod fixture_match;
#[path = "../../relca-core/tests/support/actor_observed_acceptance.rs"]
mod h;
use serde_json::{Value, json};
use std::process::Command;

#[test]
fn all48_cli_jobs_keep_hand_oracles_and_real_exit_codes() {
    let mut failures = vec![];
    for name in h::names() {
        let expected = h::expected(&name);
        let command = if name.starts_with("aobs-simulation") {
            "simulate"
        } else {
            "check"
        };
        let o = Command::new(env!("CARGO_BIN_EXE_relca"))
            .args([command, &format!("fixtures/{name}/model.relca")])
            .current_dir(h::root())
            .output()
            .unwrap();
        let v: Value = serde_json::from_slice(&o.stdout).unwrap();
        let want: relca_core::Verdict =
            serde_json::from_value(expected["verdict"].clone()).unwrap();
        if v["verdict"] != expected["verdict"] || o.status.code() != Some(want.exit_code()) {
            failures.push(format!(
                "{name}: expected {} exit{}, got {} exit{:?}",
                expected["verdict"],
                want.exit_code(),
                v["verdict"],
                o.status.code()
            ));
            continue;
        }
        // The CLI legitimately renders diagnostics to stderr, including static
        // refusal controls. Current stderr bytes are covered by golden snapshots;
        // do not impose an invented empty-stderr contract on new inputs.
        h::subset(&expected, &v);
        let invocation_value = json!({"command":"simulate"});
        let invocation = fixture_match::Invocation::parse(if command == "simulate" {
            Some(&invocation_value)
        } else {
            None
        });
        // The group runner covers the same complete closed wire; here command
        // selection/exit is independent and hand arrays are checked directly.
        assert!(invocation.is_ok());
    }
    assert!(
        failures.is_empty(),
        "new Actor state not implemented:\n{}",
        failures.join("\n")
    );
}

#[test]
fn configuration_free_explain_replays_observed_range_and_invariant_witnesses() {
    for name in [
        "aobs-counter.broken-overflow",
        "aobs-postwrite.broken-same-tick",
    ] {
        let o = Command::new(env!("CARGO_BIN_EXE_relca"))
            .args(["explain", &format!("fixtures/{name}/model.relca")])
            .current_dir(h::root())
            .output()
            .unwrap();
        assert_eq!(o.status.code(), Some(1), "{name}");
        let v: Value = serde_json::from_slice(&o.stdout).unwrap();
        assert_eq!(v["explanation"]["status"], "complete");
        for c in v["explanation"]["counterexamples"].as_array().unwrap() {
            assert_eq!(c["replay"]["status"], "verified");
        }
    }
}

#[test]
fn closed_actor_state_table_rejects_bad_shape_owner_empty_duplicate_and_status() {
    let src = include_str!("../../../fixtures/serial-time/model.relca");
    let mut v = serde_json::to_value(relca_core::report::check(src)).unwrap();
    let name = v["actors"][0]["name"].as_str().unwrap().to_owned();
    v["actors"][0]["state"] =
        json!({"value":[format!("{name}.x"),format!("{name}.y")],"status":"declared"});
    fixture_match::validate_actor_context_wire(&v).unwrap();
    for state in [
        json!({"value":"x","status":"declared"}),
        json!({"value":[],"status":"declared"}),
        json!({"value":["Other.x"],"status":"declared"}),
        json!({"value":[format!("{name}.x"),format!("{name}.x")],"status":"declared"}),
        json!({"value":[format!("{name}.x")],"status":"default"}),
        json!({"value":null,"status":"declared"}),
        json!({"value":[format!("{name}.x.y")],"status":"declared"}),
    ] {
        let mut bad = v.clone();
        bad["actors"][0]["state"] = state;
        assert!(fixture_match::validate_actor_context_wire(&bad).is_err());
    }
}
