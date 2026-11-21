use relca_core::{Verdict, engine, ir, lower, reference, syntax};

const E1: &str = include_str!("bounded_repetition/models/eb-alias-e1.relca");
const E2: &str = include_str!("bounded_repetition/models/eb-alias-e2.relca");
const E4: &str = include_str!("bounded_repetition/models/eb-alias-e4.relca");
const E5: &str = include_str!("bounded_repetition/models/eb-alias-e5.relca");
const E6: &str = include_str!("bounded_repetition/models/eb-alias-e6.relca");

fn edit(src: &str, old: &str, new: &str) -> String {
    assert_eq!(src.matches(old).count(), 1, "{old}");
    src.replace(old, new)
}

fn check_basis(m: &ir::Model, expected: Verdict) {
    let index = m
        .contracts
        .iter()
        .position(|c| matches!(c.kind, ir::ContractKind::EvidenceBasis { .. }))
        .unwrap();
    let id = ir::Property::Contract(ir::ClauseId(index));
    let reference = reference::explore(m);
    assert_eq!(reference.stop, reference::StopReason::Complete);
    for out in [engine::explore(m), reference.outcome] {
        let row = out.properties.iter().find(|p| p.property == id).unwrap();
        assert_eq!(row.verdict, expected);
    }
}

fn paired_processes(base: &str, looped: &str, hand: &str) -> String {
    let start = base.find("  process Repeated").unwrap();
    let end = base.find("  configuration Loop").unwrap();
    format!(
        "{}\n  process Repeated {{ {looped} }}\n  process Written {{ {hand} }}\n{}",
        &base[..start],
        &base[end..]
    )
}

fn read_in_continuation() -> String {
    let src = edit(E2, "draft() -> V { reads [brief];", "draft() -> V {");
    let src = edit(&src, "use(d: V) -> V {", "use(d: V) -> V { reads [brief];");
    edit(&src, "patch() -> V { reads [Doc];", "patch() -> V {")
}

fn pair_basis(src: &str, expected: Verdict) -> ir::Model {
    let ast = syntax::parse(src).unwrap();
    for configuration in ["Hand", "Loop"] {
        let m = lower::lower_with_selection(&ast, Some(configuration), None).unwrap();
        check_basis(&m, expected);
        if configuration == "Loop" {
            return m;
        }
    }
    unreachable!()
}

const EARLY_LEAVE: &str =
    "repeat rework at_most 1 { d = Clerk.draft(); leave } on_limit { end escalated }";
const TERMINATING_WRITER: &str =
    "g = Clerk.review(); if g == fail { p = Clerk.patch(); end rejected } else {}";

fn final_record_leave() -> String {
    let src = edit(E1, "actor Boss {", "actor Notes {}\n  actor Boss {");
    let src = edit(
        &src,
        "include [Doc, Clerk, Boss]",
        "include [Doc, Clerk, Boss, Notes]",
    );
    let src = edit(&src, "reads [Doc];", "");
    let src = edit(
        &src,
        "capability review()",
        "capability save() -> V { reads [Doc]; registers [Notes]; duration fixed 1min; result nondet { pass } }\n    capability review()",
    );
    let src = edit(&src, "use(d: V) -> V {", "use(d: V) -> V { reads [Notes];");
    paired_processes(
        &src,
        "repeat rework at_most 1 { d = Clerk.draft(); c = Clerk.review(); if c == pass { leave } else {}; r = Clerk.save(); leave } on_limit { end escalated }; u = Boss.use(d); end done",
        "d = Clerk.draft(); c = Clerk.review(); if c == pass { u1 = Boss.use(d); end done } else { r = Clerk.save(); u2 = Boss.use(d); end done }",
    )
}

#[test]
fn final_leave_does_not_borrow_a_later_registration() {
    // bare_0: the early exit has an empty record, despite the final exit
    // registering Doc. They must not use the same static proof.
    pair_basis(&final_record_leave(), Verdict::Violated);
}

fn snapshot_before_either_leave(terminates: bool) -> String {
    let src = edit(
        &read_in_continuation(),
        "capability review()",
        "capability restore() -> V { reads [Doc]; writes brief; duration fixed 1min; result nondet { pass } }\n    capability review()",
    );
    let arm_end = if terminates { "; end rejected" } else { "" };
    let prefix = format!(
        "d = Clerk.draft(); g = Clerk.review(); if g == fail {{ q = Clerk.patch(){arm_end} }} else {{}}; c = Clerk.review()"
    );
    paired_processes(
        &src,
        &format!(
            "i = Clerk.intake(); repeat rework at_most 1 {{ {prefix}; if c == pass {{ leave }} else {{}}; p = Clerk.restore(); leave }} on_limit {{ end escalated }}; u = Boss.use(d); end done"
        ),
        &format!(
            "i = Clerk.intake(); {prefix}; if c == pass {{ u1 = Boss.use(d); end done }} else {{ p = Clerk.restore(); u2 = Boss.use(d); end done }}"
        ),
    )
}

#[test]
fn final_leave_does_not_erase_the_terminating_writer_mask() {
    // bare_2: lexical may still includes the terminating arm's write at the
    // early exit. Reading after restore at the second exit is certain.
    pair_basis(&snapshot_before_either_leave(true), Verdict::Unsupported);
}

#[test]
fn final_leave_does_not_turn_an_early_uncertainty_into_a_violation() {
    // bare_3: either pending snapshot can actually reach the first leave;
    // do not apply the second exit's certainty to the first exit.
    pair_basis(&snapshot_before_either_leave(false), Verdict::Unsupported);
}

#[test]
fn final_leave_does_not_replace_the_exit_reported_in_a_witness() {
    // j1b_4_1: only round 1's early exit lacks Doc. Even if the second
    // review requests another round, that cannot change the first witness.
    let src = edit(
        &read_in_continuation(),
        "intake() -> V { reads [Doc];",
        "intake() -> V {",
    );
    let src = edit(
        &src,
        "patch() -> V { writes",
        "patch() -> V { reads [Doc]; writes",
    );
    let src = paired_processes(
        &src,
        "i = Clerk.intake(); repeat rework at_most 2 { d = Clerk.draft(); c = Clerk.review(); if c == pass { leave } else {}; p = Clerk.patch(); g = Clerk.review(); if g == fail { again } else {}; leave } on_limit { end escalated }; u = Boss.use(d); end done",
        "i = Clerk.intake(); d1 = Clerk.draft(); c1 = Clerk.review(); if c1 == pass { u1 = Boss.use(d1); end done } else { p1 = Clerk.patch(); g1 = Clerk.review(); if g1 == fail { d2 = Clerk.draft(); c2 = Clerk.review(); if c2 == pass { u3 = Boss.use(d2); end done } else { p2 = Clerk.patch(); g2 = Clerk.review(); if g2 == fail { end escalated } else { u4 = Boss.use(d2); end done } } } else { u2 = Boss.use(d1); end done } }",
    );
    pair_basis(&src, Verdict::Violated);
    let e = witness(
        &src,
        Some("Loop"),
        &[("rework", 1, ir::LoopPhase::Body)],
        "Doc",
    );
    assert_eq!(e.repetition_exits[0].leave_site, Some(1));
    assert_eq!(witness(&src, Some("Hand"), &[], "Doc").binding, "u1");
}

#[test]
fn a_second_exit_write_in_expanded_ir_fails_closed() {
    fn unguard_last_leave(block: &mut ir::Block) -> bool {
        for statement in block {
            if let ir::Stmt::If {
                then_body,
                else_body,
                ..
            } = &mut statement.kind
            {
                if let [last] = then_body.as_slice()
                    && let ir::Stmt::LoopControl(c) = &last.kind
                    && c.updates
                        .iter()
                        .any(|(_, v)| *v == ir::StateValue::Integer(3))
                {
                    // Lowering emitted a guard; remove only that guard, leaving
                    // the statically checked proof and exit numbers untouched.
                    statement.kind = last.kind.clone();
                    return true;
                }
                if unguard_last_leave(then_body) || unguard_last_leave(else_body) {
                    return true;
                }
            }
        }
        false
    }
    let source = final_record_leave();
    let mut m = pair_basis(&source, Verdict::Violated);
    assert!(unguard_last_leave(&mut m.process.body));
    let e = engine::explore(&m);
    assert!(
        e.diagnostics
            .iter()
            .any(|d| d.message.contains("repetition exit already selected")),
        "{:?}",
        e.diagnostics
    );
    let r = reference::explore(&m);
    assert_eq!(r.stop, reference::StopReason::SemanticAbort);
    for out in [e, r.outcome] {
        let basis = out
            .properties
            .iter()
            .find(|p| p.property == ir::Property::Contract(ir::ClauseId(0)))
            .unwrap();
        assert_ne!(basis.verdict, Verdict::Satisfied);
    }
}

#[test]
fn a_guarded_final_leave_preserves_the_next_round_for_both_checks() {
    let src = edit(E2, "patch() -> V { reads [Doc];", "patch() -> V {");
    for in_body in [false, true] {
        // The failing branch overwrites brief and goes straight to the next
        // round. A final leave must not hide the check that then lacks Doc.
        let in_round = if in_body { "u = Boss.use(d);" } else { "" };
        let after = if in_body { "" } else { "u = Boss.use(d);" };
        let looped = format!(
            "i = Clerk.intake(); repeat rework at_most 2 {{ d = Clerk.draft(); {in_round} c = Clerk.review(); if c == fail {{ p = Clerk.patch(); again }} else {{}}; leave }} on_limit {{ end escalated }}; {after} end done"
        );
        let hand_before = |n| {
            if in_body {
                format!("u{n} = Boss.use(d{n});")
            } else {
                String::new()
            }
        };
        let hand_pass = |n| {
            if in_body {
                String::new()
            } else {
                format!("u{n} = Boss.use(d{n});")
            }
        };
        let hand = format!(
            "i = Clerk.intake(); d1 = Clerk.draft(); {} c1 = Clerk.review(); if c1 == pass {{ {} end done }} else {{ p1 = Clerk.patch(); d2 = Clerk.draft(); {} c2 = Clerk.review(); if c2 == pass {{ {} end done }} else {{ p2 = Clerk.patch(); end escalated }} }}",
            hand_before(1),
            hand_pass(1),
            hand_before(2),
            hand_pass(2)
        );
        pair_basis(&paired_processes(&src, &looped, &hand), Verdict::Violated);
    }
}

fn check_distinct_spellings(src: &str, hand: Verdict, looped: Verdict) {
    let ast = syntax::parse(src).unwrap();
    for (configuration, verdict) in [("Hand", hand), ("Loop", looped)] {
        let m = lower::lower_with_selection(&ast, Some(configuration), None).unwrap();
        check_basis(&m, verdict);
        if verdict == Verdict::Unsupported {
            assert!(
                engine::explore(&m)
                    .diagnostics
                    .iter()
                    .any(|d| d.message.contains("evidence_basis_conditional_snapshot"))
            );
        }
    }
}

#[test]
fn multiple_again_sites_keep_the_conservative_snapshot_difference() {
    let base = read_in_continuation();
    let looped = "i = Clerk.intake(); repeat rework at_most 2 { d = Clerk.draft(); c = Clerk.review(); if c == pass { leave } else { g = Clerk.review(); if g == pass { p = Clerk.patch(); again } else { again } } } on_limit { end escalated }; u = Boss.use(d); end done";
    let hand = "i = Clerk.intake(); d1 = Clerk.draft(); c1 = Clerk.review(); if c1 == pass { u1 = Boss.use(d1); end done } else { g1 = Clerk.review(); if g1 == pass { p1 = Clerk.patch(); d2 = Clerk.draft(); c2 = Clerk.review(); if c2 == pass { u2 = Boss.use(d2); end done } else { end escalated } } else { d3 = Clerk.draft(); c3 = Clerk.review(); if c3 == pass { u3 = Boss.use(d3); end done } else { end escalated } } }";
    let src = paired_processes(&base, looped, hand);
    // c1_9_0: Hand's patch-only continuation definitely lacks Doc. Loop
    // joins the again sites and cannot tell which last writer reaches round 2.
    check_distinct_spellings(&src, Verdict::Violated, Verdict::Unsupported);
    let swapped = edit(&src, "intake() -> V { reads [Doc];", "intake() -> V {");
    let swapped = edit(
        &swapped,
        "patch() -> V { writes",
        "patch() -> V { reads [Doc]; writes",
    );
    // The first possible leave now definitely lacks Doc in both spellings.
    pair_basis(&swapped, Verdict::Violated);

    // c1_10_0: with one shared continuation in Hand, the same two last
    // writers are still possible and both spellings withhold a verdict.
    let shared = "i = Clerk.intake(); d1 = Clerk.draft(); c1 = Clerk.review(); if c1 == pass { u1 = Boss.use(d1); end done } else { g1 = Clerk.review(); if g1 == pass { p1 = Clerk.patch() } else {}; d2 = Clerk.draft(); c2 = Clerk.review(); if c2 == pass { u2 = Boss.use(d2); end done } else { end escalated } }";
    pair_basis(
        &paired_processes(&base, looped, shared),
        Verdict::Unsupported,
    );

    // The same conservative join applies to the legacy in-round reader.
    let inside = edit(
        looped,
        "c = Clerk.review();",
        "u = Boss.use(d); c = Clerk.review();",
    );
    let inside = edit(&inside, "; u = Boss.use(d); end done", "; end done");
    let inside_hand = [1, 2, 3].into_iter().fold(hand.to_owned(), |s, n| {
        let s = edit(&s, &format!("u{n} = Boss.use(d{n}); end done"), "end done");
        edit(
            &s,
            &format!("c{n} = Clerk.review();"),
            &format!("u{n} = Boss.use(d{n}); c{n} = Clerk.review();"),
        )
    });
    check_distinct_spellings(
        &paired_processes(&base, &inside, &inside_hand),
        Verdict::Violated,
        Verdict::Unsupported,
    );
}

#[test]
fn no_alias_argument_keeps_the_legacy_snapshot_difference() {
    let src = edit(&read_in_continuation(), "use(d: V) -> V {", "use() -> V {");
    let src = ["d", "d1", "d2"].into_iter().fold(src, |s, arg| {
        edit(&s, &format!("Boss.use({arg})"), "Boss.use()")
    });
    // na_noalias: without an alias argument the reader still uses the
    // legacy snapshot rule. The separate Hand calls can see round 2's patch.
    check_distinct_spellings(&src, Verdict::Violated, Verdict::Unsupported);
}

#[test]
fn alias_through_a_lossy_snapshot_writer_reaches_a_later_reader() {
    let src = edit(
        E2,
        "observed case { brief: optional<V> }",
        "observed case { brief: optional<V>; memo: optional<V> }",
    );
    let src = edit(
        &src,
        "capability use(d: V) -> V {",
        "capability relay(d: V) -> V { writes memo; duration fixed 1min; result nondet { pass } }\n    capability use() -> V { reads [memo];",
    );
    let src = [("u", "d"), ("u1", "d1"), ("u2", "d2")].into_iter().fold(src, |s, (u, d)| {
        edit(&s, &format!("{u} = Boss.use({d})"), &format!("s{u} = Boss.relay({d}) response_loss probability 0.5 basis \"test: half of first receipts are delivered\" retry max_attempts 2 when receipt unknown backoff fixed 1min exhausted end escalated; {u} = Boss.use()"))
    });
    // A delivered first receipt reaches use; an unknown non-idempotent
    // response cannot be cleared by retry. A blind round-2 patch loses Doc
    // on the delivered path, so neither expected verdict can be vacuous.
    pair_basis(&src, Verdict::Satisfied);
    pair_basis(
        &edit(&src, "patch() -> V { reads [Doc];", "patch() -> V {"),
        Verdict::Violated,
    );
}

#[test]
fn joining_an_alias_and_an_independently_registered_record_keeps_seven_hand_verdicts() {
    let src = final_record_leave();
    let src = edit(
        &src,
        "capability save()",
        "capability clear() -> V { registers [Notes]; duration fixed 1min; result nondet { pass } }\n    capability save()",
    );
    let cases = [
        // Early only, late only, both, then a late overwrite of an old record.
        (
            "d = Clerk.draft(); c = Clerk.review(); if c == pass { r = Clerk.save(); leave } else {}; leave",
            "d = Clerk.draft(); c = Clerk.review(); if c == pass { r = Clerk.save(); u1 = Boss.use(d); end done } else { u2 = Boss.use(d); end done }",
            Verdict::Violated,
        ),
        (
            "d = Clerk.draft(); c = Clerk.review(); if c == pass { leave } else {}; r = Clerk.save(); leave",
            "d = Clerk.draft(); c = Clerk.review(); if c == pass { u1 = Boss.use(d); end done } else { r = Clerk.save(); u2 = Boss.use(d); end done }",
            Verdict::Violated,
        ),
        (
            "d = Clerk.draft(); r = Clerk.save(); c = Clerk.review(); if c == pass { leave } else {}; leave",
            "d = Clerk.draft(); r = Clerk.save(); c = Clerk.review(); if c == pass { u1 = Boss.use(d); end done } else { u2 = Boss.use(d); end done }",
            Verdict::Satisfied,
        ),
        (
            "d = Clerk.draft(); r = Clerk.save(); c = Clerk.review(); if c == pass { leave } else {}; lost = Clerk.clear(); leave",
            "d = Clerk.draft(); r = Clerk.save(); c = Clerk.review(); if c == pass { u1 = Boss.use(d); end done } else { lost = Clerk.clear(); u2 = Boss.use(d); end done }",
            Verdict::Violated,
        ),
        // Optional registration can reach the early exit, but need not.
        (
            "d = Clerk.draft(); g = Clerk.review(); if g == pass { r = Clerk.save() } else {}; c = Clerk.review(); if c == pass { leave } else {}; s = Clerk.save(); leave",
            "d = Clerk.draft(); g = Clerk.review(); if g == pass { r = Clerk.save() } else {}; c = Clerk.review(); if c == pass { u1 = Boss.use(d); end done } else { s = Clerk.save(); u2 = Boss.use(d); end done }",
            Verdict::Unsupported,
        ),
    ];
    for (body, hand, expected) in cases {
        pair_basis(
            &paired_processes(
                &src,
                &format!(
                    "repeat rework at_most 1 {{ {body} }} on_limit {{ end escalated }}; u = Boss.use(d); end done"
                ),
                hand,
            ),
            expected,
        );
    }
    // The on_limit writer reaches that exit only; the early body exit is empty.
    let limit = paired_processes(
        &src,
        "repeat rework at_most 1 { d = Clerk.draft(); c = Clerk.review(); if c == pass { leave } else { again } } on_limit { r = Clerk.save(); leave }; u = Boss.use(d); end done",
        "d = Clerk.draft(); c = Clerk.review(); if c == pass { u1 = Boss.use(d); end done } else { r = Clerk.save(); u2 = Boss.use(d); end done }",
    );
    pair_basis(&limit, Verdict::Violated);
    // Once the case goes around again, the preceding registration carries Doc.
    let prior = paired_processes(
        &src,
        "repeat rework at_most 2 { d = Clerk.draft(); c = Clerk.review(); if c == pass { leave } else { r = Clerk.save(); again } } on_limit { end escalated }; u = Boss.use(d); end done",
        "d1 = Clerk.draft(); c1 = Clerk.review(); if c1 == pass { u1 = Boss.use(d1); end done } else { r1 = Clerk.save(); d2 = Clerk.draft(); c2 = Clerk.review(); if c2 == pass { u2 = Boss.use(d2); end done } else { r2 = Clerk.save(); end escalated } }",
    );
    pair_basis(&prior, Verdict::Violated);
}

#[test]
fn only_the_second_exit_needs_the_conditional_mask_in_each_continuation() {
    let base = edit(
        &read_in_continuation(),
        "capability patch()",
        "capability recover() -> V { reads [Doc]; writes brief; duration fixed 1min; result nondet { pass } }\n    capability patch()",
    );
    let first = "repeat rework at_most 2 { d = Clerk.draft(); c = Clerk.review(); if c == pass { leave } else { p = Clerk.patch(); again } } on_limit { end escalated }";
    // Intake (round 1) and recover both carry Doc. After the patch (round 2),
    // only the optional recover can restore Doc. Using round 1's empty mask
    // on round 2 would report a definite violation on the no-recovery path.
    let make_optional = |s: &str| {
        format!("g{s} = Clerk.review(); if g{s} == pass {{ r{s} = Clerk.recover() }} else {{}}")
    };
    let make_suffix = |kind, s: &str| match kind {
        0 => format!("{}; u{s} = Boss.use(d{s}); end done", make_optional(s)),
        1 => format!(
            "{}; v{s} = Clerk.review(); if v{s} == pass {{ u{s} = Boss.use(d{s}); end done }} else {{ end escalated }}",
            make_optional(s)
        ),
        _ => format!(
            "v{s} = Clerk.review(); {}; u{s} = Boss.use(d{s}); if v{s} == pass {{ end done }} else {{ end escalated }}",
            make_optional(s)
        ),
    };
    for kind in 0..3 {
        let loop_suffix = match kind {
            0 => make_suffix(0, ""),
            1 => format!(
                "{}; repeat inspect at_most 1 {{ v = Clerk.review(); if v == pass {{ leave }} else {{ again }} }} on_limit {{ end escalated }}; u = Boss.use(d); end done",
                make_optional("")
            ),
            _ => format!(
                "repeat inspect at_most 1 {{ v = Clerk.review(); {}; u = Boss.use(d); if v == pass {{ leave }} else {{ again }} }} on_limit {{ end escalated }}; end done",
                make_optional("")
            ),
        };
        let looped = format!("i = Clerk.intake(); {first}; {loop_suffix}");
        let hand = format!(
            "i = Clerk.intake(); d1 = Clerk.draft(); c1 = Clerk.review(); if c1 == pass {{ {} }} else {{ p1 = Clerk.patch(); d2 = Clerk.draft(); c2 = Clerk.review(); if c2 == pass {{ {} }} else {{ end escalated }} }}",
            make_suffix(kind, "1"),
            make_suffix(kind, "2")
        );
        let m = pair_basis(
            &paired_processes(&base, &looped, &hand),
            Verdict::Unsupported,
        );
        let ir::ContractKind::EvidenceBasis { calls, .. } = &m.contracts[0].kind else {
            panic!()
        };
        let u = calls
            .iter()
            .find(|c| {
                m.process.bindings[c.binding.0].name
                    == if kind == 2 { "@loop:inspect:1:u" } else { "u" }
            })
            .unwrap();
        let doc = ir::ActorId(m.actors.iter().position(|a| a.name == "Doc").unwrap());
        assert_eq!(u.exit_scopes.len(), 2);
        for scope in &u.exit_scopes {
            assert_eq!(
                scope.conditional_sources,
                if scope.exits[0].round == 2 {
                    vec![doc]
                } else {
                    vec![]
                },
                "{kind}"
            );
        }
    }
}

#[test]
fn a_terminating_arm_still_contributes_lexical_may_inside_and_outside_repeat() {
    // Hand considers a writer in a preceding if-block even when its arm ends:
    // last writer is statically uncertain, in either direction of the edit.
    let base = read_in_continuation();
    let swapped = edit(&base, "intake() -> V { reads [Doc];", "intake() -> V {");
    let swapped = edit(
        &swapped,
        "patch() -> V { writes",
        "patch() -> V { reads [Doc]; writes",
    );
    let hand = format!(
        "i = Clerk.intake(); d = Clerk.draft(); {TERMINATING_WRITER}; u = Boss.use(d); end done"
    );
    let outside = format!(
        "i = Clerk.intake(); {EARLY_LEAVE}; {TERMINATING_WRITER}; u = Boss.use(d); end done"
    );
    let inside = format!(
        "i = Clerk.intake(); repeat rework at_most 1 {{ d = Clerk.draft(); {TERMINATING_WRITER}; u = Boss.use(d); leave }} on_limit {{ end escalated }}; end done"
    );
    for base in [&base, &swapped] {
        for looped in [&inside, &outside] {
            let src = paired_processes(base, looped, &hand);
            pair_basis(&src, Verdict::Unsupported);
            let surviving_writer = src.replace(
                "{ p = Clerk.patch(); end rejected } else {}",
                "{ end rejected } else { p = Clerk.patch() }",
            );
            assert_ne!(surviving_writer, src);
            pair_basis(&surviving_writer, Verdict::Unsupported);
        }
    }
}

#[test]
fn two_leaves_in_one_round_have_distinct_proofs_and_exit_locations() {
    let src = paired_processes(
        &read_in_continuation(),
        "
        i = Clerk.intake()
        repeat rework at_most 1 {
          d = Clerk.draft(); c = Clerk.review()
          if c == pass { leave } else {}
          p = Clerk.patch(); g = Clerk.review()
          if g == pass { leave } else { again }
        } on_limit { end escalated }
        u = Boss.use(d); end done",
        "
        i = Clerk.intake(); d = Clerk.draft(); c = Clerk.review()
        if c == pass { u1 = Boss.use(d); end done } else {
          p = Clerk.patch(); g = Clerk.review()
          if g == pass { u2 = Boss.use(d); end done } else { end escalated }
        }",
    );
    // The first leave reads intake's Doc; the second follows a mandatory
    // overwrite with no Doc, not a conditional overwrite at either exit.
    pair_basis(&src, Verdict::Violated);
    let e = witness(
        &src,
        Some("Loop"),
        &[("rework", 1, ir::LoopPhase::Body)],
        "Doc",
    );
    assert_eq!(e.repetition_exits[0].leave_site, Some(2));
}

fn cases(looped: bool) -> Vec<(&'static str, String, Option<&'static str>, Verdict)> {
    use Verdict::{Satisfied, Unsupported, Violated};
    let configuration = Some(if looped { "Loop" } else { "Hand" });
    let split = if looped {
        include_str!("../../../skills/relca-design/examples/split-first-repeat.relca")
    } else {
        include_str!("../../../skills/relca-design/examples/split-first.relca")
    };
    let e3 = edit(
        split,
        "contract Controls {",
        "contract Controls {\n    evidence_basis direct Reviewer.check [Spec]",
    );
    vec![
        ("E1", E1.into(), configuration, Satisfied),
        (
            "E1 broken",
            edit(E1, "draft() -> V { reads [Doc];", "draft() -> V {"),
            configuration,
            Violated,
        ),
        ("E2", E2.into(), configuration, Satisfied),
        (
            "E2 broken",
            edit(E2, "patch() -> V { reads [Doc];", "patch() -> V {"),
            configuration,
            Violated,
        ),
        (
            "E3 broken",
            edit(
                &e3,
                "check(p:Plan,x:Part) -> Verdict { reads [Spec];",
                "check(p:Plan,x:Part) -> Verdict {",
            ),
            None,
            Violated,
        ),
        ("E3", e3, None, Satisfied),
        ("E4", E4.into(), configuration, Unsupported),
        ("E5", E5.into(), configuration, Violated),
        ("E6", E6.into(), configuration, Violated),
    ]
}

fn accept(looped: bool) {
    for (name, src, configuration, expected) in cases(looped) {
        let ast = syntax::parse(&src).unwrap_or_else(|ds| panic!("{name}: {ds:?}"));
        let m = lower::lower_with_selection(&ast, configuration, None)
            .unwrap_or_else(|ds| panic!("{name}: {ds:?}"));
        let (index, _) = m
            .contracts
            .iter()
            .enumerate()
            .find(|(_, c)| matches!(c.kind, ir::ContractKind::EvidenceBasis { .. }))
            .unwrap();
        let e = engine::explore(&m);
        let r = reference::explore(&m);
        assert_eq!(r.stop, reference::StopReason::Complete, "{name}");
        for (backend, result) in [("engine", &e), ("reference", &r.outcome)] {
            let row = result
                .properties
                .iter()
                .find(|p| p.property == ir::Property::Contract(ir::ClauseId(index)))
                .unwrap();
            assert_eq!(row.verdict, expected, "{name}: {backend}");
        }
        // The user-facing diagnostic is produced by the engine; reference
        // independently folds the contract verdict and does not emit it.
        if expected == Verdict::Unsupported {
            assert!(
                e.diagnostics.iter().any(|d| d.code == "U-SEM-0"
                    && d.message.contains("evidence_basis_conditional_snapshot")),
                "{name}: {:?}",
                e.diagnostics
            );
        }
    }
}

#[test]
fn hand_matches_decision() {
    accept(false);
}

#[test]
fn loop_matches_decision() {
    accept(true);
}

#[test]
fn snapshots_in_the_continuation_distinguish_exit_from_uncertainty() {
    // Move the read of brief to the continuation; the alias still flows to
    // the target. The source now reaches it by the last snapshot writer.
    let src = edit(E2, "draft() -> V { reads [brief];", "draft() -> V {");
    let src = edit(&src, "use(d: V) -> V {", "use(d: V) -> V { reads [brief];");
    let src = edit(&src, "patch() -> V { reads [Doc];", "patch() -> V {");
    pair_basis(&src, Verdict::Violated);
    witness(
        &src,
        Some("Loop"),
        &[("rework", 2, ir::LoopPhase::Body)],
        "Doc",
    );
}

fn witness(
    src: &str,
    configuration: Option<&str>,
    exits: &[(&str, u64, ir::LoopPhase)],
    root: &str,
) -> relca_core::explain::Evidence {
    let report = relca_core::explain::explain_with_selection(src, configuration, None);
    let trace = report
        .explanation
        .counterexamples
        .iter()
        .find(|t| t.id.ends_with("evidence_basis"))
        .unwrap_or_else(|| panic!("{:?}", report.explanation));
    assert_eq!(
        trace.replay.status,
        relca_core::explain::ReplayStatus::Verified
    );
    let evidence = trace.evidence.as_ref().unwrap();
    assert_eq!(evidence.missing, [root]);
    assert_eq!(
        evidence
            .repetition_exits
            .iter()
            .map(|e| (e.loop_name.as_str(), e.round, e.phase))
            .collect::<Vec<_>>(),
        exits
    );
    evidence.clone()
}

#[test]
fn the_proof_table_and_witness_keep_rounds_and_limit_distinct() {
    let src = edit(E2, "patch() -> V { reads [Doc];", "patch() -> V {");
    let ast = syntax::parse(&src).unwrap();
    let m = lower::lower_with_selection(&ast, Some("Loop"), None).unwrap();
    let ir::ContractKind::EvidenceBasis { calls, .. } = &m.contracts[0].kind else {
        panic!()
    };
    let after = calls
        .iter()
        .find(|c| m.process.bindings[c.binding.0].name == "u")
        .unwrap();
    assert!(after.alias_input);
    assert_eq!(after.exit_scopes.len(), 2);
    for (i, scope) in after.exit_scopes.iter().enumerate() {
        assert_eq!(scope.exits[0].round, i as u64 + 1);
        assert_eq!(scope.sources.len(), usize::from(i == 0));
        assert!(scope.conditional_sources.is_empty());
    }
    witness(
        &src,
        Some("Loop"),
        &[("rework", 2, ir::LoopPhase::Body)],
        "Doc",
    );
    witness(&src, Some("Hand"), &[], "Doc");
    witness(
        E5,
        Some("Loop"),
        &[("rework", 2, ir::LoopPhase::OnLimit)],
        "Doc",
    );
    witness(E5, Some("Hand"), &[], "Doc");
    witness(
        E6,
        Some("Loop"),
        &[
            ("rework", 2, ir::LoopPhase::Body),
            ("note", 1, ir::LoopPhase::Body),
        ],
        "Doc",
    );
    witness(E6, Some("Hand"), &[], "Doc");
    let broken = edit(E1, "draft() -> V { reads [Doc];", "draft() -> V {");
    witness(
        &broken,
        Some("Loop"),
        &[("rework", 1, ir::LoopPhase::Body)],
        "Doc",
    );
    witness(&broken, Some("Hand"), &[], "Doc");
}

#[test]
fn hand_conditional_snapshot_through_registration() {
    let start = E4.find("  process Repeated").unwrap();
    let end = E4.find("  process Written").unwrap();
    let src = format!("{}{}", &E4[..start], &E4[end..]);
    let src = edit(
        &src,
        "  configuration Loop { cut C; process Repeated }\n",
        "",
    );
    let src = edit(&src, "actor Boss {", "actor Notes {}\n  actor Boss {");
    let src = edit(
        &src,
        "include [Doc, Clerk, Boss]",
        "include [Doc, Clerk, Boss, Notes]",
    );
    let src = edit(
        &src,
        "capability use(d: V)",
        "capability register(d: V) -> V { registers [Notes]; duration fixed 1min; result nondet { pass } }\n    capability use()",
    );
    let src = edit(
        &src,
        "capability use() -> V {",
        "capability use() -> V { reads [Notes];",
    );
    let src = edit(
        &src,
        "u1 = Boss.use(d1)",
        "s1 = Boss.register(d1); u1 = Boss.use()",
    );
    let src = edit(
        &src,
        "u2 = Boss.use(d2)",
        "s2 = Boss.register(d2); u2 = Boss.use()",
    );
    // ADR 0075, sections 3.2–3.3: a record carries the registrar's must sources.
    // Round 1's draft always has Doc. In round 2, the optional patch makes
    // Doc conditional *before* registration, so it is a candidate, not a fact.
    let established = edit(
        &src,
        "patch() -> V { writes",
        "patch() -> V { reads [Doc]; writes",
    );
    // With Doc established, it is the registration itself that is optional:
    // Notes can carry Doc, but the reader has no guaranteed registrar.
    let optional = edit(
        &established,
        "s2 = Boss.register(d2);",
        "if g1 == pass { s2 = Boss.register(d2) } else {};",
    );
    for (source, expected) in [
        (&established, Verdict::Satisfied),
        (&src, Verdict::Unsupported),
        (&optional, Verdict::Unsupported),
    ] {
        let ast = syntax::parse(source).unwrap();
        let m = lower::lower_with_selection(&ast, Some("Hand"), None).unwrap();
        assert!(!m.has_loops());
        let r = reference::explore(&m);
        assert_eq!(r.stop, reference::StopReason::Complete);
        let e = engine::explore(&m);
        if expected == Verdict::Unsupported {
            assert!(
                e.diagnostics
                    .iter()
                    .any(|d| d.message.contains("evidence_basis_conditional_snapshot"))
            );
        }
        assert_eq!(
            (e.properties[0].verdict, r.outcome.properties[0].verdict),
            (expected, expected)
        );
    }
}

#[test]
fn target_can_read_a_record_that_does_not_carry_the_alias() {
    let src = edit(E4, "actor Boss {", "actor Notes {}\n  actor Boss {");
    let src = edit(
        &src,
        "include [Doc, Clerk, Boss]",
        "include [Doc, Clerk, Boss, Notes]",
    );
    let src = edit(
        &src,
        "capability draft() -> V { reads [brief];",
        "capability save() -> V { reads [brief]; registers [Notes]; duration fixed 1min; result nondet { pass } }\n    capability draft() -> V {",
    );
    let src = edit(&src, "use(d: V) -> V {", "use(d: V) -> V { reads [Notes];");
    let src = edit(
        &src,
        "d = Clerk.draft()",
        "d = Clerk.draft(); registered = Clerk.save()",
    );
    let src = edit(
        &src,
        "d1 = Clerk.draft()",
        "d1 = Clerk.draft(); r1 = Clerk.save()",
    );
    let src = edit(
        &src,
        "d2 = Clerk.draft()",
        "d2 = Clerk.draft(); r2 = Clerk.save()",
    );
    // Round 2's save can carry Doc but cannot confirm it, because brief is
    // conditional before registration. The alias d is a separate input.
    pair_basis(&src, Verdict::Unsupported);
    let good = edit(
        &src,
        "patch() -> V { writes",
        "patch() -> V { reads [Doc]; writes",
    );
    pair_basis(&good, Verdict::Satisfied);
}

#[test]
fn bounded_response_retries_of_a_consumer_see_the_same_alias() {
    let src = edit(E2, "patch() -> V { reads [Doc];", "patch() -> V {");
    // Each attempt normally finishes and checks the received alias's input.
    // Replaying the response never gives round 2 the earlier snapshot's root.
    for response in [
        "response_loss none retry max_attempts 2 when result [pass] backoff fixed 1min exhausted end done",
        "response_loss sequence [unknown,delivered] retry max_attempts 2 when receipt unknown backoff fixed 1min exhausted end done",
    ] {
        let input = ["u = Boss.use(d)", "u1 = Boss.use(d1)", "u2 = Boss.use(d2)"]
            .into_iter()
            .fold(src.clone(), |s, call| {
                edit(&s, call, &format!("{call} {response}"))
            });
        pair_basis(&input, Verdict::Violated);
    }
}

#[test]
fn too_many_exit_profiles_fail_closed() {
    let mut looped = String::new();
    for k in 0..11 {
        looped.push_str(&format!("repeat choices{k} at_most 2 {{ d{k} = Clerk.draft(); c{k} = Clerk.review(); if c{k} == pass {{ leave }} else {{ again }} }} on_limit {{ end escalated }}; "));
    }
    looped.push_str("u = Boss.use(d10); end done");
    let src = paired_processes(E1, &looped, "end done");
    let ds =
        lower::lower_with_selection(&syntax::parse(&src).unwrap(), Some("Loop"), None).unwrap_err();
    assert!(
        ds.iter()
            .any(|d| d.code == "U-SEM-0" && d.message == "loop_expansion_limit"),
        "{ds:?}"
    );
}

#[test]
fn argument_and_snapshot_aliases_are_open_but_record_aliases_are_not() {
    let forwarded = edit(
        E1,
        "actor Boss { capacity 1",
        "actor Boss { capacity 1\n    capability remember(d: V) -> V { writes brief; duration fixed 1min; result nondet { pass } }",
    );
    let forwarded = edit(
        &forwarded,
        "actor Doc {",
        "observed case { brief: optional<V> }\n  actor Doc {",
    );
    let forwarded = edit(
        &forwarded,
        "u = Boss.use(d)",
        "s = Boss.remember(d); u = Boss.use()",
    );
    let forwarded = edit(
        &forwarded,
        "u1 = Boss.use(d1)",
        "s1 = Boss.remember(d1); u1 = Boss.use()",
    );
    let forwarded = edit(
        &forwarded,
        "u2 = Boss.use(d2)",
        "s2 = Boss.remember(d2); u2 = Boss.use()",
    );
    let forwarded = edit(
        &forwarded,
        "use(d: V) -> V {",
        "use() -> V { reads [brief];",
    );
    let lowered =
        |src: &str| lower::lower_with_selection(&syntax::parse(src).unwrap(), Some("Loop"), None);
    for src in [E1, &forwarded] {
        let m = lowered(src).unwrap();
        check_basis(&m, Verdict::Satisfied);
    }
    let registered = edit(
        &forwarded,
        "observed case { brief: optional<V> }",
        "actor Notes {}",
    );
    let registered = edit(
        &registered,
        "include [Doc, Clerk, Boss]",
        "include [Doc, Clerk, Boss, Notes]",
    );
    let registered = edit(&registered, "writes brief;", "registers [Notes];");
    let registered = edit(&registered, "reads [brief];", "reads [Notes];");
    let refused = |src: &str| {
        let ds = lowered(src).unwrap_err();
        assert!(
            ds.iter()
                .any(|d| d.code == "U-SEM-0" && d.message == "loop_feature_combination"),
            "{ds:?}"
        );
    };
    refused(&registered);
    refused(&edit(
        &registered,
        "evidence_basis Boss",
        "evidence_basis direct Boss",
    ));

    let declarations = "observed case { brief: optional<V> }\n  actor Notes {}\n  actor Copy {}";
    let substrate = edit(&registered, "actor Notes {}", declarations);
    let substrate = edit(
        &substrate,
        "include [Doc, Clerk, Boss, Notes]",
        "include [Doc, Clerk, Boss, Notes, Copy]",
    );
    for (extra, before, read, arg) in [
        (
            "capability relay() -> V { reads [Notes]; registers [Copy]; duration fixed 1min; result nondet { pass } }",
            "n = Boss.relay();",
            "reads [Copy];",
            "",
        ),
        (
            "capability relay() -> V { reads [Notes]; writes brief; duration fixed 1min; result nondet { pass } }",
            "n = Boss.relay();",
            "reads [brief];",
            "",
        ),
        (
            "capability relay() -> V { reads [Notes]; duration fixed 1min; result nondet { pass } }",
            "n = Boss.relay();",
            "",
            "n",
        ),
        (
            "capability relay(d: V) -> V { writes brief; duration fixed 1min; result nondet { pass } }\n    capability snapshot_to_record() -> V { reads [brief]; registers [Copy]; duration fixed 1min; result nondet { pass } }",
            "n = Boss.relay(d); n2 = Boss.snapshot_to_record();",
            "reads [Copy];",
            "",
        ),
    ] {
        let src = edit(
            &substrate,
            "actor Boss { capacity 1",
            &format!("actor Boss {{ capacity 1\n    {extra}"),
        );
        let param = if arg.is_empty() { "" } else { "v: V" };
        let src = edit(
            &src,
            "use() -> V { reads [Notes];",
            &format!("use({param}) -> V {{ {read}"),
        );
        let src = paired_processes(
            &src,
            &format!("{EARLY_LEAVE}; s = Boss.remember(d); {before} u = Boss.use({arg}); end done"),
            "end done",
        );
        refused(&src);
    }
}
