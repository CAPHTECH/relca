use super::*;

#[test]
fn projected_failure_blocks_preserve_records_and_unrelated_viewpoints() {
    let source = r#"model ProjectionFailure version "0.2" {
        type T=enum{ok}
        actor Writer {capacity 1; capability step()->T {commits; duration fixed 1min; result nondet{ok}}}
        process Flow {p=Writer.step(); end done}
        bounds{cases 1;live_tasks 1;clock 10min}
        verify safety no_resource_overcommit
    }"#;
    let m = crate::lower::lower(&crate::syntax::parse(source).unwrap()).unwrap();
    let a = m.agency.as_derived().unwrap();
    let original = a.viewpoints.clone().unwrap();
    let failed = Limits { work: 0, ..DEFAULT };
    let (mut projected, _) = viewpoints::with_limits_and_warnings_shared(
        &m,
        a,
        [DEFAULT, failed, DEFAULT],
        failed,
        true,
    );
    let mut outside = original.clone();
    restore_projected_failures(&mut outside, &projected);
    assert_eq!(outside.framing, projected.framing);
    assert_eq!(outside.effects, projected.effects);
    assert_eq!(outside.observation, original.observation);
    assert_eq!(outside.retention, original.retention);
    assert_eq!(outside.limit_warnings().len(), 2);
    if let viewpoints::Framing::NotDerived { reason, limit, .. } = &mut projected.framing {
        *reason = viewpoints::LimitReason::InvalidIr;
        *limit = None;
    } else {
        panic!()
    }
    if let Some(effects::Effects::NotDerived { reason, limit, .. }) = &mut projected.effects {
        *reason = viewpoints::LimitReason::InvalidIr;
        *limit = None;
    } else {
        panic!()
    }
    restore_projected_failures(&mut outside, &projected);
    assert_eq!(outside.framing, projected.framing);
    assert_eq!(outside.effects, projected.effects);
    assert!(outside.limit_warnings().is_empty());
}
