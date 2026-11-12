//! Hand-derived structural and fail-closed checks.
#[path = "support/agency_limit_inputs.rs"]
mod input;
use relca_core::{agency, diag, ir, lower, syntax};
use serde_json::{Value, json};

fn lowered(s: &str) -> ir::Model {
    lower::lower(&syntax::parse(s).expect("valid syntax")).expect("valid finite model")
}

fn refusal(a: &Value, n: usize, start: usize, stops: &str, bypasses: u64) {
    let expected = (start..start + n).map(|i| json!({"call":format!("j{i}"),"capability":"Judge.check","stops":stops,"bypasses":bypasses})).collect::<Vec<_>>();
    assert_eq!(
        a["refusal"],
        json!([{"effect":"Ledger.post","call":"p","reachable":true,"judgements":expected}])
    );
    assert_eq!(a["lineage"].as_array().unwrap().len(), n);
}

#[test]
fn every_input_reaches_agency_not_a_syntax_name_or_type_error() {
    for (name, s, configuration) in input::all() {
        let ast = syntax::parse(&s).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        match lower::lower_with_selection(&ast, configuration, None) {
            Ok(m) => assert!(!m.process.bindings.is_empty(), "{name}"),
            Err(ds) => {
                let errors = ds
                    .iter()
                    .filter(|d| d.severity == diag::Severity::Error)
                    .collect::<Vec<_>>();
                assert_eq!(errors.len(), 1, "{name}: {ds:?}");
                assert_eq!(errors[0].code, "U-SEM-0", "{name}: {ds:?}");
                assert!(
                    errors[0].message == "Agency derivation limit exceeded or invalid static IR"
                        || errors[0]
                            .message
                            .starts_with("Agency derivation limit exceeded:"),
                    "{name}: must reach the Agency derivation, {ds:?}"
                );
            }
        }
    }
}

#[test]
fn control_thirteen_serial_gates_each_stop_the_effect() {
    let a = json!(agency::derive(&lowered(&input::gate(13))).unwrap().0);
    refusal(&a, 13, 1, "all_paths", 0);
}

#[test]
fn twelve_requires_fit_the_exact_path_budget() {
    let m = lowered(&input::requires(12));
    let a = json!(agency::derive(&m).unwrap().0);
    assert_ne!(a["status"], "not_derived");
    assert_eq!(a["refusal"], json!([]));
    assert!(
        a["unchecked"]
            .as_array()
            .unwrap()
            .contains(&json!("state_guards"))
    );
}

macro_rules! serial {
    ($name:ident, $k:literal) => {
        #[test]
        fn $name() {
            let a = json!(agency::derive(&lowered(&input::gate($k))).unwrap().0);
            refusal(&a, $k, 1, "all_paths", 0);
        }
    };
}
serial!(independent_gates_fourteen_derive, 14);
serial!(independent_gates_forty_derive, 40);
serial!(independent_gates_two_hundred_derive, 200);

#[test]
fn true_and_twenty_has_twenty_independent_stopping_judgements() {
    let a = json!(
        agency::derive(&lowered(&input::joined(20, " && ")))
            .unwrap()
            .0
    );
    refusal(&a, 20, 1, "all_paths", 0);
}

#[test]
fn chain_has_j_zero_through_j_986_not_986_rows() {
    let a = json!(agency::derive(&lowered(&input::chain(986))).unwrap().0);
    refusal(&a, 987, 0, "none", 1);
    let expected = (0..=986).map(|i| json!({"call":format!("j{i}"),"capability":"Judge.check","subject_sources":["Doc"],"independent_sources":[]})).collect::<Vec<_>>();
    assert_eq!(a["lineage"], json!(expected));
    assert_eq!(
        a["lineage_groups"],
        json!([{"subject_sources":["Doc"],"judgements":987,"actors":1,"independent_sources":[]}])
    );
}

#[test]
fn or_is_not_split_into_independent_conjuncts() {
    for (k, bypasses) in [(3, 3), (10, 511)] {
        let a = json!(
            agency::derive(&lowered(&input::joined(k, " || ")))
                .unwrap()
                .0
        );
        refusal(&a, k, 1, "conditional", bypasses);
    }
}

#[test]
fn continuing_branches_have_four_bypasses_and_no_stopping_power() {
    let a = json!(agency::derive(&lowered(&input::continuing(3))).unwrap().0);
    refusal(&a, 3, 1, "none", 4);
}

fn invalid_error(m: &ir::Model) -> diag::Diagnostic {
    let error = agency::derive(m).expect_err("invalid IR must not become an uncomputed Agency");
    assert_eq!(error.code, "U-SEM-0");
    assert_eq!(error.severity, diag::Severity::Error);
    error
}

#[test]
fn corrupt_indices_duplicates_and_cycles_still_fail_closed() {
    let base = lowered(&input::gate(1));
    for fault in 0..4 {
        let mut bad = base.clone();
        let ir::Stmt::Call(c) = &mut bad.process.body[1].kind else {
            panic!()
        };
        match fault {
            0 => c.args[0] = ir::BindingId(usize::MAX - 1),
            1 => c.capability = ir::CapabilityId(usize::MAX),
            2 => c.args[0] = c.binding,
            _ => {
                let repeated = bad.process.body[1].clone();
                bad.process.body.push(repeated);
            }
        }
        invalid_error(&bad);
    }
}

#[test]
fn invalid_ir_has_a_distinct_reason_not_a_limit_message() {
    let mut bad = lowered(&input::gate(1));
    bad.process.body.push(bad.process.body[1].clone());
    let e = invalid_error(&bad);
    assert!(
        e.message.starts_with("invalid static IR for Agency:"),
        "{e:?}"
    );
    assert!(e.message.contains("duplicate"), "{e:?}");
    assert!(!e.message.contains("limit exceeded"), "{e:?}");
}

#[test]
fn structural_oracles_reject_single_input_faults_then_accept_the_original() {
    // Source faults, not altered expected numbers: bypass a gate, turn Or into
    // And, and turn continuing else arms into terminal rejection.
    for (good, bad, n, stops, bypasses) in [
        (
            input::gate(3),
            input::gate(3).replacen("else { end rejected }", "else { }", 1),
            3,
            "all_paths",
            0,
        ),
        (
            input::joined(3, " || "),
            input::joined(3, " && "),
            3,
            "conditional",
            3,
        ),
        (input::continuing(3), input::gate(3), 3, "none", 4),
    ] {
        let broken = json!(agency::derive(&lowered(&bad)).unwrap().0);
        assert!(std::panic::catch_unwind(|| refusal(&broken, n, 1, stops, bypasses)).is_err());
        let original = json!(agency::derive(&lowered(&good)).unwrap().0);
        refusal(&original, n, 1, stops, bypasses);
    }
}
