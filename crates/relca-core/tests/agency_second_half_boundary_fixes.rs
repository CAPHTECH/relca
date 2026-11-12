//! Boundary review of ADR 0043 (c5) m5: the Viewpoints wire is closed on the
//! rule version. p1-1 has the four blocks only; p1-2 requires effects. Each
//! rejection is caused by the one changed field (positive controls).
//!
//! Boundary m1: W-RESP-0
//! for an owner that refuses only by version_check (ADR 0041). The expected
//! diagnostics are written by hand from ADR 0043's W-RESP-0 rule (U12), not
//! copied from output.
use relca_core::{agency::Viewpoints, lower, report, syntax};
use serde_json::{Value, json};

fn viewpoints() -> Value {
    let src = std::fs::read_to_string(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/agy2-jue/model.relca"),
    )
    .unwrap();
    let v = serde_json::to_value(report::check(&src)).unwrap();
    let vp = v["agency"]["viewpoints"].clone();
    assert_eq!(vp["rules"], "agency-viewpoints-p1-2");
    assert!(vp.get("effects").is_some());
    vp
}
fn read(v: Value) -> Result<Viewpoints, String> {
    serde_json::from_value::<Viewpoints>(v).map_err(|e| e.to_string())
}

#[test]
fn m5_p1_1_with_effects_is_rejected() {
    let mut v = viewpoints();
    v["rules"] = json!("agency-viewpoints-p1-1");
    assert!(read(v).is_err(), "p1-1 must not carry effects");
}

#[test]
fn m5_controls_p1_2_with_effects_and_p1_1_without_are_accepted() {
    read(viewpoints()).unwrap();
    let mut old = viewpoints();
    old.as_object_mut().unwrap().remove("effects");
    old["rules"] = json!("agency-viewpoints-p1-1");
    read(old).unwrap();
    let mut missing = viewpoints();
    missing.as_object_mut().unwrap().remove("effects");
    assert!(read(missing).is_err(), "p1-2 requires effects");
}

fn version_check_source() -> String {
    std::fs::read_to_string(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/agy2-version-check/model.relca"),
    )
    .unwrap()
}
fn resp0_messages(src: &str) -> Vec<String> {
    let m = lower::lower(&syntax::parse(src).unwrap()).unwrap();
    m.diagnostics
        .iter()
        .filter(|d| d.code == "W-RESP-0")
        .map(|d| d.message.clone())
        .collect()
}

/// Hand (ADR 0043 U12): the model has `people [p]`, so the people condition
/// holds, and no responsibility is declared, which no longer silences the
/// warning. Ledger owns `version_check [n]` on post, so it is the effect's
/// state refuser (the M1 ruling: the version_check owner is a state refuser,
/// as for atomic) and Ledger is the effect's Actor; Ledger is the only Actor.
/// Ledger has no executors, so no executor holds a duty: exactly one W-RESP-0,
/// on Ledger. Control: the same input with executors on Ledger and
/// `responsible p for Ledger.post` gives an executor a duty, so no W-RESP-0.
/// Ledger keeps `capacity 2`, which must equal the executor count (E-CAP-0),
/// so the control declares `people [p, q]` and `executors [p, q]`.
#[test]
fn post_integration_m1_version_check_owner_without_duty_gets_one_resp0() {
    let src = version_check_source();
    assert_eq!(
        resp0_messages(&src),
        ["Ledger has no declared responsibility"]
    );
    let declared = src
        .replace("people [p]", "people [p, q]")
        .replace(
            "actor Ledger { capacity 2;",
            "actor Ledger { executors [p, q]; capacity 2;",
        )
        .replace(
            "  process P {",
            "  responsible p for Ledger.post\n  process P {",
        );
    assert_eq!(declared.matches("people [p, q]").count(), 1);
    assert_eq!(declared.matches("executors [p, q]").count(), 1);
    assert_eq!(declared.matches("responsible p for Ledger.post").count(), 1);
    assert_eq!(resp0_messages(&declared), Vec::<String>::new());
}
