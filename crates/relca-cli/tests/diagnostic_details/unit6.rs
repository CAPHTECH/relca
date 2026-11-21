use super::{
    blocker, invoke, location,
    unit5::{accepts, rejected},
};

#[test]
fn concurrent_read_write() {
    let source = include_str!(
        "../../../../fixtures/agy2-static-fork-reader-first.broken-concurrent/model.relca"
    );
    let reason = "concurrent_read_write";
    rejected(
        invoke("check", source, &[]),
        "read variable has a concurrent writer",
        reason,
        "fork_read_write",
        location(6, 24, 6, 36),
        vec![
            blocker("call", Some("r"), location(6, 24, 6, 36)),
            blocker("call", Some("w"), location(6, 38, 6, 51)),
        ],
        "  at 6:24\n  kind: analysis_not_connected\n  cause: fork_read_write\n  blocked by: call r at 6:24\n  blocked by: call w at 6:38\n  see: docs/guide/dsl.md#解析が未接続のとき\n",
    );
    accepts(
        "check",
        &source.replace(
            "fork f { r = R.read(); w = W.write() }; join all f",
            "w = W.write(); r = R.read()",
        ),
        &[],
        reason,
    );
}

#[test]
fn concurrent_writers() {
    let source =
        include_str!("../../../../fixtures/aset-static-fork-target.broken-fork-target/model.relca");
    let reason = "concurrent_writers";
    rejected(
        invoke("check", source, &[]),
        "fork siblings write the same observed variable",
        reason,
        "fork_shared_write",
        location(8, 18, 8, 76),
        vec![
            blocker("call", Some("a"), location(8, 25, 8, 44)),
            blocker("call", Some("b"), location(8, 45, 8, 64)),
        ],
        "  at 8:18\n  kind: analysis_not_connected\n  cause: fork_shared_write\n  blocked by: call a at 8:25\n  blocked by: call b at 8:45\n  see: docs/guide/dsl.md#解析が未接続のとき\n",
    );
    accepts(
        "check",
        &source.replace(
            "fork f{a=Ledger.a(case.id);b=Ledger.b(case.id)};join all f",
            "a=Ledger.a(case.id);b=Ledger.b(case.id)",
        ),
        &[],
        reason,
    );
}

#[test]
fn result_not_definitely_available() {
    let source = include_str!("../../../../fixtures/rsrc-all-start.broken-no-proof/model.relca");
    let reason = "result_not_definitely_available";
    rejected(
        invoke("check", source, &[]),
        "argument result is not definitely available",
        reason,
        "argument_result",
        location(5, 152, 5, 153),
        vec![
            blocker("call", Some("b"), location(5, 35, 5, 44)),
            blocker("reference", Some("b"), location(5, 152, 5, 153)),
        ],
        "  at 5:152\n  kind: analysis_not_connected\n  cause: argument_result\n  blocked by: call b at 5:35\n  blocked by: reference b at 5:152\n  see: docs/guide/dsl.md#解析が未接続のとき\n",
    );
    accepts("check", &source.replace("join any f where result == pass tie by [a,b] remaining cancel_then_drain if none -> done", "join all f"), &[], reason);
}

#[test]
fn executor_identity_unavailable() {
    let source = include_str!("../../../../fixtures/exec-anonymous.broken-no-executor/model.relca");
    let reason = "executor_identity_unavailable";
    rejected(
        invoke("check", source, &[]),
        "executor identity is required",
        reason,
        "contract_executor",
        location(11, 16, 11, 26),
        vec![blocker("feature", Some("A"), location(4, 3, 4, 92))],
        "  at 11:16\n  kind: analysis_not_connected\n  cause: contract_executor\n  blocked by: feature A at 4:3\n  see: docs/guide/dsl.md#解析が未接続のとき\n",
    );
    accepts(
        "check",
        &source.replace("actor A { capacity 1", "actor A { executors [p]"),
        &[],
        reason,
    );
}

#[test]
fn simulation_priority_tie() {
    // The single slot frees at minute 2. Cases 3 and 4 arrived at
    // minute 1 and share rank 1; neither outranks the other for admission.
    let source =
        include_str!("../../../../fixtures/ld-priority-admission.broken-one-slot/model.relca");
    let reason = "simulation_unresolved_nondeterminism";
    rejected(
        invoke("simulate", source, &[]),
        "unresolved simulation nondeterminism: priority_tie_remaining: replication 1, time 2, cases [], sites []; at least two distinct successor actions remain",
        reason,
        "priority_tie_remaining",
        location(4, 3, 4, 42),
        vec![],
        "  at 4:3\n  kind: analysis_not_connected\n  cause: priority_tie_remaining\n  see: docs/guide/dsl.md#解析が未接続のとき\n",
    );
    accepts(
        "simulate",
        include_str!("../../../../fixtures/ld-priority-admission/model.relca"),
        &[],
        reason,
    );
}

#[test]
fn simulation_unresolved_nondeterminism() {
    let source = include_str!(
        "../../../../fixtures/sim-actor-initial.broken-missing-distribution/model.relca"
    );
    let reason = "simulation_unresolved_nondeterminism";
    rejected(
        invoke("simulate", source, &[]),
        "simulation encountered unresolved nondeterminism: replication 1, time 0, cases [0], sites [init.actor.A.z]; at least two distinct successor actions remain",
        reason,
        "unresolved_choices",
        location(3, 20, 3, 23),
        vec![blocker(
            "feature",
            Some("init.actor.A.z"),
            location(3, 20, 3, 23),
        )],
        "  at 3:20\n  kind: analysis_not_connected\n  cause: unresolved_choices\n  blocked by: feature init.actor.A.z at 3:20\n  see: docs/guide/dsl.md#解析が未接続のとき\n",
    );
    accepts(
        "simulate",
        include_str!("../../../../fixtures/sim-actor-initial/model.relca"),
        &[],
        reason,
    );
}
