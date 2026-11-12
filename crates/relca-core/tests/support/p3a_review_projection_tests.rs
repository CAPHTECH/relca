//! Private projection regression; fixed original support remains untouched.
use super::*;

#[test]
fn p3a_review_cutoff_rejects_every_forged_receipt_observation_field() {
    let source = r#"model CutoffObservation version "0.2" {type V=enum {ok}
      actor A {capacity 1;capability run()->V {duration fixed 2min;result nondet {ok}}}
      process P {r=A.run() response_loss none;end done}
      scenario Day {arrivals fixed [0min];horizon 1min;bounds {cases 1;live_tasks 1;clock 5min}}
      verify safety no_resource_overcommit}"#;
    let ast = crate::syntax::parse(source).unwrap();
    let model = crate::lower::lower_with_selection(&ast, None, Some("Day")).unwrap();
    let control = crate::report::check_with_selection(source, None, Some("Day"));
    exhaustive(&control, &model).expect("establish valid cutoff before negative controls");
    assert_eq!(
        control.case_outcomes.as_ref().unwrap()[0].terminals[0].kind,
        report::CaseTerminalKind::CutOff
    );
    for field in 0..3 {
        let mut bad = control.clone();
        let t = &mut bad.case_outcomes.as_mut().unwrap()[0].terminals[0];
        let value = Some(report::Range { min: 1, max: 1 });
        match field {
            0 => t.observed_until = value,
            1 => t.observed_elapsed = value,
            _ => t.observed_work = value,
        }
        assert!(
            exhaustive(&bad, &model).is_err(),
            "forged cutoff observation field {field}"
        );
    }
}
