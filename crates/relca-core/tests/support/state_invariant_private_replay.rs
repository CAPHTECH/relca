//! Link from lib.rs cfg(test) once StateInvariantDetail exists. Public explain
//! tests verify honest replay; these verify replay independently distrusts DTOs.
use crate::{explain, lower, reference, syntax};
#[test]
fn replay_rejects_each_forged_truth_value_case_visibility_and_location() {
    for src in [
        include_str!("../../../../fixtures/sinv-initial.broken-initial/model.relca"),
        include_str!("../../../../fixtures/sinv-ended-clean.broken-increment/model.relca"),
    ] {
        let m = lower::lower(&syntax::parse(src).unwrap()).unwrap();
        let report = explain::explain(src);
        let c = report
            .explanation
            .counterexamples
            .iter()
            .find(|c| c.id == "contract Controls #1 invariant")
            .unwrap();
        assert!(reference::replay::replay(&m, &c.steps, &c.id, c.case_id).is_ok());
        for fault in 0..7 {
            let mut steps = c.steps.clone();
            let last = steps.last_mut().unwrap();
            let d = last
                .violation
                .as_mut()
                .unwrap()
                .state_invariant
                .as_mut()
                .unwrap();
            match fault {
                0 => d.antecedent = false,
                1 => d.condition = true,
                2 => d.case_id = if d.case_id == 1 { 2 } else { 1 },
                3 => d.values[0].value = "forged".into(),
                4 => d.values[0].visibility = explain::Visibility::Latent,
                5 => d.values[0].location.span.start += 1,
                6 => {
                    d.values.pop();
                }
                _ => unreachable!(),
            }
            assert!(
                reference::replay::replay(&m, &steps, &c.id, c.case_id).is_err(),
                "forgery {fault}"
            );
        }
    }
}
