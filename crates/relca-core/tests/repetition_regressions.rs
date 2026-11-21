use relca_core::{engine, lower, reference, syntax};
#[path = "support/r2_inputs.rs"]
mod recovery_scope_inputs;

fn replace_once(source: &str, before: &str, after: &str) -> String {
    assert_eq!(source.matches(before).count(), 1, "{before}");
    source.replacen(before, after, 1)
}

const STATIC_ROUND: &str = r#"model StaticRound version "0.2" {
    type V = enum { ok, denied }
    kernel K given [work.round] { 1 -> { 1: ok }; 2 -> { 1: ok } }
    actor A { capacity 1
        capability kernel() -> V { duration fixed 1min; outcomes kernel K }
        capability safe() -> V { duration fixed 1min; result nondet { ok } }
    }
    process Looped { repeat work at_most 2 { wait 1min; again } on_limit { leave } end done }
    process U { r = A.safe(); end done }
    cut C { include [A] }
    configuration Loop { cut C; process Looped }
    configuration Hand { cut C; process U }
    bounds { cases 1; live_tasks 1; clock 20min }
    simulation replications 2 seed 7 confidence 0.5
    verify safety no_resource_overcommit
}"#;

#[test]
fn all_lexical_task_routes_reject_static_round_kernel_and_requires_consumption() {
    for body in [
        "r = A.CONSUME(); end done",
        "fork f { r = A.CONSUME() } join all f; end done",
        "r = A.CONSUME() response_loss none; end done",
        "r = A.CONSUME() response_loss none retry max_attempts 2 when result [denied] backoff fixed 1min exhausted end done; end done",
        "r = A.safe(); degrade Manual when r == denied { alt = A.CONSUME(); end done } else { end done }",
    ] {
        let source = replace_once(STATIC_ROUND, "r = A.safe(); end done", body);
        let baseline = replace_once(&source, "A.CONSUME()", "A.safe()");
        lower::lower_with_selection(&syntax::parse(&baseline).unwrap(), Some("Hand"), None)
            .unwrap();
        let kernel = replace_once(&source, "A.CONSUME()", "A.kernel()");
        for source in [
            &kernel,
            &replace_once(
                &kernel,
                "outcomes kernel K",
                "requires work.round >= 1 else denied; result nondet { ok }",
            ),
        ] {
            let ds =
                lower::lower_with_selection(&syntax::parse(source).unwrap(), Some("Hand"), None)
                    .unwrap_err();
            assert!(
                ds.iter()
                    .any(|d| d.code == "U-SEM-0" && d.message == "loop_round_out_of_scope"),
                "{body}: {ds:?}"
            );
        }
        let source = replace_once(
            STATIC_ROUND,
            "on_limit { leave } end done",
            &format!("on_limit {{ leave }} {body}"),
        );
        let kernel = replace_once(&source, "A.CONSUME()", "A.kernel()");
        for source in [
            &kernel,
            &replace_once(
                &kernel,
                "outcomes kernel K",
                "requires work.round >= 1 else denied; result nondet { ok }",
            ),
        ] {
            let ds =
                lower::lower_with_selection(&syntax::parse(source).unwrap(), Some("Loop"), None)
                    .unwrap_err();
            assert!(
                ds.iter()
                    .any(|d| d.code == "U-SEM-0" && d.message == "loop_round_out_of_scope"),
                "executable suffix {body}: {ds:?}"
            );
        }
    }
}

#[test]
fn receipt_unknown_handler_and_query_are_not_scope_bypasses() {
    let source = recovery_scope_inputs::body(
        Some(recovery_scope_inputs::PROCEED),
        recovery_scope_inputs::LOSS,
    );
    let source = recovery_scope_inputs::append(
        &source,
        "process Other { repeat work at_most 2 { wait 1min; again } on_limit { end shipped } end shipped }\nconfiguration Loop { cut C; process Other }\nconfiguration Hand { cut C; process Flow }",
    );
    lower::lower_with_selection(&syntax::parse(&source).unwrap(), Some("Hand"), None).unwrap();
    for (before, after, reason) in [
        (
            "writes [keys,charged_truth,charges]; duration fixed 1min",
            "writes [keys,charged_truth,charges]; requires work.round >= 1 else rejected_duplicate; duration fixed 1min",
            "loop_round_out_of_scope",
        ),
        (
            "duration fixed 2min; receipt_query",
            "requires work.round >= 1 else still_unknown; duration fixed 2min; receipt_query",
            "recovery_query_unmodeled", // Query contracts forbid requires before task lowering.
        ),
        (
            "duration fixed 3min; result nondet {proceed}",
            "requires work.round >= 1 else withdraw; duration fixed 3min; result nondet {proceed}",
            "loop_round_out_of_scope",
        ),
        (
            "capability instruct_ship(d: Decision) -> Ack { duration fixed 1min",
            "capability instruct_ship(d: Decision) -> Ack { requires work.round >= 1 else recorded; duration fixed 1min",
            "loop_round_out_of_scope",
        ),
        (
            "capability ship_known(p: Pay) -> Ack { duration fixed 1min",
            "capability ship_known(p: Pay) -> Ack { requires work.round >= 1 else recorded; duration fixed 1min",
            "loop_round_out_of_scope",
        ),
    ] {
        let source = replace_once(&source, before, after);
        let ds = lower::lower_with_selection(&syntax::parse(&source).unwrap(), Some("Hand"), None)
            .unwrap_err();
        assert!(
            ds.iter()
                .any(|d| d.code == "U-SEM-0" && d.message == reason),
            "{before}: {ds:?}"
        );
    }
}

#[test]
fn static_round_atomic_is_rejected_but_existing_executable_atomic_scope_is_preserved() {
    let source = replace_once(
        STATIC_ROUND,
        "    kernel K",
        "    observed case { flag: bool = false }\n    kernel K",
    );
    let source = replace_once(
        &source,
        "result nondet { ok }",
        "writes flag; atomic { if work.round >= 1 { flag = true; result ok } else { result denied } }",
    );
    let ds = lower::lower_with_selection(&syntax::parse(&source).unwrap(), Some("Hand"), None)
        .unwrap_err();
    assert!(
        ds.iter()
            .any(|d| d.code == "U-SEM-0" && d.message == "loop_round_out_of_scope"),
        "{ds:?}"
    );
    let source = replace_once(
        &source,
        "on_limit { leave } end done",
        "on_limit { leave } after = A.safe(); end done",
    );
    let m =
        lower::lower_with_selection(&syntax::parse(&source).unwrap(), Some("Loop"), None).unwrap();
    let r = reference::explore(&m);
    assert_eq!(r.stop, reference::StopReason::Complete);
    for o in [engine::explore(&m), r.outcome] {
        assert_eq!(o.terminals.len(), 1);
        assert_eq!(
            o.terminals[0].elapsed,
            relca_core::outcome::Range { min: 3, max: 3 }
        );
        assert!(
            o.diagnostics
                .iter()
                .all(|d| d.severity != relca_core::diag::Severity::Error)
        );
    }
}

#[test]
fn runtime_static_round_read_is_a_diagnostic_in_both_modes_and_backends() {
    let mut m =
        lower::lower_with_selection(&syntax::parse(STATIC_ROUND).unwrap(), Some("Hand"), None)
            .unwrap();
    assert!(m.state_variables.is_empty());
    let id = m
        .capabilities
        .iter()
        .position(|c| c.name == "kernel")
        .unwrap();
    let relca_core::ir::Stmt::Call(call) = &mut m.process.body[0].kind else {
        panic!("call")
    };
    call.capability = relca_core::ir::CapabilityId(id);
    // Deliberately bypass lower: a declaration-only round must never be read
    // as an initialized runtime value, even from forged public IR.
    for cases in [1, 2] {
        m.bounds.cases = cases;
        m.bounds.live_tasks = cases;
        m.arrivals = (cases == 2).then(|| vec![0, 3]);
        let e = engine::explore(&m);
        assert!(
            e.diagnostics
                .iter()
                .any(|d| d.code == "U-SEM-0" && d.message.contains("missing kernel row")),
            "{e:?}"
        );
        assert!(e.terminals.is_empty());
        let r = reference::explore(&m);
        assert_eq!(r.stop, reference::StopReason::SemanticAbort);
        assert!(r.outcome.terminals.is_empty());
        let e = engine::simulate(&m);
        assert_eq!(e.completed_replications, 0);
        assert!(
            e.diagnostics
                .iter()
                .any(|d| d.code == "U-SEM-0" && d.message.contains("missing kernel row")),
            "cases={cases}: {e:?}"
        );
        let r = reference::simulate(&m);
        assert_eq!(r.stop, reference::StopReason::SemanticAbort);
        assert_eq!(r.outcome.completed_replications, 0);
    }
}

#[test]
fn loop_state_and_backend_observations_belong_to_the_selected_process() {
    let src = include_str!("bounded_repetition/models/L2-pair.relca");
    let ast = syntax::parse(src).unwrap();
    for (name, has_loops) in [("Hand", false), ("Loop", true)] {
        let m = lower::lower_with_selection(&ast, Some(name), None).unwrap();
        assert_eq!(m.has_loops(), has_loops);
        assert_eq!(
            m.state_variables
                .iter()
                .filter(|v| v.name.starts_with("@loop:"))
                .count(),
            if has_loops { 5 } else { 0 }
        );
        let e = engine::explore(&m);
        let r = reference::explore(&m);
        assert_eq!(r.stop, reference::StopReason::Complete);
        assert_eq!(e.terminals, r.outcome.terminals);
        assert_eq!(e.properties, r.outcome.properties);
        assert_eq!(e.metrics, r.outcome.metrics);
        for o in [&e, &r.outcome] {
            assert!(!o.truncated);
            assert_eq!(o.properties.len(), 1);
            assert_eq!(o.properties[0].verdict, relca_core::Verdict::Satisfied);
            assert_eq!(o.terminals.len(), 2);
            for (end, expected, probability) in [("done", 1, "9/10"), ("escalated", 9, "1/10")] {
                let t = o.terminals.iter().find(|t| m.ends[t.end.0] == end).unwrap();
                assert_eq!((t.elapsed.min, t.elapsed.max), (expected, expected));
                assert_eq!((t.work.min, t.work.max), (expected, expected));
                let p = t.probability.as_ref().unwrap();
                assert_eq!(
                    (p.min.to_string(), p.max.to_string()),
                    (probability.into(), probability.into())
                );
            }
        }
    }
}

#[test]
fn unselected_loop_names_still_collide_model_wide() {
    let src = include_str!("bounded_repetition/models/L2-pair.relca");
    for addition in [
        "repeat rework at_most 1 { wait 1min; again } on_limit { leave }",
        "repeat r at_most 1 { wait 1min; again } on_limit { leave }",
    ] {
        let src = replace_once(
            src,
            "process Unrolled {",
            &format!("process Unrolled {{ {addition}"),
        );
        let ast = syntax::parse(&src).unwrap();
        for configuration in ["Hand", "Loop"] {
            let ds = lower::lower_with_selection(&ast, Some(configuration), None).unwrap_err();
            assert!(
                ds.iter()
                    .any(|d| d.code == "E-NAME-0" && d.message == "loop name collision"),
                "{ds:?}"
            );
        }
    }
}

#[test]
fn unselected_round_kernel_keeps_its_static_domain_without_runtime_state() {
    let src = include_str!("bounded_repetition/models/L4.relca");
    assert_eq!(
        src.lines()
            .filter(|line| line.contains("verify probability"))
            .count(),
        4
    );
    let src = src
        .lines()
        .filter(|line| !line.contains("verify probability"))
        .collect::<Vec<_>>()
        .join("\n");
    let src = replace_once(
        &src,
        "  bounds",
        "  process U { wait 1min; end done }\n  configuration Loop { cut C; process P }\n  configuration Hand { cut C; process U }\n  bounds",
    );
    for src in [
        &src,
        &replace_once(
            &src,
            "duration fixed 1min; outcomes kernel K",
            "requires rework.round >= 1 else fail; duration fixed 1min; outcomes kernel K",
        ),
    ] {
        let m =
            lower::lower_with_selection(&syntax::parse(src).unwrap(), Some("Hand"), None).unwrap();
        assert!(m.state_variables.is_empty());
        assert!(!m.has_loops());
        assert_eq!(m.static_loop_rounds.len(), 1);
        let e = engine::explore(&m);
        let r = reference::explore(&m);
        assert_eq!(r.stop, reference::StopReason::Complete);
        assert_eq!(e.terminals, r.outcome.terminals);
        assert_eq!(e.properties, r.outcome.properties);
        assert_eq!(e.metrics, r.outcome.metrics);
        for o in [&e, &r.outcome] {
            assert!(
                o.diagnostics
                    .iter()
                    .all(|d| d.severity != relca_core::diag::Severity::Error),
                "{o:?}"
            );
            assert!(!o.truncated);
            assert_eq!(o.properties.len(), 1);
            assert_eq!(o.properties[0].verdict, relca_core::Verdict::Satisfied);
            assert_eq!(o.terminals.len(), 1);
            assert_eq!(m.ends[o.terminals[0].end.0], "done");
            assert_eq!(
                o.terminals[0].elapsed,
                relca_core::outcome::Range { min: 1, max: 1 }
            );
            assert_eq!(
                o.terminals[0].work,
                relca_core::outcome::Range { min: 0, max: 0 }
            );
        }
        let mut malformed = m.clone();
        malformed.static_loop_rounds[0].integer_max = None;
        assert_eq!(
            reference::explore(&malformed).stop,
            reference::StopReason::SemanticAbort
        );
        assert!(
            engine::explore(&malformed)
                .diagnostics
                .iter()
                .any(|d| d.severity == relca_core::diag::Severity::Error)
        );
    }
}

#[test]
fn expanded_round_control_reaches_both_interpreters() {
    let src = r#"model M version "0.2" {
        type V = enum { ok }
        actor A { capacity 1; capability c() -> V { duration fixed 1min; result nondet { ok } } }
        process P { repeat work at_most 3 { r = A.c(); again } on_limit { end limit } end done }
        bounds { cases 1; live_tasks 1; clock 20min }
        verify reachable end.limit
    }"#;
    let m = lower::lower(&syntax::parse(src).unwrap()).unwrap();
    let e = engine::explore(&m);
    let r = reference::explore(&m);
    assert!(
        e.diagnostics.iter().all(|d| !d.code.starts_with("U-")),
        "engine: {e:?}"
    );
    assert_eq!(
        r.stop,
        reference::StopReason::Complete,
        "reference: {:?}",
        r.outcome
    );
    assert_eq!(e.terminals, r.outcome.terminals);
    assert_eq!(e.terminals[0].elapsed.min, 3);
}

#[test]
fn iteration_distribution_and_source_mapping_are_typed_observations() {
    let src = include_str!("bounded_repetition/models/L2.relca");
    let r = relca_core::report::check(src);
    let wire = serde_json::to_value(&r).unwrap();
    assert_eq!(
        wire["repetition"]["coverage"], "complete",
        "{}",
        wire["repetition"]["unchecked"]
    );
    let row = &wire["repetition"]["cases"][0];
    assert_eq!(row["rounds"], serde_json::json!({"min":1,"max":3}));
    assert_eq!(
        row["distribution"][1]["probability"],
        serde_json::json!({"min":"9/10","max":"9/10"})
    );
    assert_eq!(
        row["distribution"][2]["probability"],
        serde_json::json!({"min":"0","max":"0"})
    );
    assert_eq!(
        row["distribution"][3]["probability"],
        serde_json::json!({"min":"1/10","max":"1/10"})
    );
    assert_eq!(
        row["on_limit_probability"],
        serde_json::json!({"min":"1/10","max":"1/10"})
    );
    for site in wire["repetition"]["sites"].as_array().unwrap() {
        let lo = site["source_span"]["start"].as_u64().unwrap() as usize;
        let hi = site["source_span"]["end"].as_u64().unwrap() as usize;
        assert_eq!(&src[lo..hi], site["origin"]["call_site"].as_str().unwrap());
    }
    let decoded: relca_core::report::Report = serde_json::from_value(wire.clone()).unwrap();
    assert_eq!(serde_json::to_value(decoded).unwrap(), wire);
}

#[test]
fn iteration_samples_are_not_probability_guarantees_and_cuts_are_not_limits() {
    let src = include_str!("bounded_repetition/models/P1-r3.relca");
    let src = format!(
        "{} simulation replications 4 seed 7 confidence 0.5 }}",
        src.trim_end().strip_suffix('}').unwrap()
    );
    let r = relca_core::report::simulate(&src);
    let wire = serde_json::to_value(&r).unwrap();
    assert_eq!(wire["repetition"]["coverage"], "sampled");
    let row = &wire["repetition"]["cases"][0];
    assert_eq!(row["distribution"][3]["observed_count"], 4);
    assert_eq!(row["observed_on_limit"], 0);
    assert!(row["on_limit_probability"].is_null());
    let cut = relca_core::report::check(include_str!("bounded_repetition/models/L2-clock.relca"));
    let cut = serde_json::to_value(cut).unwrap();
    assert_eq!(cut["repetition"]["coverage"], "not_derived");
    assert_eq!(cut["repetition"]["cases"], serde_json::json!([]));
}

#[test]
fn random_sites_distinguish_rounds_without_shifting_the_suffix() {
    let src = r#"model Sites version "0.2" {
        type V = enum { a, b }
        kernel K { * -> { 0.5: a; 0.5: b } }
        actor A { capacity 1; capability c() -> V { duration fixed 1min; outcomes kernel K } }
        process P { repeat work at_most 3 { r = A.c(); again } on_limit { leave }
            after = A.c(); if after == a { end a } else { end b } }
        bounds { cases 1; live_tasks 1; clock 20min }
        simulation replications 12 seed 7 confidence 0.5
        verify safety no_resource_overcommit
    }"#;
    let run = |s: &str| engine::simulate(&lower::lower(&syntax::parse(s).unwrap()).unwrap());
    let a = run(src);
    let b = run(&src.replace("at_most 3", "at_most 1"));
    assert!(
        a.diagnostics.iter().all(|d| !d.code.starts_with("U-")),
        "{:?}",
        a.diagnostics
    );
    assert_eq!(a.completed_replications, 12);
    assert_eq!(b.completed_replications, 12);
    for (a, b) in a.replications.iter().zip(&b.replications) {
        assert_eq!(a.cases[0].terminal, b.cases[0].terminal);
        let suffix = |r: &relca_core::outcome::ReplicationOutcome| {
            r.draws
                .iter()
                .filter(|d| d.key.contains("after"))
                .map(|d| d.key.clone())
                .collect::<Vec<_>>()
        };
        assert!(!suffix(a).is_empty());
        assert_eq!(suffix(a), suffix(b));
        for k in 1..=3 {
            assert!(
                a.draws
                    .iter()
                    .any(|d| d.key.contains(&format!("@loop:work:{k}:r")))
            );
        }
    }
}

#[test]
fn limit_tasks_have_loop_provenance_and_publish_their_result() {
    let src = r#"model Limit version "0.2" {
        type V = enum { ok }
        actor A { capacity 1; capability c() -> V { duration fixed 1min; result nondet { ok } } }
        process P { repeat work at_most 2 { r = A.c(); again } on_limit { last = A.c(); leave }
            if last == ok { end done } else { end wrong } }
        bounds { cases 1; live_tasks 1; clock 20min }
        verify reachable end.done
    }"#;
    let m = lower::lower(&syntax::parse(src).unwrap()).unwrap();
    for o in [engine::explore(&m), reference::explore(&m).outcome] {
        assert_eq!(o.terminals.len(), 1, "{o:?}");
        assert_eq!(o.terminals[0].elapsed.min, 3);
    }
    let r = relca_core::report::check(src);
    let wire = serde_json::to_value(r).unwrap();
    assert!(wire["repetition"]["sites"].as_array().unwrap().iter().any(
        |s| s["origin"]["call_site"] == "last"
            && s["origin"]["phase"] == "on_limit"
            && s["origin"]["round"].is_null()
    ));
}

#[test]
fn finish_invariants_read_the_latest_published_loop_result() {
    let src = r#"model Latest version "0.2" {
        type V = enum { ok, wrong }
        actor A { capacity 1; capability c() -> V { duration fixed 1min; result nondet { ok } } }
        process P { repeat work at_most 2 { r = A.c(); again } on_limit { end done } }
        bounds { cases 1; live_tasks 1; clock 20min }
        verify safety no_resource_overcommit
        contract C { invariant A.c.finish -> r == ok }
    }"#;
    let m = lower::lower(&syntax::parse(src).unwrap()).unwrap();
    for o in [engine::explore(&m), reference::explore(&m).outcome] {
        assert!(
            o.properties
                .iter()
                .all(|p| p.verdict == relca_core::Verdict::Satisfied),
            "{o:?}"
        );
    }
    let bad = src.replace("-> r == ok", "-> r == wrong");
    let report = serde_json::to_value(relca_core::explain::explain(&bad)).unwrap();
    assert_eq!(report["verdict"], "violated", "{report}");
    let traces = report["explanation"]["counterexamples"].as_array().unwrap();
    assert!(!traces.is_empty());
    assert!(
        traces.iter().all(|t| t["replay"]["status"] == "verified"),
        "{traces:?}"
    );
}

#[test]
fn requires_reads_the_current_round_and_refusal_has_no_service_time() {
    let src = r#"model RequiresRound version "0.2" {
        type V = enum { ok, denied }
        actor A { capacity 1; capability c() -> V { requires work.round >= 2 else denied; duration fixed 1min; result nondet { ok } } }
        process P { repeat work at_most 3 { r = A.c(); wait 1min; again } on_limit { end done } }
        bounds { cases 1; live_tasks 1; clock 20min }
        verify reachable end.done
    }"#;
    let m = lower::lower(&syntax::parse(src).unwrap()).unwrap();
    let e = engine::explore(&m);
    let r = reference::explore(&m);
    assert_eq!(r.stop, reference::StopReason::Complete);
    for o in [e, r.outcome] {
        assert_eq!(o.terminals.len(), 1, "{o:?}");
        assert_eq!(
            o.terminals[0].elapsed,
            relca_core::outcome::Range { min: 5, max: 5 }
        );
        assert_eq!(
            o.terminals[0].work,
            relca_core::outcome::Range { min: 2, max: 2 }
        );
    }
}

#[test]
fn round_given_and_polling_gap_counterexamples_replay_independently() {
    for (src, ends, deadline) in [
        (
            include_str!("bounded_repetition/models/L4.relca"),
            "end.done,end.escalated",
            8,
        ),
        (
            include_str!("bounded_repetition/models/P1-r3.relca"),
            "end.shipped,end.gave_up",
            60,
        ),
    ] {
        let src = format!(
            "{} contract Deadline {{ response case.received -> one_of({ends}) by {deadline}min after case.received otherwise unresolved }} }}",
            src.trim_end().strip_suffix('}').unwrap()
        );
        let wire = serde_json::to_value(relca_core::explain::explain(&src)).unwrap();
        assert_eq!(wire["verdict"], "violated", "{wire}");
        let traces = wire["explanation"]["counterexamples"].as_array().unwrap();
        assert!(!traces.is_empty(), "{wire}");
        assert!(
            traces.iter().all(|t| t["replay"]["status"] == "verified"),
            "{traces:?}"
        );
    }
}

#[test]
fn round_scope_belongs_to_its_own_body_not_the_declaration_or_suffix() {
    let make = |cap: &str, process: &str, extra: &str| {
        format!(
            r#"model Scope version "0.2" {{
        type V = enum {{ ok, denied }}
        actor A {{ capacity 1; capability c() -> V {{ {cap} }} }}
        process P {{ {process} }}
        bounds {{ cases 1; live_tasks 1; clock 20min }}
        verify safety no_resource_overcommit
        {extra}
    }}"#
        )
    };
    let plain = "duration fixed 1min; result nondet { ok }";
    let requires =
        "duration fixed 1min; result nondet { ok }; requires work.round >= 1 else denied";
    let given = "duration fixed 1min; outcomes kernel K";
    let kernel = "kernel K given [work.round] { 1 -> { 1: ok }; 2 -> { 1: ok } }";
    let prefix = "repeat work at_most 2 { r = A.c(); wait 1min; again } on_limit { leave }";
    let labels = [
        "suffix_if",
        "suffix_requires",
        "on_limit_if",
        "suffix_given",
        "other_loop_if",
        "global_invariant",
    ];
    let mut unexpectedly_supported = Vec::new();
    for (index, source) in [
        make(
            plain,
            &format!("{prefix} if work.round == 2 {{ end done }} else {{ end wrong }}"),
            "",
        ),
        make(requires, &format!("{prefix} after = A.c(); end done"), ""),
        make(
            plain,
            "repeat work at_most 2 { r = A.c(); again } on_limit { if work.round == 2 { leave } else { end wrong } } end done",
            "",
        ),
        make(given, &format!("{prefix} after = A.c(); end done"), kernel),
        make(
            plain,
            &format!(
                "{prefix} repeat later at_most 2 {{ s = A.c(); if work.round == 2 {{ leave }} else {{ again }} }} on_limit {{ end limit }} end done"
            ),
            "",
        ),
        make(
            plain,
            &format!("{prefix} end done"),
            "contract C { invariant A.c.finish -> work.round >= 1 }",
        ),
    ].into_iter().enumerate() {
        let ast = syntax::parse(&source).unwrap();
        let ds = match lower::lower(&ast) {
            Ok(_) => { unexpectedly_supported.push(labels[index]); continue; }
            Err(ds) => ds,
        };
        let errors: Vec<_> = ds
            .iter()
            .filter(|d| d.severity == relca_core::diag::Severity::Error)
            .collect();
        assert!(!errors.is_empty());
        assert!(
            errors
                .iter()
                .all(|d| d.code == "U-SEM-0" && d.message == "loop_round_out_of_scope"),
            "{ds:?}"
        );
    }
    assert!(
        unexpectedly_supported.is_empty(),
        "missing scope refusal: {unexpectedly_supported:?}"
    );
    for (cap, extra) in [(plain, ""), (requires, ""), (given, kernel)] {
        let src = make(
            cap,
            "repeat work at_most 2 { r = A.c(); wait 1min; if work.round == 2 { leave } else { again } } on_limit { end limit } end done",
            extra,
        );
        let m = lower::lower(&syntax::parse(&src).unwrap()).unwrap();
        let reference = reference::explore(&m);
        assert_eq!(reference.stop, reference::StopReason::Complete);
        for o in [engine::explore(&m), reference.outcome] {
            assert!(
                o.properties
                    .iter()
                    .all(|p| p.verdict == relca_core::Verdict::Satisfied),
                "{o:?}"
            );
            assert_eq!(m.ends[o.terminals[0].end.0], "done");
            assert_eq!(
                o.terminals[0].elapsed,
                relca_core::outcome::Range { min: 4, max: 4 }
            );
        }
    }
}

#[test]
fn expansion_limit_is_not_misclassified_as_a_kernel_row_error() {
    let rows = (1..=33)
        .map(|k| format!("{k} -> {{ 1: ok }}"))
        .collect::<Vec<_>>()
        .join("; ");
    let src = format!(
        r#"model TooMany version "0.2" {{
        type V = enum {{ ok }}
        kernel K given [work.round] {{ {rows} }}
        actor A {{ capacity 1; capability c() -> V {{ duration fixed 1min; outcomes kernel K }} }}
        process P {{ repeat work at_most 33 {{ r = A.c(); again }} on_limit {{ end limit }} end done }}
        bounds {{ cases 1; live_tasks 1; clock 100min }}
        verify safety no_resource_overcommit
    }}"#
    );
    let ds = lower::lower(&syntax::parse(&src).unwrap()).unwrap_err();
    let errors: Vec<_> = ds
        .iter()
        .filter(|d| d.severity == relca_core::diag::Severity::Error)
        .collect();
    assert!(!errors.is_empty());
    assert!(
        errors
            .iter()
            .all(|d| d.code == "U-SEM-0" && d.message == "loop_expansion_limit"),
        "{ds:?}"
    );
}

#[test]
fn each_repeated_parallel_join_completes_in_both_interpreters() {
    let source = r#"model ParallelRepeat version "0.2" {
      type V = enum { ok }
      actor A { capacity 1; capability work() -> V { duration fixed 2min; result nondet { ok } } }
      actor B { capacity 1; capability work() -> V { duration fixed 3min; result nondet { ok } } }
      process P { repeat wave at_most 2 { fork f { a = A.work(); b = B.work() }; join all f; again } on_limit { end done } }
      bounds { cases 1; live_tasks 2; clock 20min }
      verify reachable end.done
    }"#;
    let m = lower::lower(&syntax::parse(source).unwrap()).unwrap();
    assert_eq!(m.process.forks, ["@loop:wave:1:f", "@loop:wave:2:f"]);
    let r = reference::explore(&m);
    assert_eq!(r.stop, reference::StopReason::Complete);
    for out in [engine::explore(&m), r.outcome] {
        let [t] = out.terminals.as_slice() else {
            panic!("{:?}", out.diagnostics)
        };
        assert_eq!(
            (t.elapsed.min, t.elapsed.max, t.work.min, t.work.max),
            (6, 6, 10, 10)
        );
    }
}

fn independent(source: &str, expected: relca_core::Verdict) {
    let m = lower::lower(&syntax::parse(source).unwrap()).unwrap();
    let r = reference::explore(&m);
    assert_eq!(
        r.stop,
        reference::StopReason::Complete,
        "{:?}",
        r.outcome.diagnostics
    );
    let e = engine::explore(&m);
    let (a, b) = (
        e.properties.last().unwrap().verdict,
        r.outcome.properties.last().unwrap().verdict,
    );
    assert_eq!((a, b), (expected, expected), "engine/reference");
    assert_eq!(e.properties, r.outcome.properties);
    assert_eq!(e.terminals, r.outcome.terminals);
}

fn replace_process(source: &str, name: &str, next: &str) -> String {
    let start = source.find(&format!("  process {name} {{")).unwrap();
    let end = source
        .find("  cut ")
        .or_else(|| source.find("  bounds "))
        .unwrap();
    assert!(start < end);
    format!(
        "{}  process {name} {{ {next} }}\n{}",
        &source[..start],
        &source[end..]
    )
}

#[test]
fn both_interpreters_agree_on_round_contract_controls_and_faults() {
    use relca_core::Verdict::{Satisfied, Violated};
    let e5 = include_str!("bounded_repetition/models/E5.relca");
    independent(e5, Violated);
    independent(
        &replace_once(e5, "reads [Mail]", "reads [Mail, Invoice]"),
        Satisfied,
    );
    let disjoint_span_fault = replace_once(
        e5,
        "capability review()",
        "capability choose() -> V { duration fixed 1min; result nondet { pass, fail } }; capability review()",
    );
    let disjoint_span_fault = replace_once(
        &disjoint_span_fault,
        "r = Rev.review()\n      if r == pass { leave } else { f = Clerk.fix(); again }",
        "r = Rev.choose()\n      if r == fail { f = Clerk.fix(); again } else { v = Rev.review(); leave }",
    );
    independent(&disjoint_span_fault, Violated);

    let r0 = include_str!("record_provenance/r0.relca");
    let g5 = replace_once(
        r0,
        "    r = Clerk.register()\n    a = Clerk.judge(e)\n    b = Reviewer.review(e)\n    if a == pass && b == pass {} else { end returned }\n    x = Ledger.post(a, b)",
        "    repeat work at_most 2 { r = Clerk.register(); b = Reviewer.review(e); if b == pass { leave } else { again } } on_limit { end returned }\n    x = Ledger.post(e, b)",
    );
    independent(&g5, Satisfied);
    independent(
        &replace_once(
            &g5,
            "r = Clerk.register(); b = Reviewer.review(e)",
            "b = Reviewer.review(e); r = Clerk.register()",
        ),
        Violated,
    );
    let limit = replace_once(
        &g5,
        "repeat work at_most 2 { r = Clerk.register(); b = Reviewer.review(e); if b == pass { leave } else { again } } on_limit { end returned }",
        "repeat work at_most 2 { wait 1min; again } on_limit { r = Clerk.register(); b = Reviewer.review(e); leave }",
    );
    independent(&limit, Satisfied);

    let split = include_str!("../../../skills/relca-design/examples/split-first-repeat.relca");
    let v5 = replace_once(
        split,
        "separation Lead.split.finish, Reviewer.check.finish",
        "separation Lead.split.finish, Reviewer.check.finish\n    same_version Reviewer.check_plan(p), Dev.refine_a(p)",
    );
    independent(&v5, Satisfied);
    let v5bad = replace_once(
        &v5,
        "process SplitFirst {",
        "process SplitFirst { p0 = Lead.split()",
    );
    independent(
        &replace_once(&v5bad, "a = Dev.refine_a(p)", "a = Dev.refine_a(p0)"),
        Violated,
    );
    let v5limit = replace_once(
        &v5,
        "      if c == pass { leave } else { again }\n    } on_limit { end escalated }",
        "      again\n    } on_limit { p_last = Lead.split(); c_last = Reviewer.check_plan(p_last); leave }",
    );
    let v5limit = replace_once(&v5limit, "a = Dev.refine_a(p)", "a = Dev.refine_a(p_last)");
    let v5limit = replace_once(&v5limit, "<= 0.01", "<= 1");
    independent(&v5limit, Satisfied);
}

#[test]
fn parallel_repetition_with_contracts_matches_the_independent_execution() {
    let source = include_str!("../../../skills/relca-design/examples/check-at-end.relca");
    let folded = replace_process(
        source,
        "CheckAtEnd",
        "repeat attempt at_most 2 {
        p = Lead.split(); fork w { a = Dev.refine_a(p); b = Dev.refine_b(p) } join all w
        x = Lead.integrate(a,b); v = Reviewer.check(p,x)
        if v == pass { leave } else { again }
      } on_limit { end escalated }
      r = Lead.release(x); end delivered",
    );
    for s in [source, &folded] {
        independent(s, relca_core::Verdict::Satisfied);
        let r = relca_core::report::check(s);
        assert_eq!(r.verdict, relca_core::Verdict::Violated);
        let property = r
            .properties
            .iter()
            .find(|p| p.id == "probability reached #1")
            .unwrap();
        assert_eq!(
            property.probability.as_ref().unwrap().min.to_string(),
            "783/20000"
        );
    }
    let split = include_str!("../../../skills/relca-design/examples/split-first.relca");
    let folded_split = replace_process(
        split,
        "SplitFirst",
        "repeat attempt at_most 2 {
        p = Lead.split(); c = Reviewer.check_plan(p)
        if c == pass { fork w { a = Dev.refine_a(p); b = Dev.refine_b(p) } join all w
          x = Lead.integrate(a,b); v = Reviewer.check(p,x)
          if v == pass { r = Lead.release(x); end delivered } else { end late_fail }
        } else { again }
      } on_limit { end escalated }",
    );
    independent(split, relca_core::Verdict::Satisfied);
    independent(&folded_split, relca_core::Verdict::Satisfied);
}

#[test]
fn repeated_join_waits_and_second_round_refusal_agree() {
    let pair = include_str!("bounded_repetition/models/fork-pair.relca");
    let pair = replace_once(pair, "  policy fifo tie_by case_id then call_order\n", "");
    for refused in [false, true] {
        let input = if refused {
            replace_once(
                &pair,
                "capacity 1; capability work() -> V { duration fixed 3min",
                "capacity 1; capability work() -> V { requires wave.round == 1 else denied; duration fixed 3min",
            )
        } else {
            pair.clone()
        };
        let m = lower::lower_with_selection(&syntax::parse(&input).unwrap(), Some("Loop"), None)
            .unwrap();
        let e = engine::explore(&m);
        let r = reference::explore(&m);
        assert_eq!(r.stop, reference::StopReason::Complete);
        assert_eq!(e.terminals, r.outcome.terminals);
        assert_eq!(e.properties, r.outcome.properties);
        let joins = |m: &relca_core::outcome::Outcome| {
            m.metrics.as_ref().unwrap().cases[0]
                .joins
                .as_ref()
                .unwrap()
                .clone()
        };
        let actual = joins(&e);
        assert_eq!(actual, joins(&r.outcome));
        assert_eq!(
            actual.iter().map(|r| r.fork.as_str()).collect::<Vec<_>>(),
            ["@loop:wave:1:f", "@loop:wave:2:f"]
        );
        assert_eq!(e.case_outcomes, r.outcome.case_outcomes);
        assert_eq!(
            e.case_outcomes.as_ref().unwrap()[0].terminals[0]
                .elapsed
                .unwrap()
                .min,
            if refused { 5 } else { 6 }
        );
        if !refused {
            let m =
                lower::lower_with_selection(&syntax::parse(&input).unwrap(), Some("Hand"), None)
                    .unwrap();
            let mut hand = joins(&engine::explore(&m));
            for (hand, round) in hand.iter_mut().zip(&actual) {
                hand.fork.clone_from(&round.fork);
                for (h, r) in hand.branches.iter_mut().zip(&round.branches) {
                    h.binding.clone_from(&r.binding);
                }
            }
            assert_eq!(actual, hand);
        }
    }
}

#[test]
fn unrelated_outer_snapshot_keeps_the_legacy_lexical_scope() {
    use relca_core::Verdict::Unsupported;
    let s = include_str!("bounded_repetition/models/E5.relca");
    let s = replace_once(
        s,
        "capability review()",
        "capability choose() -> V { duration fixed 1min; result nondet { pass, fail } }; capability review()",
    );
    let with = |wait: &str| {
        replace_process(
            &s,
            "P",
            &format!(
                "e = Reader.enter(); p = Rev.choose(); if p == pass {{}} else {{ f = Clerk.fix(); end escalated }}; {wait}; y = Rev.review(); end done"
            ),
        )
    };
    independent(&with("wait 1min"), Unsupported);
    independent(
        &replace_process(
            &s,
            "P",
            "e = Reader.enter(); p = Rev.choose(); if p == pass {} else { f = Clerk.fix(); end escalated }; repeat work at_most 1 { y = Rev.review(); leave } on_limit { end escalated }; end done",
        ),
        Unsupported,
    );
    independent(
        &with("repeat work at_most 1 { wait 1min; leave } on_limit { end escalated }"),
        Unsupported,
    );
    let a = with("repeat work at_most 1 { wait 1min; leave } on_limit { end escalated }");
    independent(
        &replace_once(
            &a,
            "e = Reader.enter()",
            "e = Reader.enter() response_loss none",
        ),
        Unsupported,
    );
}

#[test]
fn wait_in_repeat_preserves_shared_state_progress_and_deadlock() {
    let hand = include_str!("support/q4/condition.relca");
    let looped = replace_once(
        hand,
        "wait until Board.ready == true; r = Desk.run(); end done",
        "repeat hold at_most 1 { wait until Board.ready == true; r = Desk.run(); leave } on_limit { end done }; end done",
    );
    for write in [true, false] {
        let run = |s: &str| {
            let input = if write {
                s.to_string()
            } else {
                replace_once(s, "finish { ready = true }", "finish { ready = false }")
            };
            let m = lower::lower(&syntax::parse(&input).unwrap()).unwrap();
            let engine = engine::explore(&m);
            let reference = reference::explore(&m);
            assert_eq!(reference.stop, reference::StopReason::Complete);
            assert_eq!(engine.terminals, reference.outcome.terminals);
            assert_eq!(engine.properties, reference.outcome.properties);
            for r in [&engine, &reference.outcome] {
                assert_eq!(
                    r.diagnostics.iter().any(|d| d.code == "W-DEADLOCK-0"),
                    !write
                );
            }
            (engine.terminals, engine.properties)
        };
        assert_eq!(run(hand), run(&looped));
    }
}

#[test]
fn refused_round_publishes_the_new_result_identity_instead_of_reusing_one() {
    use relca_core::Verdict::{Satisfied, Violated};
    let source = r#"model RefuseRound version "0.2" {
        type V = enum { ok, bad, fail }
        observed case { enabled: bool = false }
        actor Lead { capacity 1
          capability split() -> V { requires work.round <= 1 else bad; duration fixed 1min; result nondet { ok } }
          capability first() -> V { duration fixed 1min; result nondet { ok } }
          capability blocked() -> V { requires enabled == true else bad; duration fixed 1min; result nondet { ok } }
        }
        actor Review { capacity 1; capability check(p:V) -> V { duration fixed 1min; result nondet { fail } } }
        actor Dev { capacity 1; capability refine(p:V) -> V { duration fixed 1min; result nondet { ok } } }
        process P {
          repeat work at_most 2 { p = Lead.split(); if p == bad { leave } else { c = Review.check(p); if c == ok { leave } else { again } } }
          on_limit { end limit }
          a = Dev.refine(p); end done
        }
        bounds { cases 1; live_tasks 1; clock 20min }
        verify reachable end.done
        contract VCheck { same_version Review.check(p), Dev.refine(p) }
    }"#;
    let hand = replace_once(
        source,
        "repeat work at_most 2 { p = Lead.split(); if p == bad { leave } else { c = Review.check(p); if c == ok { leave } else { again } } }\n          on_limit { end limit }\n          a = Dev.refine(p); end done",
        "p1 = Lead.first(); c = Review.check(p1); p2 = Lead.blocked(); a = Dev.refine(p2); end done",
    );
    let hand = replace_once(&hand, "requires work.round <= 1 else bad;", "");
    independent(&hand, Violated);
    independent(source, Violated);
    let positive = replace_once(
        source,
        "a = Dev.refine(p)",
        "rec = Review.check(p); a = Dev.refine(p)",
    );
    independent(&positive, Satisfied);
    let positive = replace_once(&positive, "work.round <= 1", "work.round >= 2");
    independent(&positive, Satisfied);
    let inside = replace_once(
        source,
        "if p == bad { leave }",
        "if p == bad { rec = Review.check(p); leave }",
    );
    independent(
        &replace_once(&inside, "work.round <= 1", "work.round >= 2"),
        Satisfied,
    );
    for source in [source, positive.as_str()] {
        let pair = replace_once(
            source,
            "bounds { cases 1; live_tasks 1; clock 20min }",
            "bounds { cases 2; live_tasks 2; clock 20min }\narrivals fixed [0min, 5min]",
        );
        independent(
            &pair,
            if source == positive {
                Satisfied
            } else {
                Violated
            },
        );
    }
}
