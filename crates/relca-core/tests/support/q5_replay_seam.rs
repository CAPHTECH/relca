//! ADR0062 P14: author-owned private replay probes, NOT CONNECTED in acceptance.
//! Implementation may only add cfg(test) + path/mod lines in arrivals_replay.rs.
//! No reference Frame/Task/type or production transition is copied here.
use super::*;
#[path = "q5_inputs.rs"]
mod input;

fn selected(source: &str) -> Model {
    crate::syntax::parse(source)
        .and_then(|ast| crate::lower::lower(&ast))
        .expect("Q5 grammar and lower must be connected before replay seam")
}

// A trace from the opposite explicitly declared priority is an otherwise legal
// adversarial witness. Same-width rank edits retain every DSL source span.
// Both the positive replay and independently fixed first case are checked.
fn opposite_witness(source: &str, target_suffix: &str) -> (Vec<Stage>, String, Option<u64>) {
    let explained = crate::explain::explain(source);
    let counterexample = explained
        .explanation
        .counterexamples
        .iter()
        .find(|c| c.id.ends_with(target_suffix))
        .expect("required structural counterexample, not an omitted probability explanation");
    assert_eq!(counterexample.replay.status, x::ReplayStatus::Verified);
    let m = selected(source);
    let choices = vec![None; counterexample.steps.len()];
    assert_eq!(
        replay(
            &m,
            &counterexample.steps,
            &counterexample.id,
            counterexample.case_id,
            &choices
        ),
        Ok(()),
        "positive control: the complete generated witness is independently legal"
    );
    (
        counterexample.steps.clone(),
        counterexample.id.clone(),
        counterexample.case_id,
    )
}

fn first_case(steps: &[Stage], kind: Kind, after_zero: bool) -> u64 {
    for step in steps {
        if after_zero && step.time == 0 {
            continue;
        }
        if step.kind == kind
            && let Some(case) = step.case_id
        {
            return case;
        }
        for event in &step.events {
            if let x::Event::Business(event) = event
                && event.kind == kind
                && let Some(case) = event.case_id
            {
                return case;
            }
        }
    }
    panic!("required {kind:?} event");
}

fn rejects(base: &str, steps: &[Stage], target: &str, case: Option<u64>) {
    assert_eq!(
        replay(
            &selected(base),
            steps,
            target,
            case,
            &vec![None; steps.len()]
        ),
        Err(ReplayReason::IllegalTransition),
        "priority-illegal witness must fail at the transition, not just final violation mismatch"
    );
}

#[test]
fn q5_replay_illegal_low_start_has_otherwise_legal_opposite_priority_control() {
    let base = input::append(
        &input::edit(&input::source("equal"), "[5, 5]", "[0, 9]"),
        "contract Deadline { response case.received -> one_of(end.done) by 3min after case.received otherwise unresolved }",
    );
    let opposite = input::edit(&base, "[0, 9]", "[9, 0]");
    assert_eq!(base.len(), opposite.len());
    let (steps, target, case) = opposite_witness(&opposite, " response");
    assert_eq!(first_case(&steps, Kind::Start, false), 1);
    rejects(&base, &steps, &target, case);
}

#[test]
fn q5_replay_illegal_held_admission_does_not_reorder_initial_admission() {
    let base = input::source("explain-external");
    let opposite = input::edit(&base, "[0, 1, 9]", "[0, 9, 1]");
    assert_eq!(base.len(), opposite.len());
    let (steps, target, case) = opposite_witness(&opposite, " response");
    assert_eq!(
        first_case(&steps, Kind::Admitted, false),
        1,
        "initial CaseId unchanged"
    );
    assert_eq!(
        first_case(&steps, Kind::Admitted, true),
        2,
        "adversarial readmission"
    );
    rejects(&base, &steps, &target, case);
}

#[test]
fn q5_replay_illegal_low_version_finish_has_otherwise_legal_opposite_priority_control() {
    let base = input::source("explain-conflict");
    let opposite = input::edit(&base, "[0, 9]", "[9, 0]");
    assert_eq!(base.len(), opposite.len());
    let (steps, target, case) = opposite_witness(&opposite, " invariant");
    assert_eq!(first_case(&steps, Kind::Finish, false), 1);
    rejects(&base, &steps, &target, case);
}
