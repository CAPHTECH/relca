//! ADR 0072: integer/rational hand oracles, not values recorded from a binary.
#[path = "support/agency_viewpoint_limit_inputs.rs"]
mod input;
use relca_core::{ir, lower, syntax};
use serde_json::{Value, json};

fn model(source: &str) -> ir::Model {
    lower::lower(&syntax::parse(source).expect("existing DSL must parse"))
        .unwrap_or_else(|ds| panic!("existing DSL must lower: {ds:?}\n{source}"))
}
fn vp(source: &str) -> Value {
    let m = model(source);
    let a = m.agency.as_derived().expect("main Agency must derive");
    json!(a.viewpoints.as_ref().expect("viewpoints"))
}

#[test]
fn preflight_every_synthetic_input_is_existing_dsl_and_reaches_lower() {
    let mut sources = input::differential_models();
    for k in [10, 11, 12, 20, 24] {
        sources.push((format!("gate-{k}"), input::gate(k)));
    }
    for (s, w) in [(5, 2), (2, 4), (6, 4)] {
        sources.push((format!("stage-{s}-{w}"), input::stage(s, w, "and")));
    }
    sources.extend([
        ("violation".into(), input::violation()),
        ("simulation".into(), input::simulation()),
        ("rederivation".into(), input::rederivation()),
    ]);
    for (name, source) in sources {
        let m = model(&source);
        assert!(!m.process.body.is_empty(), "{name}");
        assert!(
            m.diagnostics
                .iter()
                .all(|d| d.severity != relca_core::diag::Severity::Error),
            "{name}: {:?}",
            m.diagnostics
        );
    }
}

#[test]
fn control_gate_four_and_ten_match_the_hand_table() {
    for k in [4, 10] {
        input::assert_gate(&vp(&input::gate(k)), k);
    }
}

#[test]
fn s1_gate_eleven_retention_is_one_through_eleven() {
    input::assert_gate(&vp(&input::gate(11)), 11);
}

#[test]
fn s2_gate_twenty_derives_with_default_limits() {
    input::assert_gate(&vp(&input::gate(20)), 20);
}

#[test]
fn stage_five_by_two_and_two_by_four_match_the_hand_table() {
    for (s, w) in [(5, 2), (2, 4)] {
        input::assert_stage(&vp(&input::stage(s, w, "and")), s, w);
    }
}

#[test]
fn g1_state_taint_is_not_a_zero_through_one_count() {
    let v = vp(&input::state_taint());
    let rows = v["retention"]["capabilities"].as_array().unwrap();
    let post = rows
        .iter()
        .find(|r| r["capability"] == "Ledger.post")
        .unwrap();
    assert_eq!(post["status"], "not_derived");
    assert_eq!(post["reason"], "state_guards");
    assert!(post.get("normal").is_none());
    assert_eq!(input::count(&v, "Intake.start"), json!({"min":1,"max":1}));
}

#[test]
fn g2_unsatisfiable_sink_cannot_increase_the_retention_maximum() {
    assert_eq!(
        input::count(&vp(&input::contradiction()), "Ledger.post"),
        json!({"min":0,"max":0})
    );
    let broken = input::contradiction().replace("j1==pass && j1==fail", "j1==pass || j1==fail");
    assert_eq!(
        input::count(&vp(&broken), "Ledger.post"),
        json!({"min":1,"max":1})
    );
}

#[test]
fn version_check_dsl_preserves_state_guard_withdrawal() {
    let v = vp(&input::version_domain());
    for end in v["framing"]["ends"].as_array().unwrap() {
        assert_eq!(end["status"], "not_derived");
        assert_eq!(end["reason"], "state_guards");
    }
}

#[test]
fn g4_or_and_negated_and_preserve_joint_contexts() {
    for (op, done, rejected) in [
        (" || ", "some_contexts", "all_contexts"),
        (" && ", "all_contexts", "some_contexts"),
    ] {
        let v = vp(&input::joined(3, op));
        assert_eq!(
            input::framing(&v, "done"),
            json!({"j1":done,"j2":done,"j3":done})
        );
        assert_eq!(
            input::framing(&v, "rejected"),
            json!({"j1":rejected,"j2":rejected,"j3":rejected})
        );
    }
}

#[test]
fn old_identifier_spellings_are_not_reserved_by_a_cli_option() {
    let s = input::gate(4)
        .replace("ViewpointLimit", "agency_limit_scale")
        .replace("Judge", "framing")
        .replace("Ledger", "retention")
        .replace("j1", "invalid_ir");
    let m = model(&s);
    assert_eq!(m.process.bindings[1].name, "invalid_ir");
    assert!(m.capabilities.iter().any(|c| c.name == "check"));
}

#[test]
fn static_rejection_of_hidden_state_is_not_weakened_by_viewpoint_changes() {
    let source = input::state_taint().replace("observed case", "latent case");
    let ds = lower::lower(&syntax::parse(&source).unwrap()).expect_err("latent flow condition");
    let errors = ds
        .iter()
        .filter(|d| d.severity == relca_core::diag::Severity::Error)
        .collect::<Vec<_>>();
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(errors[0].code, "E-POL-LATENT");
    assert_eq!(errors[0].message, "policy cannot read latent state");
}

#[test]
fn private_acceptance_adapter_is_connected() {
    // This is a connection gate, not evidence of equivalence. The runner also
    // executes the named lib tests and rejects an empty test selection.
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/agency.rs"),
    )
    .unwrap();
    assert!(
        source.contains("agency_viewpoint_limit_private.rs"),
        "missing cfg(test) agency adapter; see agency_viewpoint_limit_calculations.md"
    );
}
