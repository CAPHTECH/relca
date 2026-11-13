use relca_core::{engine, lower, reference, syntax};

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
