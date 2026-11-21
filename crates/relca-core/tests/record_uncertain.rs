use relca_core::{Verdict, engine, explain, ir, lower, reference, report, syntax};

const BASE: &str = r#"model RecordEvidence version "0.2" {
    type V = enum { pass, fail }
    observed case { brief: optional<V> }
    actor Doc { source document }
    actor Policy { source document }
    actor Missing { source document }
    actor Notes {}
    actor Copy {}
    actor Clerk { capacity 1
        capability intake() -> V { reads [Doc]; writes brief; duration fixed 1min; result nondet { pass } }
        capability patch() -> V { writes brief; duration fixed 1min; result nondet { pass } }
        capability draft() -> V { reads [brief]; duration fixed 1min; result nondet { pass } }
        capability review() -> V { duration fixed 1min; result nondet { pass, fail } }
    }
    actor Boss { capacity 1
        capability register(d: V) -> V { registers [Notes]; duration fixed 1min; result nondet { pass } }
        capability copy() -> V { reads [Notes]; registers [Copy]; duration fixed 1min; result nondet { pass } }
        capability use() -> V { reads [Notes]; duration fixed 1min; result nondet { pass } }
    }
    cut Scope { include [Doc, Policy, Missing, Notes, Copy, Clerk, Boss] }
    process P { BEFORE d = Clerk.draft(); REGISTER u = Boss.use(); end done }
    bounds { cases 1; live_tasks 2; clock 40min }
    contract Basis { evidence_basis Boss.use [Doc] }
}"#;

const FORK: &str = "fork f { i = Clerk.intake() } join all f;";
const CONDITIONAL: &str =
    "i = Clerk.intake(); g = Clerk.review(); if g == pass { p = Clerk.patch() } else {};";
const REGISTER: &str = "s = Boss.register(d);";

fn model(before: &str, register: &str) -> String {
    BASE.replace("BEFORE", before).replace("REGISTER", register)
}

fn check(src: &str, want: Verdict) {
    let m = lower::lower(&syntax::parse(src).unwrap()).unwrap();
    assert!(!m.has_loops());
    let reference = reference::explore(&m);
    assert_eq!(reference.stop, reference::StopReason::Complete);
    let outcomes = [engine::explore(&m), reference.outcome];
    assert_eq!(
        outcomes.map(|out| out
            .properties
            .iter()
            .find(|p| p.property == ir::Property::Contract(ir::ClauseId(0)))
            .unwrap()
            .verdict),
        [want, want],
        "engine/reference"
    );
    let checked = report::check(src);
    let explained = explain::explain(src);
    assert_eq!(checked.verdict, want);
    assert_eq!(explained.report.verdict, want);
    if want == Verdict::Unsupported {
        assert!(
            checked.diagnostics.iter().any(|d| d.code == "U-SEM-0"
                && d.message.contains("evidence_basis_conditional_snapshot"))
        );
    }
    if want == Verdict::Violated {
        let proof = explained
            .explanation
            .counterexamples
            .iter()
            .find(|c| c.id == "contract Basis #1 evidence_basis")
            .unwrap();
        assert_eq!(proof.replay.status, explain::ReplayStatus::Verified);
        assert!(!proof.evidence.as_ref().unwrap().missing.is_empty());
    } else {
        assert!(explained.explanation.counterexamples.is_empty());
    }
}

#[test]
fn r1_fork_writer_is_a_candidate_even_through_the_record() {
    // The single fork writer always finishes here, but the analysis does not
    // treat fork writers as dominating. The record cannot prove more.
    let src = model(FORK, REGISTER);
    check(&src, Verdict::Unsupported);
    let without_record = model(FORK, "")
        .replace("use() -> V { reads [Notes];", "use(d: V) -> V {")
        .replace("u = Boss.use()", "u = Boss.use(d)");
    check(&without_record, Verdict::Unsupported);
    check(
        &src.replace("evidence_basis Boss", "evidence_basis direct Boss"),
        Verdict::Violated,
    );
}

#[test]
fn r2_an_unconditional_registration_inherits_the_optional_overwrite() {
    // Some histories use intake's Doc, others use the blind patch. In both,
    // register runs; its latest observed input is the uncertain part.
    check(&model(CONDITIONAL, REGISTER), Verdict::Unsupported);
}

#[test]
fn r3_r4_r5_r6_missing_certain_and_optional_are_distinct() {
    check(
        &model(CONDITIONAL, REGISTER).replace("reads [Doc];", ""),
        Verdict::Violated,
    );
    check(
        &model(CONDITIONAL, REGISTER).replace(
            "patch() -> V { writes",
            "patch() -> V { reads [Doc]; writes",
        ),
        Verdict::Satisfied,
    );
    check(
        &model(
            "i = Clerk.intake(); g = Clerk.review();",
            "if g == pass { s = Boss.register(d) } else {};",
        ),
        Verdict::Unsupported,
    );
    let adr_r0 = include_str!("record_provenance/r0.relca");
    assert_eq!(
        adr_r0.matches("reads [Know]; registers [Notes];").count(),
        1
    );
    let broken = adr_r0.replace("reads [Know]; registers [Notes];", "registers [Notes];");
    check(&broken, Verdict::Violated);
}

#[test]
fn r7_uncertain_doc_does_not_erase_a_certain_policy_source() {
    let src = model(CONDITIONAL, REGISTER).replace(
        "{ registers [Notes];",
        "{ reads [Policy]; registers [Notes];",
    );
    check(
        &src.replace("Boss.use [Doc]", "Boss.use [Policy,Doc]"),
        Verdict::Unsupported,
    );
    check(
        &src.replace("Boss.use [Doc]", "Boss.use [Policy]"),
        Verdict::Satisfied,
    );
    let model = lower::lower(&syntax::parse(&src).unwrap()).unwrap();
    let ir::ContractKind::EvidenceBasis { calls, .. } = &model.contracts[0].kind else {
        panic!()
    };
    let proof = calls
        .iter()
        .find(|c| model.process.bindings[c.binding.0].name == "u")
        .unwrap();
    let names = |ids: &[ir::ActorId]| {
        ids.iter()
            .map(|id| model.actors[id.0].name.as_str())
            .collect::<Vec<_>>()
    };
    assert_eq!(names(&proof.sources), ["Policy"]);
    assert_eq!(names(&proof.conditional_sources), ["Doc"]);

    // The known missing source still makes this a violation; the witness must
    // not turn its unknown Doc into either a fact or a second violation.
    let mixed = src.replace("Boss.use [Doc]", "Boss.use [Policy,Doc,Missing]");
    check(&mixed, Verdict::Violated);
    let explained = explain::explain(&mixed);
    let evidence = explained
        .explanation
        .counterexamples
        .iter()
        .find_map(|c| c.evidence.as_ref())
        .unwrap();
    assert_eq!(evidence.sources, ["Policy"]);
    assert_eq!(evidence.conditional_sources, ["Doc"]);
    assert_eq!(evidence.missing, ["Missing"]);
}

#[test]
fn r8_a_second_registration_keeps_the_candidate_without_promoting_it() {
    let src = model(CONDITIONAL, "s = Boss.register(d); copied = Boss.copy();")
        .replace("use() -> V { reads [Notes];", "use() -> V { reads [Copy];");
    check(&src, Verdict::Unsupported);
    check(&src.replace("reads [Doc];", ""), Verdict::Violated);
}

#[test]
fn optional_registration_and_optional_inputs_use_the_same_evidence_field() {
    let early = "i = Clerk.intake();";
    let register =
        "h = Clerk.review(); if h == pass { s = Boss.register(d) } else { q = Clerk.patch() };";
    for before in [early, CONDITIONAL] {
        // x02c / x07: Extra never reaches use. Doc is a candidate whether
        // only registration is optional or both the input and registration are.
        let src = model(before, register).replace("Boss.use [Doc]", "Boss.use [Doc,Missing]");
        check(&src, Verdict::Violated);
        let out = explain::explain(&src);
        let evidence = out
            .explanation
            .counterexamples
            .iter()
            .find_map(|c| c.evidence.as_ref())
            .unwrap();
        assert!(evidence.sources.is_empty());
        assert_eq!(evidence.conditional_sources, ["Doc"]);
        assert_eq!(evidence.missing, ["Missing"]);

        // An unrequested candidate stays out of the requested-source list.
        let out = explain::explain(&src.replace("Boss.use [Doc,Missing]", "Boss.use [Missing]"));
        let evidence = out
            .explanation
            .counterexamples
            .iter()
            .find_map(|c| c.evidence.as_ref())
            .unwrap();
        assert!(evidence.conditional_sources.is_empty());
        assert_eq!(evidence.missing, ["Missing"]);
    }
}

#[test]
fn r7_candidates_do_not_become_confirmed_or_independent_sources() {
    use serde_json::json;
    let src = model(CONDITIONAL, REGISTER)
        .replace("{ registers [Notes];", "{ reads [Policy]; registers [Notes];")
        .replace("cut Scope { include [Doc,", "actor Ledger { capacity 1; capability post(u: V) -> V { commits; duration fixed 1min; result nondet { pass } } }\ncut Scope { include [Ledger,Doc,")
        .replace("u = Boss.use(); end done", "u = Boss.use(); if u == pass { out = Ledger.post(u); end done } else { end refused }")
        .replace("Boss.use [Doc]", "Boss.use [Policy]");
    let report = serde_json::to_value(report::check(&src)).unwrap();
    assert_eq!(report["verdict"], "satisfied");
    let agency = &report["agency"];
    let boss = agency["origin"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["actor"] == "Boss")
        .unwrap();
    assert_eq!(
        boss["via_records"],
        json!([{
            "record": "Notes", "registrars": [{"call": "s", "people": []}],
            "sources": [{"source": "Policy", "route": "registrar_direct"}]
        }])
    );
    // The informational subject view has always kept possible antecedents.
    // Neither the record nor candidate Doc adds an independent source.
    assert_eq!(
        agency["lineage"],
        json!([
            {"call": "d", "capability": "Clerk.draft", "subject_sources": ["Doc"], "independent_sources": []},
            {"call": "s", "capability": "Boss.register", "subject_sources": ["Doc"], "independent_sources": ["Policy"]},
            {"call": "u", "capability": "Boss.use", "subject_sources": ["Doc", "Policy"], "independent_sources": []},
        ])
    );
    assert_eq!(
        agency["lineage_groups"],
        json!([
            {"subject_sources": ["Doc"], "judgements": 2, "actors": 2, "independent_sources": ["Policy"]},
            {"subject_sources": ["Doc", "Policy"], "judgements": 1, "actors": 1, "independent_sources": []},
        ])
    );
}

#[test]
fn records_can_supply_distinct_definite_missing_and_uncertain_exit_proofs() {
    let base = BASE
        .replace("capability review()", "capability save() -> V { reads [Doc]; registers [Notes]; duration fixed 1min; result nondet { pass } }\ncapability blank() -> V { registers [Notes]; duration fixed 1min; result nondet { pass } }\ncapability saveb() -> V { reads [brief]; registers [Notes]; duration fixed 1min; result nondet { pass } }\ncapability review()")
        .replace("capability use()", "capability use(d: V)");
    let optional = "g = Clerk.review(); if g == pass { i = Clerk.intake() } else {}";
    let limit = "g = Clerk.review(); if g == pass { i = Clerk.intake() } else { p = Clerk.patch() }; r = Clerk.saveb();";
    let cases = [
        // P6b: the first leaf can only see the blind write. The second
        // leaf can see Doc conditionally. The definite absence wins.
        (format!("p0 = Clerk.patch(); repeat work at_most 1 {{ d = Clerk.draft(); c = Clerk.review(); if c == pass {{ r1 = Clerk.saveb(); leave }} else {{}}; {optional}; r2 = Clerk.saveb(); leave }} on_limit {{ end escalated }}; u = Boss.use(d); end done"),
         format!("p0 = Clerk.patch(); d = Clerk.draft(); c = Clerk.review(); if c == pass {{ r1 = Clerk.saveb(); u1 = Boss.use(d); end done }} else {{ {optional}; r2 = Clerk.saveb(); u2 = Boss.use(d); end done }}"),
         [(false, 'N'), (false, 'P')], Verdict::Violated),
        // P6c: reverse the definite absence and the candidate.
        (format!("p0 = Clerk.patch(); repeat work at_most 1 {{ d = Clerk.draft(); c = Clerk.review(); if c == pass {{ {optional}; r1 = Clerk.saveb(); leave }} else {{}}; r2 = Clerk.saveb(); leave }} on_limit {{ end escalated }}; u = Boss.use(d); end done"),
         format!("p0 = Clerk.patch(); d = Clerk.draft(); c = Clerk.review(); if c == pass {{ {optional}; r1 = Clerk.saveb(); u1 = Boss.use(d); end done }} else {{ r2 = Clerk.saveb(); u2 = Boss.use(d); end done }}"),
         [(false, 'P'), (false, 'N')], Verdict::Violated),
        // P7: the body saves Doc, but the limit's replacement is conditional.
        (format!("repeat work at_most 1 {{ d = Clerk.draft(); c = Clerk.review(); if c == pass {{ r1 = Clerk.save(); leave }} else {{ again }} }} on_limit {{ {limit} leave }}; u = Boss.use(d); end done"),
         format!("d = Clerk.draft(); c = Clerk.review(); if c == pass {{ r1 = Clerk.save(); u1 = Boss.use(d); end done }} else {{ {limit} u2 = Boss.use(d); end done }}"),
         [(false, 'C'), (true, 'P')], Verdict::Unsupported),
        // P7b: writing a blank record at the limit makes the absence certain.
        ("repeat work at_most 1 { d = Clerk.draft(); c = Clerk.review(); if c == pass { r1 = Clerk.save(); leave } else { again } } on_limit { r = Clerk.blank(); leave }; u = Boss.use(d); end done".into(),
         "d = Clerk.draft(); c = Clerk.review(); if c == pass { r1 = Clerk.save(); u1 = Boss.use(d); end done } else { r = Clerk.blank(); u2 = Boss.use(d); end done }".into(),
         [(false, 'C'), (true, 'N')], Verdict::Violated),
    ];
    for (looped, hand, states, expected) in cases {
        let src = base.replace("process P { BEFORE d = Clerk.draft(); REGISTER u = Boss.use(); end done }",
            &format!("process Repeated {{ {looped} }}\nprocess Written {{ {hand} }}\nconfiguration Loop {{ cut Scope; process Repeated }}\nconfiguration Hand {{ cut Scope; process Written }}"));
        let ast = syntax::parse(&src).unwrap();
        for spec in ["Hand", "Loop"] {
            let m = lower::lower_with_selection(&ast, Some(spec), None).unwrap();
            let r = reference::explore(&m);
            assert_eq!(r.stop, reference::StopReason::Complete);
            for out in [engine::explore(&m), r.outcome] {
                assert_eq!(
                    out.properties
                        .iter()
                        .find(|p| p.property == ir::Property::Contract(ir::ClauseId(0)))
                        .unwrap()
                        .verdict,
                    expected,
                    "{spec}: {states:?}"
                );
            }
            if spec == "Loop" {
                let ir::ContractKind::EvidenceBasis { calls, .. } = &m.contracts[0].kind else {
                    panic!()
                };
                let table = &calls
                    .iter()
                    .find(|c| m.process.bindings[c.binding.0].name == "u")
                    .unwrap()
                    .exit_scopes;
                let doc = ir::ActorId(m.actors.iter().position(|a| a.name == "Doc").unwrap());
                let actual: Vec<_> = table
                    .iter()
                    .map(|row| {
                        (
                            row.exits[0].on_limit,
                            if row.sources.contains(&doc) {
                                'C'
                            } else if row.conditional_sources.contains(&doc) {
                                'P'
                            } else {
                                assert!(!row.possible_sources.contains(&doc));
                                'N'
                            },
                        )
                    })
                    .collect();
                assert_eq!(actual, states);
            }
        }
    }
}
