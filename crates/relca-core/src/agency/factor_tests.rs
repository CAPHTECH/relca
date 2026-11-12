//! The pre-C1 enumerator and pre-C3 tree remain an independent test strategy.
use super::derivation::test_input as input;
use super::*;

fn same(source: &str) {
    same_selected(source, None);
}
fn same_selected(source: &str, selection: Option<&str>) {
    let m =
        crate::lower::lower_with_selection(&crate::syntax::parse(source).unwrap(), selection, None)
            .unwrap();
    let actual = derive(&m).unwrap();
    let reference = derive_strategy(
        &m,
        DEFAULT,
        [DEFAULT; 3],
        DEFAULT,
        degradation_for_model(&m).unwrap(),
        true,
    )
    .unwrap();
    assert_eq!(
        actual, reference,
        "entire Agency plus ordered diagnostics: {source}"
    );
}

fn sharing(kind: usize) -> String {
    let gate = "actor Gate { capacity 1; state permit: V = pass; capability check(x:V) -> V { requires permit == pass else fail; duration fixed 1min; result nondet { pass } } }";
    let (body, extra) = match kind {
        0 => (
            "s=Intake.start(); j1=Judge.check(s); j2=Judge.check(s); if j1==pass {} else {end rejected}; if j1==pass || j2==pass {} else {end rejected}; p=Ledger.post(j2); end done",
            "",
        ),
        1 => (
            "s=Intake.start(); j0=Judge.check(s); g=Gate.check(s); j2=Judge.check(s); if g==pass {} else {end rejected}; if j2==pass {} else {end rejected}; p=Ledger.post(j0); end done",
            gate,
        ),
        2 => (
            "s=Intake.start(); j0=Judge.check(s); j1=Judge.check(s); g=Gate.check(s); j2=Judge.check(s); if j1==pass {} else {end rejected}; if j2==pass {} else {end rejected}; p=Ledger.post(j0); end done",
            gate,
        ),
        3 => (
            "s=Intake.start(); j0=Judge.check(s); j1=Judge.check(s); j2=Judge.check(s); j3=Judge.check(s); j4=Judge.check(s); if j1==pass || j1==fail {} else {end rejected}; if j2==pass || j2==fail {} else {end rejected}; if j3==pass || j3==fail {} else {end rejected}; if j4==pass {} else {end rejected}; if j1==pass || j2==pass {} else {end rejected}; if j2==pass || j3==pass {} else {end rejected}; p=Ledger.post(j0); end done",
            "",
        ),
        _ => (
            "s=Intake.start(); j1=Judge.check(s); j2=Judge.check(s); if j1==pass {} else {end rejected}; if j1==fail {} else {end rejected}; if j2==pass {} else {end rejected}; p=Ledger.post(j2); end done",
            "",
        ),
    };
    let mut s = input::model(body, extra, 92);
    if kind == 1 || kind == 2 {
        s = s.replace(
            "[Doc,Intake,Judge,Ledger]",
            "[Doc,Intake,Judge,Ledger,Gate]",
        );
    }
    if kind == 2 {
        s = s
            .replace("actor Doc", "observed case { gate: V = fail }\n actor Doc")
            .replace(
                "capability check(x:V) -> V { duration",
                "capability check(x:V) -> V { writes gate; duration",
            )
            .replace(
                "requires permit == pass",
                "requires gate == pass || gate == fail",
            );
    }
    if kind == 1 {
        s = s.replace(
            gate,
            &gate.replace("result nondet { pass }", "result nondet { pass, fail }"),
        );
        s = s
            .replace(
                "actor Doc",
                "observed case { permit_case: V = pass }\n actor Doc",
            )
            .replace("requires permit == pass", "requires permit_case == pass");
    }
    if kind == 3 {
        s = s
            .replace("enum { pass, fail }", "enum { pass, fail, other }")
            .replace("nondet { pass, fail }", "nondet { pass, fail, other }");
    }
    s
}

macro_rules! sharing_test {
    ($name:ident,$kind:expr) => {
        #[test]
        fn $name() {
            same(&sharing($kind));
        }
    };
}
sharing_test!(merge_if_and_if_share_a_binding, 0);
sharing_test!(merge_requires_domain_and_if_share_a_binding, 1);
sharing_test!(merge_result_provenance_requires_and_if_share_a_binding, 2);
sharing_test!(merge_three_components_are_transitively_joined, 3);
sharing_test!(merge_contradictory_shared_binding_is_unreachable, 4);

fn producers(k: usize, candidate: bool, guarded: bool) -> String {
    let calls = (0..k)
        .map(|i| format!("v{i}=P.value();"))
        .collect::<String>();
    let guard = if guarded {
        format!(
            "if t==pass{{}}else{{end rejected}}; if {}{{}}else{{end rejected}};",
            (0..k)
                .map(|i| format!("v{i}==pass"))
                .collect::<Vec<_>>()
                .join(" || ")
        )
    } else {
        String::new()
    };
    let judge = if candidate { "j=J.check(t);" } else { "" };
    let effect = if candidate {
        "p=E.post(j);"
    } else {
        "p=E.post();"
    };
    let parameter = if candidate { "x:V" } else { "" };
    let result = if guarded { "pass,fail" } else { "pass" };
    format!(
        "model Producers version \"0.2\" {{ type V=enum{{pass,fail}} actor P{{capacity 1; capability value()->V{{duration fixed 1min; result nondet{{{result}}}}}}} actor J{{capacity 1;capability check(x:V)->V{{duration fixed 1min;result nondet{{pass}}}}}} actor E{{capacity 1;capability post({parameter})->V{{commits;duration fixed 1min;result nondet{{pass}}}}}} process Flow{{t=P.value();{judge}{calls}{guard}{effect}end done}} cut C{{include[P,J,E]}} bounds{{cases 1;live_tasks 2;clock 64min}} verify safety no_resource_overcommit }}"
    )
}

#[test]
fn r1_candidate_free_reachability_keeps_the_old_short_circuit() {
    same(&producers(20, false, true));
}
#[test]
fn r1_shallow_argument_free_graph_does_not_reserve_a_dense_transitive_matrix() {
    same(&producers(9000, false, false));
}
#[test]
fn r1_repeated_connected_guards_do_not_copy_an_ever_growing_component() {
    let body = format!(
        "s=Intake.start(); j1=Judge.check(s); j2=Judge.check(s); {} p=Ledger.post(j1); end done",
        "if j1==pass || j2==pass {} else {end rejected};".repeat(1200)
    );
    same(&input::model(&body, "", 92));
}
#[test]
fn r1_small_default_budget_matrix_does_not_regress_any_reference_success() {
    for k in 2..=13 {
        for candidate in [false, true] {
            let s = producers(k, candidate, true);
            let m = crate::lower::lower(&crate::syntax::parse(&s).unwrap()).unwrap();
            if let Ok(old) = derive_strategy(
                &m,
                DEFAULT,
                [DEFAULT; 3],
                DEFAULT,
                degradation_for_model(&m).unwrap(),
                true,
            ) {
                assert_eq!(derive(&m).unwrap(), old, "k={k} candidate={candidate}");
            }
        }
    }
}

fn two_admissible_values_per_independent_gate(k: usize) -> String {
    let mut s = input::gate(k)
        .replace("enum { pass, fail }", "enum { pass, fail, other }")
        .replace("nondet { pass, fail }", "nondet { pass, fail, other }")
        .replace(
            "s = Intake.start()",
            "s = Intake.start(); j0 = Judge.check(s)",
        )
        .replace(&format!("Ledger.post(j{k})"), "Ledger.post(j0)");
    for i in 1..=k {
        s = s.replace(
            &format!("if j{i} == pass"),
            &format!("if j{i} == pass || j{i} == fail"),
        );
    }
    s
}

#[test]
fn product_counts_are_exact_without_materializing_a_million_contexts() {
    same(&two_admissible_values_per_independent_gate(6));
    let source = two_admissible_values_per_independent_gate(20);
    let m = crate::lower::lower(&crate::syntax::parse(&source).unwrap()).unwrap();
    let (a, _) = derive(&m).unwrap();
    let rows: Vec<_> = a.refusal[0]
        .judgements
        .iter()
        .filter_map(Judgement::process)
        .collect();
    assert_eq!(rows.len(), 21);
    assert_eq!(rows[0].call, "j0");
    assert_eq!(rows[0].stops, Stops::None);
    assert_eq!(rows[0].bypasses, 1 << 20);
    assert!(
        rows[1..]
            .iter()
            .all(|r| r.stops == Stops::AllPaths && r.bypasses == 0)
    );
}

#[test]
fn unrepresentable_product_count_is_unavailable_not_wrapped_or_partial() {
    let s = two_admissible_values_per_independent_gate(70);
    let m = crate::lower::lower(&crate::syntax::parse(&s).unwrap()).unwrap();
    let limit = m
        .agency
        .limit()
        .expect("2^70 cannot be represented by usize");
    assert_eq!(limit.counter, "count_representation");
    assert!(limit.requested.is_none());
    assert!(limit.requested_overflow);
    assert_eq!(limit.judgement.as_deref(), Some("j0"));
    assert!(m.agency.as_derived().is_none());
    assert!(m.diagnostics.iter().any(|d| d.code == diag::W_AGY_4));
}

#[test]
fn r1_product_overflow_is_not_a_context_budget_claim() {
    let s = two_admissible_values_per_independent_gate(70);
    let m = crate::lower::lower(&crate::syntax::parse(&s).unwrap()).unwrap();
    let l = serde_json::to_value(m.agency.limit().unwrap()).unwrap();
    assert_eq!(l["counter"], "count_representation");
    assert!(l["requested"].is_null());
    assert_eq!(l["requested_overflow"], true);
}

#[test]
fn r1_budget_add_overflow_does_not_report_a_fabricated_request() {
    let mut b = viewpoints::budget(Limits {
        work: usize::MAX,
        ..DEFAULT
    });
    b.work = usize::MAX;
    let Error::Limit(l) = b.work().unwrap_err() else {
        panic!()
    };
    let v = serde_json::to_value(&l).unwrap();
    assert!(v["requested"].is_null());
    assert_eq!(v["requested_overflow"], true);
    assert!(l.to_string().contains("requested unrepresentable"));
}

#[test]
fn component_relations_are_sets_even_when_public_ir_repeats_a_domain_value() {
    let s = two_admissible_values_per_independent_gate(3);
    let mut m = crate::lower::lower(&crate::syntax::parse(&s).unwrap()).unwrap();
    let judge = m
        .capabilities
        .iter_mut()
        .find(|c| c.name == "check")
        .unwrap();
    judge.results.push(judge.results[0]);
    let actual = derive(&m).unwrap();
    let reference = derive_strategy(
        &m,
        DEFAULT,
        [DEFAULT; 3],
        DEFAULT,
        degradation_for_model(&m).unwrap(),
        true,
    )
    .unwrap();
    assert_eq!(actual, reference);
    assert_eq!(
        actual.0.refusal[0].judgements[0]
            .process()
            .unwrap()
            .bypasses,
        8
    );
}

#[test]
fn component_and_bitset_strategy_preserves_every_output_of_the_reference() {
    for k in 1..=6 {
        same(&input::gate(k));
        same(
            &input::gate(k)
                .replace("enum { pass, fail }", "enum { pass, fail, other }")
                .replace("nondet { pass, fail }", "nondet { pass, fail, other }"),
        );
        if k >= 2 {
            for op in [" && ", " || "] {
                let s = input::joined(k, op);
                same(&s);
                same(&s.replace("{ } else { end rejected }", "{ end rejected } else { }"));
            }
        }
        if k <= 4 {
            same(&input::continuing(k));
            same(&input::requires(k));
        }
    }
    for k in [0, 1, 2, 62, 63, 64, 65, 126, 127, 128, 130] {
        same(&input::chain(k));
    }
    let mixed = input::joined(3, " || ").replace(
        "j1 == pass || j2 == pass || j3 == pass",
        "j1 == pass && (j2 == pass || j3 == pass)",
    );
    same(&mixed);
    same(&input::model(
        "s=Intake.start(); j1=Judge.check(s); if j1==pass { j2=Judge.check(s); if j2==pass {} else {end rejected} } else {}; p=Ledger.post(s); end done",
        "",
        92,
    ));
    for s in [
        include_str!("../../../../fixtures/agency-origin/model.relca"),
        include_str!("../../../../fixtures/agency-lineage/model.relca"),
        include_str!("../../../../fixtures/agy2-reads-observed/model.relca"),
        include_str!("../../../../fixtures/lin-basis/model.relca"),
    ] {
        same(s);
    }
    for selected in ["AsIs", "ToBe"] {
        same_selected(
            include_str!("../../../../fixtures/agency-refusal/model.relca"),
            Some(selected),
        );
    }
}

#[test]
fn bitset_words_have_the_same_membership_as_the_tree_in_nonlexical_dag_order() {
    let mut b = viewpoints::budget(DEFAULT);
    let mut sets = (0..193)
        .map(|_| Ancestors::new(193, false, &mut b).unwrap())
        .collect::<Vec<_>>();
    for (to, from) in [
        (128, 0),
        (64, 128),
        (192, 64),
        (192, 63),
        (192, 65),
        (192, 127),
        (192, 129),
    ] {
        Ancestors::inherit(&mut sets, to, ir::BindingId(from), &mut b).unwrap();
    }
    let expected = [0, 63, 64, 65, 127, 128, 129].map(ir::BindingId);
    assert_eq!(sets[192].iter().collect::<Vec<_>>(), expected);
    for id in 0..193 {
        assert_eq!(
            sets[192].contains(&ir::BindingId(id)),
            expected.contains(&ir::BindingId(id))
        );
    }
}
