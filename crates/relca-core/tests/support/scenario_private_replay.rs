//! Scenario replay probes, linked from lib.rs under cfg(test).
use crate::{explain, lower, reference, syntax};

#[test]
fn horizon_prefix_is_independently_replayed_and_wrong_times_or_skipped_boundary_events_are_refused()
{
    let source = include_str!("../../../../fixtures/scn-explain/model.relca");
    let m =
        lower::lower_with_selection(&syntax::parse(source).unwrap(), None, Some("Day")).unwrap();
    let report = explain::explain_with_scenario(source, Some("Day"));
    let c = report
        .explanation
        .counterexamples
        .iter()
        .find(|c| c.case_id == Some(3))
        .unwrap();
    assert!(reference::replay::replay(&m, &c.steps, &c.id, c.case_id).is_ok());
    assert_eq!(c.steps.last().unwrap().time, 12);
    assert_eq!(c.steps.last().unwrap().kind, explain::Kind::Horizon);
    for wrong in [11, 13] {
        let mut steps = c.steps.clone();
        steps.last_mut().unwrap().time = wrong;
        assert!(
            reference::replay::replay(&m, &steps, &c.id, c.case_id).is_err(),
            "forged horizon {wrong}"
        );
    }
    let at = c
        .steps
        .iter()
        .position(|s| s.kind == explain::Kind::Finish && s.time == 10)
        .unwrap();
    let mut steps = c.steps.clone();
    steps.remove(at);
    for (i, s) in steps.iter_mut().enumerate() {
        s.index = i;
        if let Some(v) = &mut s.violation {
            v.at_step = i;
        }
    }
    assert_ne!(steps.len(), c.steps.len());
    assert!(
        reference::replay::replay(&m, &steps, &c.id, c.case_id).is_err(),
        "same-time finish cannot be skipped"
    );
}

#[test]
fn horizon_step_with_events_still_due_at_the_horizon_is_refused() {
    // Mutation survivor F32a. Service 6, arrivals 0, 4, 4: case 1 runs 0..6,
    // case 2 runs 6..12, case 3 starts at 12. by 8: deadlines 8, 12, 12. The
    // events at D=12 (case 2's finish, its end, case 3's start) are processed
    // before the horizon (ADR 0042 M3), then case 3 is pending with deadline
    // 12 <= D: an expiry violation whose trace ends with horizon 12. A trace
    // that drops those same-time events and still stops at the horizon is not
    // legal, although every other step and time is honest.
    let source = r#"model M version "0.2" {
type T = enum { ok }
observed case { hit: bool = false }
actor A {
  capacity 1
  capability run() -> T {
    duration fixed 6min
    result nondet { ok }
    writes hit
    finish { hit = true }
  }
}
process Flow { r = A.run(); end done }
cut C { include [A] }
policy fifo tie_by case_id
scenario Day {
  arrivals fixed [0min, 4min, 4min]
  horizon 12min
  bounds { cases 3; live_tasks 1; clock 30min }
}
contract K { response case.received -> one_of(end.done) by 8min after case.received otherwise unresolved }
}
"#;
    let m =
        lower::lower_with_selection(&syntax::parse(source).unwrap(), None, Some("Day")).unwrap();
    let report = explain::explain_with_scenario(source, Some("Day"));
    let c = report
        .explanation
        .counterexamples
        .iter()
        .find(|c| c.case_id == Some(3))
        .expect("case 3 expires at D");
    assert!(reference::replay::replay(&m, &c.steps, &c.id, c.case_id).is_ok());
    let last = c.steps.last().unwrap();
    assert_eq!((last.kind, last.time), (explain::Kind::Horizon, 12));
    let immediate = |s: &explain::Stage| {
        s.time == 12
            && matches!(
                s.kind,
                explain::Kind::Finish
                    | explain::Kind::Join
                    | explain::Kind::End
                    | explain::Kind::Start
            )
    };
    let dropped: Vec<_> = c
        .steps
        .iter()
        .filter(|s| immediate(s))
        .map(|s| s.kind)
        .collect();
    assert!(dropped.contains(&explain::Kind::Finish), "{dropped:?}");
    assert!(dropped.contains(&explain::Kind::Start), "{dropped:?}");
    let kept: Vec<usize> = (0..c.steps.len())
        .filter(|&i| !immediate(&c.steps[i]))
        .collect();
    let new = |old: usize| kept.iter().position(|&k| k == old);
    let mut steps: Vec<_> = kept.iter().map(|&i| c.steps[i].clone()).collect();
    for (i, s) in steps.iter_mut().enumerate() {
        s.index = i;
        if let Some(v) = &mut s.violation {
            v.at_step = i;
            v.trigger_step = v.trigger_step.and_then(new);
            v.related_steps = v.related_steps.iter().filter_map(|&r| new(r)).collect();
        }
    }
    assert_eq!(steps.last().unwrap().kind, explain::Kind::Horizon);
    assert!(
        reference::replay::replay(&m, &steps, &c.id, c.case_id).is_err(),
        "a horizon cannot skip events due at D"
    );
}
