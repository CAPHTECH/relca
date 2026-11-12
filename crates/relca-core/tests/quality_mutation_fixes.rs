//! ADR 0045 mutation survivors (P13, P35, P35r, O15, O14e, P34, P34r): the
//! engine and the independent reference both against hand JSON, and the
//! public IR entrances against a membership quality target. Inputs and hand
//! values are in support/quality_mutation_inputs.rs (shared with the CLI).
#[path = "support/quality_mutation_inputs.rs"]
mod inputs;
use relca_core::{diag, engine, ir, lower, reference, syntax};

fn lowered(src: &str) -> ir::Model {
    lower::lower(&syntax::parse(src).unwrap()).unwrap()
}

/// The reference recurses once per step; the twenty-step chain needs more
/// than the default test thread stack in a debug build.
fn deep<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new()
        .stack_size(256 << 20)
        .spawn(f)
        .unwrap()
        .join()
        .unwrap()
}

#[test]
fn engine_and_reference_quality_match_hand_values_collect_all() {
    deep(engine_and_reference_quality_match_hand_values);
}
fn engine_and_reference_quality_match_hand_values() {
    let mut failures = vec![];
    for input in inputs::all() {
        let m = lowered(&input.source);
        let (e, r) = if input.command == "simulate" {
            (
                engine::simulate(&m).quality,
                reference::simulate(&m).outcome.quality,
            )
        } else {
            (
                engine::explore(&m).quality,
                reference::explore(&m).outcome.quality,
            )
        };
        for (side, got) in [("engine", e), ("reference", r)] {
            let got = serde_json::to_value(&got).unwrap();
            if got != input.quality {
                failures.push(format!("{} {side}: {got}", input.name));
            }
        }
    }
    assert!(failures.is_empty(), "hand mismatches: {failures:#?}");
}

/// P34 / P34r: the quality slot alone exceeds 4096 bits. The existing outcome
/// of both sides is the outcome of the same model without quality: no
/// truncated flag, no W-BUDGET-0, the reference enumeration complete.
#[test]
fn quality_only_numeric_cut_leaves_existing_facts_of_both_sides_unchanged() {
    deep(quality_only_numeric_cut_leaves_existing_facts);
}
fn quality_only_numeric_cut_leaves_existing_facts() {
    for input in [
        inputs::quality_only_numeric(),
        inputs::quality_only_numeric_arrivals(),
    ] {
        existing_facts_unchanged(&input);
    }
}
fn existing_facts_unchanged(input: &inputs::Input) {
    let name = input.name;
    let m = lowered(&input.source);
    let mut plain = m.clone();
    plain.quality = None;

    let e = engine::explore(&m);
    assert_eq!(
        e.quality.as_ref().unwrap().cohort[0]["value"]["reason"],
        "truncated"
    );
    assert!(!e.truncated, "{name}: engine truncated");
    assert!(e.diagnostics.iter().all(|d| d.code != diag::W_BUDGET_0));
    let mut without = e.clone();
    without.quality = None;
    assert_eq!(
        format!("{without:?}"),
        format!("{:?}", engine::explore(&plain)),
        "{name}: engine existing facts"
    );

    let r = reference::explore(&m);
    assert_eq!(r.stop, reference::StopReason::Complete, "{name}");
    assert!(!r.outcome.truncated, "{name}: reference truncated");
    let p = reference::explore(&plain);
    assert_eq!(p.stop, reference::StopReason::Complete);
    let mut without = r.outcome.clone();
    without.quality = None;
    assert_eq!(
        format!("{without:?}"),
        format!("{:?}", p.outcome),
        "{name}: reference existing facts"
    );
}

/// O14e: a membership condition `case.id in SetHolder.keys` as a quality
/// state target is valid as a condition (a case set and the current case), so
/// only the quality declaration check refuses it. lower never produces it
/// (U-SYN), so it is built by hand on the IR, as in the private qp05 table.
#[test]
fn membership_quality_target_is_refused_by_both_public_ir_entrances() {
    let src = r#"model QualityBase version "0.2" {
  type V = enum { pass, fail }
  type Ack = enum { committed }
  observed case { registered: bool = false }
  latent case { correct: bool = false }
  kernel judge { * -> { 0.9: pass with correct = true; 0.02: pass with correct = false; 0.08: fail with correct = false } }
  actor Checker { capacity 2; capability check() -> V { duration fixed 3min; outcomes kernel judge; writes correct } }
  actor Ledger { capacity 2; capability post() -> Ack { commits; duration fixed 1min; result nondet { committed }; writes registered; finish { registered = true } } }
  process Flow { v = Checker.check(); if v == pass {} else { end returned }; a = Ledger.post(); end posted }
  cut C { include [Checker, Ledger, SetHolder] }
  arrivals fixed [0min, 10min]
  bounds { cases 2; live_tasks 2; clock 30min }
  verify reachable end.posted
  property case.Wrong = reached(registered == true && correct == false)
  property case.Registered = reached(registered == true)
  metric cohort.WrongPerArrival = expected(count(case.Wrong)) / arrivals
  metric cohort.WrongPerRegistered = expected(count(case.Wrong)) / expected(count(case.Registered))
  actor SetHolder { state keys: set<CaseId> }
}
"#;
    let base = lowered(src);
    // The unmodified model is accepted by both sides (control).
    assert!(
        engine::explore(&base)
            .diagnostics
            .iter()
            .all(|d| d.code != diag::U_SEM_0)
    );
    assert_eq!(
        reference::explore(&base).stop,
        reference::StopReason::Complete
    );
    let set = base
        .state_variables
        .iter()
        .position(|v| v.case_set_size.is_some())
        .expect("SetHolder.keys");
    let mut m = base.clone();
    m.quality.as_mut().unwrap().properties[0].target =
        ir::QualityTarget::State(ir::Cond::Contains {
            set: ir::StateVarId(set),
            element: ir::Element::CurrentCase,
        });
    let e = engine::explore(&m);
    assert!(
        e.diagnostics.iter().any(|d| d.code == diag::U_SEM_0),
        "engine: {e:?}"
    );
    assert!(e.quality.is_none(), "engine publishes no quality");
    let r = reference::explore(&m);
    assert_eq!(r.stop, reference::StopReason::SemanticAbort);
    assert!(
        r.outcome
            .diagnostics
            .iter()
            .any(|d| d.code == diag::U_SEM_0)
    );
    assert!(
        r.outcome.quality.is_none(),
        "reference publishes no quality"
    );
}
