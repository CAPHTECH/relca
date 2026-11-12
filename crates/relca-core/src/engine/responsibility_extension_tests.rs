// Included as a child of the existing responsibility_census test module. No
// runtime path or instrumentation is added to the production engine.
use super::*;

fn fingerprint(m: &ir::Model) -> (BTreeSet<String>, (usize, usize, usize)) {
    if m.uses_probability() {
        let mut e = Exploration::default();
        let (nodes, _) = probability_graph(m, DEFAULT_STATE_BUDGET, &mut e).unwrap();
        (
            nodes.iter().map(|n| format!("{:?}", n.state)).collect(),
            (nodes.len(), e.census.1, e.census.2),
        )
    } else {
        (snapshots(m), counts(m, DEFAULT_STATE_BUDGET).0)
    }
}

#[test]
fn r2_twenty_six_runtime_state_task_monitor_census_and_room_are_identical() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let mut runtime = 0;
    let mut rejected = 0;
    for name in crate::responsibility::extension_population::NAMES {
        let src = std::fs::read_to_string(root.join(name).join("model.relca")).unwrap();
        let parsed = crate::syntax::parse(&src).and_then(|a| crate::lower::lower(&a));
        let Ok(m) = parsed else { rejected += 1; continue; };
        runtime += 1;
        let stripped = src.lines().filter(|l| !l.trim_start().starts_with("responsible ")).collect::<Vec<_>>().join("\n");
        let old = lower(&stripped);
        // The Debug snapshot includes every private State/Task/Monitor field.
        let actual = fingerprint(&m);
        assert_eq!(actual, fingerprint(&old), "{name}: private states and visited/generated/held");
        assert!(!actual.0.is_empty(), "{name}: nonempty state population");
        if !m.uses_probability() {
            for budget in 0..=actual.1.2 + 1 {
                assert_eq!(counts(&m,budget), counts(&old,budget), "{name}: room budget {budget}");
            }
        }
    }
    assert_eq!((runtime,rejected),(26,4));
}

#[test]
fn r2_simulation_draw_keys_and_replication_audit_are_unchanged() {
    // Remove result nondeterminism rather than sampling it uniformly. Duration
    // remains probabilistic, so equality of an empty draw list cannot pass.
    let src = include_str!("../../../../fixtures/resp2-duration/model.relca")
        .replace("result nondet { pass,fail }", "result nondet { pass }")
        .replace("  bounds", "  simulation replications 8 seed 0 confidence 0.5\n  bounds");
    let old = src.lines().filter(|l| !l.trim_start().starts_with("responsible ")).collect::<Vec<_>>().join("\n");
    let m = lower(&src);
    let before = lower(&old);
    let actual = simulate(&m);
    assert_eq!(actual.completed_replications,8,"{actual:?}");
    assert!(actual.replications.iter().all(|r| !r.draws.is_empty()));
    assert_eq!(actual,simulate(&before));
    assert_eq!(actual,crate::reference::simulate(&m).outcome);
}
