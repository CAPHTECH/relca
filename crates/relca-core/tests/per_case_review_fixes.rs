//! ADR 0030 acceptance that kills the per-case mutation survivors.
//! Expectations are derived by hand from the per-case model (not copied from runs).
use relca_core::{
    diag::Diagnostic, engine, explain, ir, lower, outcome::Outcome, reference, report, syntax,
};
use serde_json::{Value, json};
#[path = "support/percase_acceptance.rs"]
mod p;

const FORBIDDEN: &str = "per_case Actor cannot declare capacity, executors or Capability";
const SHARED: &str = "multi-case Actor latent state must be fixed and read-only";

fn replaced(src: &str, from: &str, to: &str) -> String {
    let out = src.replace(from, to);
    assert_ne!(out, src, "replacement of {from:?} did not happen");
    out
}
fn lower_err(s: &str) -> Vec<Diagnostic> {
    syntax::parse(s)
        .and_then(|a| lower::lower(&a))
        .expect_err("input must be rejected statically")
}
fn has_usem(out: &Outcome) -> bool {
    out.diagnostics.iter().any(|d| d.code == "U-SEM-0")
}
fn invoice(m: &ir::Model) -> usize {
    m.actors.iter().position(|a| a.name == "Invoice").unwrap()
}

/// Q08: the nondeterminism diagnostic names the per_case origins by the
/// 1-based arrival id, in both engine and reference simulate.
#[test]
fn q08_nondet_diagnostic_case_ids_are_arrival_ids_in_engine_and_reference() {
    let s = replaced(
        &p::source("pcase-nondet"),
        "  bounds",
        "  simulation replications 4 seed 7 confidence 0.95\n  bounds",
    );
    let m = p::model(&s);
    let e = serde_json::to_value(engine::simulate(&m)).unwrap();
    let r = serde_json::to_value(reference::simulate(&m).outcome).unwrap();
    for (who, v) in [("engine", &e), ("reference", &r)] {
        let ds = v["diagnostics"].as_array().unwrap();
        let msg = ds
            .iter()
            .find(|d| d["code"] == "U-SEM-0")
            .unwrap_or_else(|| panic!("{who}: no U-SEM-0 in {ds:?}"))["message"]
            .as_str()
            .unwrap();
        assert!(
            msg.contains("cases [1,2]") && msg.contains("sites [init.actor.Invoice.difficulty]"),
            "{who}: {msg}"
        );
        assert!(!msg.contains("cases [0"), "{who}: {msg}");
    }
}

/// Q11: the public-IR guard rejects capacity on a per_case Actor even when
/// the declaration flag is absent (runtime guard, not lower only).
#[test]
fn q11_engine_rejects_public_ir_capacity_without_declared_flag() {
    let mut m = p::model(&p::source("pcase-capacity"));
    let doc = invoice(&m);
    m.actors[doc].capacity = Some(1);
    assert!(!m.actors[doc].metadata.capacity_declared);
    let e = engine::explore(&m);
    assert!(
        e.diagnostics
            .iter()
            .any(|d| d.code == "U-SEM-0" && d.message == FORBIDDEN),
        "engine: {:?}",
        e.diagnostics
    );
}

/// Q11b: the independent reference guard for the same public IR.
#[test]
fn q11b_reference_rejects_public_ir_capacity_without_declared_flag() {
    let mut m = p::model(&p::source("pcase-capacity"));
    let doc = invoice(&m);
    m.actors[doc].capacity = Some(1);
    assert!(!m.actors[doc].metadata.capacity_declared);
    let r = reference::explore(&m);
    assert_eq!(r.stop, reference::StopReason::SemanticAbort);
    assert!(
        has_usem(&r.outcome),
        "reference: {:?}",
        r.outcome.diagnostics
    );
}

/// Q13: a 0-minute, unused Capability owned by a per_case Actor in public IR
/// is refused by both runtimes; the same IR with the
/// Capability on a capacity Actor is accepted (control).
#[test]
fn q13_public_ir_capability_owned_by_per_case_actor_is_refused_by_both() {
    let s = replaced(
        &p::source("pcase-capability"),
        "  process Flow",
        "  actor Helper { capacity 1; capability unused() -> T { duration fixed 0min; result nondet { ok } } }\n  process Flow",
    );
    let s = replaced(&s, "include [Invoice]", "include [Invoice,Helper]");
    let good = p::model(&s);
    let e0 = engine::explore(&good);
    let r0 = reference::explore(&good);
    assert!(!has_usem(&e0), "control engine: {:?}", e0.diagnostics);
    assert_eq!(r0.stop, reference::StopReason::Complete);
    assert!(
        !has_usem(&r0.outcome),
        "control reference: {:?}",
        r0.outcome.diagnostics
    );
    let mut m = good.clone();
    let doc = invoice(&m);
    let cap = m
        .capabilities
        .iter()
        .position(|c| c.name == "unused")
        .unwrap();
    m.capabilities[cap].actor = ir::ActorId(doc);
    let e = engine::explore(&m);
    assert!(has_usem(&e), "engine: {:?}", e.diagnostics);
    let r = reference::explore(&m);
    assert_eq!(r.stop, reference::StopReason::SemanticAbort, "reference");
    assert!(
        has_usem(&r.outcome),
        "reference: {:?}",
        r.outcome.diagnostics
    );
}

/// Q15: the new rows are appended after the existing static rows of lower
/// (the old first row stays first, preserving stable order).
#[test]
fn q15_instance_rows_are_appended_after_existing_lower_assumptions() {
    let m = p::model(&p::source("pcase-common"));
    let fields: Vec<_> = m.assumptions.iter().map(|a| a.field.as_str()).collect();
    assert_eq!(fields[0], "Invoice.boundary", "{fields:?}");
    let i = fields
        .iter()
        .position(|f| *f == "Invoice.instance")
        .unwrap();
    assert_eq!(
        &fields[i - 1..i + 2],
        &[
            "kernel.U",
            "Invoice.instance",
            "initial.actor_case_independence"
        ],
        "{fields:?}"
    );
}

/// Q16: a per_case latent without a Distribution (Nondet only) gets the
/// instance row but no independence row.
#[test]
fn q16_nondet_only_per_case_latent_has_no_independence_row() {
    let v = serde_json::to_value(report::check(&p::source("pcase-nondet"))).unwrap();
    p::assumptions(&v, &["Invoice"], false);
}

/// Q18: the forbidden-clause diagnostic carries the clause span, not the
/// whole Actor; each violating clause has its own span.
#[test]
fn q18_forbidden_clause_span_is_the_clause_only() {
    for (name, prefix) in [
        ("pcase-capacity.broken-capacity", "capacity 0"),
        ("pcase-executors.broken-executors", "executors [p]"),
        ("pcase-capability.broken-capability", "capability unused()"),
    ] {
        let s = p::source(name);
        let ds = lower_err(&s);
        let d = ds
            .iter()
            .find(|d| d.message == FORBIDDEN)
            .unwrap_or_else(|| panic!("{name}: {ds:?}"));
        let sp = d.span.unwrap();
        let text = &s[sp.start..sp.end];
        assert!(text.starts_with(prefix), "{name}: {text}");
        assert!(!text.contains("latent difficulty"), "{name}: {text}");
    }
}

/// Q20: `instance` with no value directly before `}` is E-SYN-0, not U-SYN-0
/// (an empty value is E-SYN-0).
#[test]
fn q20_instance_without_value_before_brace_is_e_syn() {
    let s = replaced(&p::source("pcase-capacity"), "instance per_case; ", "");
    let s = replaced(
        &s,
        "Difficulty = routine }",
        "Difficulty = routine; instance }",
    );
    let ds = lower_err(&s);
    assert!(ds.iter().any(|d| d.code == "E-SYN-0"), "{ds:?}");
    assert!(!ds.iter().any(|d| d.code == "U-SYN-0"), "{ds:?}");
}

/// Q21: a per_case Actor writing its own latent is refused by the per_case
/// rule (Capability on the document) and the existing capacity rule only;
/// the shared multi-case rule is for unmarked Actors.
/// The verdict is the same either way; the diagnostic set is the contract.
#[test]
fn q21_per_case_own_write_is_not_reported_by_the_shared_rule() {
    let s = replaced(
        &p::source("pcase-joint"),
        "actor Invoice { instance per_case;",
        "kernel W { * -> { 1: ok with difficulty = difficult } }\n  actor Invoice { instance per_case; capability mutate() -> T { writes difficulty; duration fixed 1min; outcomes kernel W };",
    );
    let ds = lower_err(&s);
    let mut got: Vec<_> = ds
        .iter()
        .map(|d| (d.code.to_string(), d.message.clone()))
        .collect();
    got.sort();
    got.dedup();
    let codes: Vec<_> = got.iter().map(|(c, _)| c.as_str()).collect();
    assert_eq!(codes, ["E-CAP-0", "U-SEM-0"], "{got:?}");
    assert!(got.iter().any(|(_, m)| m == FORBIDDEN), "{got:?}");
    assert!(!got.iter().any(|(_, m)| m == SHARED), "{got:?}");
}

/// Q26: equal-named locals on another per_case Actor on the single-case
/// legacy replay path; the replay matches values by declaration, so the
/// counterexample is verified by replay with the same local variable name.
#[test]
fn q26_single_case_equal_names_legacy_replay_is_verified() {
    let base = replaced(
        &p::source("pcase-explain"),
        "include [Invoice,Reader]",
        "include [Invoice,Reader,Other]",
    );
    let base = replaced(&base, "cases 2", "cases 1");
    let base = replaced(&base, "  arrivals fixed [0min, 0min]\n", "");
    let base = replaced(&base, "  policy fifo tie_by case_id\n", "");
    let other = "  actor Other { instance per_case; latent difficulty: bool = true }\n";
    for (order, src) in [
        (
            "other after Invoice",
            replaced(&base, "  process Flow", &format!("{other}  process Flow")),
        ),
        (
            "other before Invoice",
            replaced(
                &base,
                "  actor Invoice {",
                &format!("{other}  actor Invoice {{"),
            ),
        ),
    ] {
        let v: Value = serde_json::to_value(explain::explain(&src)).unwrap();
        assert_eq!(v["verdict"], "violated", "{order}: {v}");
        assert_eq!(
            v["explanation"]["counterexamples"][0]["replay"]["status"],
            json!("verified"),
            "{order}: {}",
            v["explanation"]["counterexamples"][0]["replay"]
        );
    }
}
