//! Independent execution acceptance; all source-set verdicts are handwritten.
use relca_core::{Verdict, engine, ir, lower, reference, syntax};
use serde_json::Value;
use std::{process::Command, sync::OnceLock};

fn cases() -> &'static Value {
    static CASES: OnceLock<Value> = OnceLock::new();
    CASES.get_or_init(|| {
        let out = Command::new("python3")
            .args(["-B", "crates/relca-core/tests/record_provenance/inputs.py"])
            .current_dir(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
            .output()
            .unwrap();
        assert!(out.status.success(), "{:?}", out);
        serde_json::from_slice(&out.stdout).unwrap()
    })
}

fn check(name: &str, key: &str) {
    let row = &cases()[name];
    let mut source = row[key].as_str().unwrap().to_owned();
    // Explicit, reversible harness fault for the initially GREEN legacy half.
    if std::env::var_os("RELCA_RECORD_SUBSTRATE_FAULT").is_some() {
        source = source.replace("type V =", "type V ??? =");
    }
    let parsed = syntax::parse(&source).unwrap_or_else(|e| panic!("{name}/{key} parse: {e:?}"));
    let m = lower::lower(&parsed).unwrap_or_else(|e| panic!("{name}/{key} lower: {e:?}"));
    let expected: Verdict = serde_json::from_value(row["expected"].clone()).unwrap();
    let actual = engine::explore(&m);
    let independent = reference::explore(&m);
    if expected != Verdict::Unsupported {
        assert_eq!(independent.stop, reference::StopReason::Complete);
    }
    for (implementation, outcome) in [("engine", &actual), ("reference", &independent.outcome)] {
        let properties: Vec<_> = outcome
            .properties
            .iter()
            .filter(|p| matches!(p.property, ir::Property::Contract(_)))
            .collect();
        assert_eq!(properties.len(), 1, "{implementation}/{name}: one Basis");
        assert_eq!(properties[0].verdict, expected, "{implementation}/{name}");
        assert!(!outcome.truncated, "{implementation}/{name}: not a cut");
        if expected != Verdict::Unsupported {
            assert!(
                !outcome
                    .diagnostics
                    .iter()
                    .any(|d| d.severity == relca_core::diag::Severity::Error)
            );
        }
    }
    assert_eq!(actual.properties, independent.outcome.properties);
    if expected == Verdict::Unsupported {
        // The wire/report layer attaches the conditional reason; raw engine
        // outcomes carry a property verdict without that diagnostic.
        let report = serde_json::to_value(relca_core::report::check(&source)).unwrap();
        assert!(report["diagnostics"].as_array().unwrap().iter().any(|d| {
            d["code"] == "U-SEM-0"
                && d["message"]
                    .as_str()
                    .unwrap()
                    .contains("evidence_basis_conditional_snapshot")
        }));
    }
}

macro_rules! acceptance {
    ($module:ident) => {
        mod $module {
            #[test]
            fn new_registration_engine_and_reference() {
                super::check(stringify!($module), "source");
            }
            #[test]
            fn old_snapshot_equivalent_engine_and_reference() {
                super::check(stringify!($module), "legacy");
            }
        }
    };
}
acceptance!(a1_r0);
acceptance!(a1_registrar_blind);
acceptance!(a1_direct);
acceptance!(a2_dominating);
acceptance!(a2_conditional);
acceptance!(a2_never);
acceptance!(a3_single_result);
acceptance!(a4_overwrite_before);
acceptance!(a4_overwrite_after);
acceptance!(a4_overwrite_maybe);
acceptance!(a5_copy_of_copy);
acceptance!(f5_latent_not_observation);
acceptance!(f5_latent_with_direct_control);
acceptance!(f8_requires);
acceptance!(u4_relayed_argument);
acceptance!(u4_relayed_snapshot);
acceptance!(u2_multiple_targets);
acceptance!(u4_control_not_data);
acceptance!(u7_same_source_conditional_overwrite);

fn reconstruct(key: &str) {
    for name in [
        "a1_r0",
        "a1_registrar_blind",
        "a4_overwrite_after",
        "a5_copy_of_copy",
    ] {
        let row = &cases()[name];
        let mut source = row[key].as_str().unwrap().to_owned();
        if std::env::var_os("RELCA_RECORD_SUBSTRATE_FAULT").is_some() {
            source = source.replace("type V =", "type V ??? =");
        }
        let mut m = lower::lower(&syntax::parse(&source).unwrap()).unwrap();
        let ir::ContractKind::EvidenceBasis { calls, .. } = &mut m.contracts[0].kind else {
            panic!("Basis must be the evidence contract");
        };
        calls.clear();
        let independent = reference::explore(&m);
        assert_eq!(independent.stop, reference::StopReason::Complete);
        let want: Verdict = serde_json::from_value(row["expected"].clone()).unwrap();
        let property = independent
            .outcome
            .properties
            .iter()
            .find(|p| matches!(p.property, ir::Property::Contract(_)))
            .unwrap();
        assert_eq!(property.verdict, want, "independent reconstruction: {name}");
        assert!(
            engine::explore(&m)
                .diagnostics
                .iter()
                .any(|d| d.code == "U-SEM-0")
        );
    }
}

#[test]
fn new_registration_reference_does_not_trust_the_engine_evidence_table() {
    reconstruct("source");
}

#[test]
fn old_snapshot_equivalent_reference_does_not_trust_the_engine_evidence_table() {
    reconstruct("legacy");
}
