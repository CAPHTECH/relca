//! ADR0038 Stage C: assertions are independent hand tables, not interpreter votes.
#[path = "support/record_acceptance.rs"]
mod hand;
#[path = "support/record_shape_probes.rs"]
mod record_shape_probes;
use relca_core::{engine, ir::*, reference};
use serde_json::{Value, json};

include!("support/record_fixture_cases.rs");

#[test]
fn closed_population_pairs_and_no_dsl_comments() {
    let mut actual: Vec<_> = std::fs::read_dir(hand::root().join("fixtures"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("rec-"))
        .collect();
    actual.sort();
    assert_eq!(actual, hand::names());
    let pairs: Value = serde_json::from_str(include_str!("support/record_pairs.json")).unwrap();
    assert_eq!(pairs.as_array().unwrap().len() * 2, actual.len());
    for p in pairs.as_array().unwrap() {
        let a = hand::source(p["base"].as_str().unwrap());
        let b = hand::source(p["broken"].as_str().unwrap());
        assert_ne!(a, b, "no duplicate-input fault pairs");
        for s in [a, b] {
            assert!(!s.contains("//") && !s.contains('#'));
        }
    }
}

#[test]
fn all91_legacy_skeletons_are_preflighted_without_new_syntax_or_hidden_errors() {
    let cases: Value =
        serde_json::from_str(include_str!("support/record_legacy_skeletons.json")).unwrap();
    assert_eq!(cases.as_object().unwrap().len(), 91);
    for (name, s) in cases.as_object().unwrap() {
        let s = s.as_str().unwrap();
        let m = hand::model(s);
        hand::report_oracle_for_source(
            name,
            s,
            &serde_json::to_value(relca_core::report::check(s)).unwrap(),
        );
        assert_eq!(
            reference::explore(&m).stop,
            reference::StopReason::Complete,
            "{name}: old scaffold"
        );
    }
}

fn first_if(stmts: &[Statement]) -> &Cond {
    stmts
        .iter()
        .find_map(|s| match &s.kind {
            Stmt::If { condition, .. } => Some(condition),
            _ => None,
        })
        .expect("field guard")
}
fn result_ids(c: &Cond) -> Vec<usize> {
    match c {
        Cond::Eq(m) => vec![m.value.0],
        Cond::Or(xs) => xs.iter().flat_map(result_ids).collect(),
        other => panic!("binding projection must be flat Eq/Or, not {other:?}"),
    }
}
fn pair_type(m: &Model) -> usize {
    m.types.iter().position(|t| t.name == "Pair").unwrap()
}

#[test]
fn t15_mixed_radix_3_by_2_and_projection_are_fixed_independently() {
    let m = hand::model(&hand::source("rec-product"));
    let ty = &m.types[pair_type(&m)];
    assert_eq!(
        ty.members,
        [
            "Pair { color = red, flag = true }",
            "Pair { color = red, flag = false }",
            "Pair { color = green, flag = true }",
            "Pair { color = green, flag = false }",
            "Pair { color = blue, flag = true }",
            "Pair { color = blue, flag = false }",
        ]
    );
    assert_eq!(result_ids(first_if(&m.process.body)), vec![1, 3, 5]);
    let s = hand::changed(
        &hand::source("rec-product"),
        "p.flag == false",
        "p.color == green",
    );
    assert_eq!(
        result_ids(first_if(&hand::model(&s).process.body)),
        vec![2, 3]
    );
    let source = hand::source("rec-product");
    let enum_decl = "type Color = enum { red, green, blue }";
    let forward = hand::changed(&source, enum_decl, "");
    let forward = hand::changed(
        &forward,
        "verify safety",
        &format!("{enum_decl}\nverify safety"),
    );
    let forward = hand::model(&forward);
    assert_eq!(
        forward.types[pair_type(&forward)].members,
        ty.members,
        "forward named field type has the same independent table"
    );
}

#[test]
fn t15_all_six_record_values_keep_distinct_branch_states_in_both_backends() {
    let s = hand::source("rec-product");
    let mut branches = String::new();
    for color in ["red", "green"] {
        branches.push_str(&format!("if p.color == {color} {{ if p.flag == false {{ end {color}_false }} else {{ end {color}_true }} }} else {{ "));
    }
    branches.push_str("if p.flag == false { end blue_false } else { end blue_true } } }");
    let s = hand::changed(
        &s,
        "if p.flag == false { end off } else { end on }",
        &branches,
    );
    let queries = [
        "red_true",
        "red_false",
        "green_true",
        "green_false",
        "blue_true",
        "blue_false",
    ]
    .map(|n| format!("verify reachable end.{n}"))
    .join("\n");
    let s = hand::changed(
        &s,
        "verify reachable end.off\nverify reachable end.on",
        &queries,
    );
    let m = hand::model(&s);
    let a = engine::explore(&m);
    let r = reference::explore(&m);
    assert_eq!(r.stop, reference::StopReason::Complete);
    for o in [&a, &r.outcome] {
        assert_eq!(o.terminals.len(), 6);
        assert_eq!(o.properties.len(), 7);
        assert!(
            o.properties
                .iter()
                .all(|p| p.verdict == relca_core::Verdict::Satisfied)
        );
        for t in &o.terminals {
            assert_eq!(
                (t.elapsed.min, t.elapsed.max, t.work.min, t.work.max),
                (1, 1, 1, 1)
            );
        }
    }
}

#[test]
fn t15_reverse_asymmetric_2_by_3_radix_and_singleton_projection() {
    let s = hand::changed(
        &hand::source("rec-product"),
        "color: Color; flag: bool",
        "flag: bool; color: Color",
    );
    let m = hand::model(&s);
    assert_eq!(
        m.types[pair_type(&m)].members,
        [
            "Pair { flag = true, color = red }",
            "Pair { flag = true, color = green }",
            "Pair { flag = true, color = blue }",
            "Pair { flag = false, color = red }",
            "Pair { flag = false, color = green }",
            "Pair { flag = false, color = blue }",
        ]
    );
    assert_eq!(result_ids(first_if(&m.process.body)), vec![3, 4, 5]);
    let color = hand::changed(&s, "p.flag == false", "p.color == green");
    assert_eq!(
        result_ids(first_if(&hand::model(&color).process.body)),
        vec![1, 4]
    );
    let one = hand::model(&hand::source("rec-singleton"));
    assert!(
        matches!(first_if(&one.process.body), Cond::StateEq(_)),
        "single member still tests optional state, never true"
    );
}

#[test]
fn t15_deliberately_wrong_or_is_caught_by_hand_table_even_when_backends_agree() {
    let mut m = hand::model(&hand::source("rec-product.broken-one-value"));
    let guard = m
        .process
        .body
        .iter_mut()
        .find_map(|s| match &mut s.kind {
            Stmt::If { condition, .. } => Some(condition),
            _ => None,
        })
        .unwrap();
    assert_eq!(result_ids(guard), vec![1, 3, 5]);
    if let Cond::Or(xs) = guard {
        xs.remove(1);
    } else {
        panic!("multi-value field projection")
    }
    assert_eq!(result_ids(guard), vec![1, 5]);
    // U9: neither runtime evaluator knows a field was intended. They may agree
    // on the wrong legal Or; the fixed membership table is the independent oracle.
    let a = engine::explore(&m);
    let r = reference::explore(&m);
    assert_eq!(r.stop, reference::StopReason::Complete);
    for o in [&a, &r.outcome] {
        let ends = hand::terminal_map(o, &m);
        assert_eq!(
            ends.keys().map(String::as_str).collect::<Vec<_>>(),
            vec!["on"]
        );
        assert!(!ends.contains_key("off"));
    }
}

#[test]
fn t08_domain_1024_is_not_normal_result_count() {
    let m = hand::model(&hand::source("rec-size"));
    let r = m.types.iter().position(|t| t.name == "R").unwrap();
    assert_eq!(m.types[r].members.len(), 1024);
    assert_eq!(m.capabilities[0].results.len(), 1);
    for (n, verdict) in [(10, "satisfied"), (11, "unsupported"), (65, "unsupported")] {
        let fields = (0..n)
            .map(|i| format!("f{i}:bool"))
            .collect::<Vec<_>>()
            .join(";");
        let s = format!(
            "model Unused version \"0.2\" {{type R=record {{{fields}}} process P {{end done}} bounds {{cases 1;live_tasks 1;clock 1min}} verify safety no_resource_overcommit}}"
        );
        let v = serde_json::to_value(relca_core::report::check(&s)).unwrap();
        assert_eq!(v["verdict"], verdict, "unused domain2^{n}");
        if n > 10 {
            assert!(v["diagnostics"].as_array().unwrap().iter().any(|d| {
                d["code"] == "U-SEM-0"
                    && d["message"]
                        .as_str()
                        .unwrap()
                        .contains("record domain exceeds the supported size")
            }));
        }
    }
}

#[test]
fn t16_two_inline_positions_and_declared_choice_have_distinct_type_ids() {
    let s = r#"model Nominal version "0.2" {
      type Choice = enum { withdraw, dispute }
      actor Desk { capacity 1;
        capability first() -> enum { withdraw, dispute } { duration fixed 1min; result nondet { withdraw, dispute } }
        capability second() -> enum { withdraw, dispute } { duration fixed 1min; result nondet { withdraw, dispute } }
        capability declared() -> Choice { duration fixed 1min; result nondet { withdraw, dispute } }
      }
      process Flow { a=Desk.first(); b=Desk.second(); c=Desk.declared(); end done }
      bounds { cases 1; live_tasks 1; clock 4min }
      verify safety no_resource_overcommit
    }"#;
    let m = hand::model(s);
    let ids: Vec<_> = m.process.bindings.iter().map(|b| b.ty).collect();
    assert_eq!(ids.len(), 3);
    assert_ne!(ids[0], ids[1]);
    assert_ne!(ids[1], ids[2]);
    assert_ne!(ids[0], ids[2]);
    for id in ids {
        assert_eq!(m.types[id.0].members, ["withdraw", "dispute"]);
    }
}

#[test]
fn t24_public_ir_bad_members_names_domains_and_field_domains_fail_closed() {
    let good = hand::model(&hand::source("rec-product"));
    let id = pair_type(&good);
    let mut bad = Vec::new();
    let mut m = good.clone();
    m.types[id].members.pop();
    bad.push(("missing member", m));
    let mut m = good.clone();
    m.types[id].members.swap(0, 1);
    bad.push(("canonical order", m));
    let mut m = good.clone();
    m.types[id].members[1] = m.types[id].members[0].clone();
    bad.push(("duplicate member", m));
    let mut m = good.clone();
    m.types[id].name = "Wrong".into();
    bad.push(("canonical type spelling", m));
    let mut m = good.clone();
    m.types[id].members.resize(1025, "overflow".into());
    bad.push(("oversize", m));
    let color = good.types.iter().position(|t| t.name == "Color").unwrap();
    let mut m = good.clone();
    m.types[color].members.clear();
    bad.push(("empty field domain", m));
    let mut m = good.clone();
    m.types[color] = m.types[id].clone();
    bad.push(("nested record field", m));
    for (fault, m) in bad {
        for arrival in [false, true] {
            let mut m = m.clone();
            if arrival {
                m.arrivals = Some(vec![0]);
            }
            let a = engine::explore(&m);
            assert!(
                a.diagnostics.iter().any(|d| d.code == "U-SEM-0"),
                "{fault}, arrival={arrival}"
            );
            assert!(
                a.properties.is_empty() && a.terminals.is_empty(),
                "{fault}: no static results"
            );
            assert_eq!(
                reference::explore(&m).stop,
                reference::StopReason::SemanticAbort,
                "{fault}, arrival={arrival}: independent validation"
            );
        }
    }
    let source = hand::changed(
        &hand::source("rec-prob-field"),
        "verify safety",
        "simulation replications 3 seed 19 confidence 0.95\nverify safety",
    );
    let simulation = hand::model(&source);
    for arrival in [false, true] {
        let mut good = simulation.clone();
        if arrival {
            good.arrivals = Some(vec![0]);
        }
        assert_eq!(
            engine::simulate(&good).completed_replications,
            3,
            "positive simulation control, arrival={arrival}"
        );
        let id = pair_type(&good);
        good.types[id].members.swap(0, 1);
        let bad = engine::simulate(&good);
        assert_eq!(
            bad.completed_replications, 0,
            "malformed record shape must not sample"
        );
        assert!(bad.diagnostics.iter().any(|d| d.code == "U-SEM-0"));
    }
}

#[test]
fn t24_public_record_state_kind_and_initial_value_fail_closed_at_both_entries() {
    let good = hand::model(&hand::source("rec-prob-field"));
    let state = good
        .state_variables
        .iter()
        .position(|v| v.name == "pair")
        .unwrap();
    let mut bad = Vec::new();
    for kind in [
        StateKind::LatentCase,
        StateKind::ActorLatent(ActorId(0)),
        StateKind::ActorObserved(ActorId(0)),
    ] {
        let mut m = good.clone();
        m.state_variables[state].kind = kind;
        bad.push((format!("kind {kind:?}"), m));
    }
    let mut m = good.clone();
    m.state_variables[state].initial = InitialValue::Nondet;
    bad.push(("record initial nondet".into(), m));
    let mut m = good.clone();
    m.initial_distributions.push(vec![(
        StateValue::Value(ValueId(0)),
        relca_core::probability::Rational::one(),
    )]);
    m.state_variables[state].initial =
        InitialValue::Distribution(m.initial_distributions.len() - 1);
    bad.push(("record initial distribution".into(), m));
    let mut m = good.clone();
    m.state_variables[state].initial = InitialValue::Fixed(StateValue::Value(ValueId(usize::MAX)));
    bad.push(("record initial invalid ValueId".into(), m));
    let mut m = good.clone();
    m.state_variables[state].integer_max = Some(1);
    m.state_variables[state].initial = InitialValue::Fixed(StateValue::Integer(0));
    bad.push(("integer metadata on record".into(), m));
    let mut m = hand::model(&hand::source("rec-s18-given"));
    let entry = m
        .state_variables
        .iter()
        .position(|v| v.name == "entry")
        .unwrap();
    m.state_variables[entry].optional = false;
    m.state_variables[entry].initial = InitialValue::Fixed(StateValue::Value(ValueId(0)));
    let given = StateVarId(entry);
    m.kernels[0].given = vec![given];
    m.capabilities[0].kernel.as_mut().unwrap().given = vec![given];
    m.capabilities[0].kernel_reads_latent = vec![given];
    bad.push(("record kernel given".into(), m));
    for (fault, m) in bad {
        for arrival in [false, true] {
            let mut m = m.clone();
            if arrival {
                m.arrivals = Some(vec![0]);
            }
            let e = engine::explore(&m);
            assert!(
                e.diagnostics.iter().any(|d| d.code == "U-SEM-0"),
                "{fault}, arrival={arrival}"
            );
            assert!(e.properties.is_empty() && e.terminals.is_empty(), "{fault}");
            assert_eq!(
                reference::explore(&m).stop,
                reference::StopReason::SemanticAbort,
                "{fault}, arrival={arrival}"
            );
        }
    }
}

fn lineage_oracles(legacy: bool) {
    for (name, sources, subjects) in [
        (
            "rec-lineage",
            json!(["PurchaseOrder", "ReceiptRecord"]),
            json!(["Invoice", "PurchaseOrder", "ReceiptRecord"]),
        ),
        (
            "rec-lineage.broken-relay",
            json!(["PurchaseOrder"]),
            json!(["Invoice", "PurchaseOrder"]),
        ),
    ] {
        let source = if legacy {
            let cases: Value =
                serde_json::from_str(include_str!("support/record_legacy_skeletons.json")).unwrap();
            cases[name].as_str().unwrap().to_owned()
        } else {
            hand::source(name)
        };
        let v = serde_json::to_value(relca_core::report::check(&source)).unwrap();
        assert_eq!(v["verdict"], "satisfied");
        assert_eq!(
            v["agency"]["lineage_groups"],
            json!([
                {"subject_sources":["Invoice"],"judgements":2,"actors":1,"independent_sources":sources},
                {"subject_sources":subjects,"judgements":1,"actors":1,"independent_sources":[]}
            ])
        );
    }
}

#[test]
fn t05_record_parameters_keep_two_lineage_groups_and_receipt_removal_only() {
    lineage_oracles(false);
}

fn framing_oracles(legacy: bool) {
    for (name, binding) in [
        ("rec-agency-f", true),
        ("rec-agency-f.broken-case-guard", false),
    ] {
        let source = if legacy {
            let cases: Value =
                serde_json::from_str(include_str!("support/record_legacy_skeletons.json")).unwrap();
            cases[name].as_str().unwrap().to_owned()
        } else {
            hand::source(name)
        };
        let v = serde_json::to_value(relca_core::report::check(&source)).unwrap();
        assert_eq!(v["verdict"], "satisfied");
        let f = &v["agency"]["viewpoints"]["framing"];
        assert_eq!(f["complete"], binding);
        assert_eq!(
            f["actors"],
            if binding {
                json!(["Extractor"])
            } else {
                json!([])
            }
        );
        assert_eq!(f["ends"].as_array().unwrap().len(), 2);
        for row in f["ends"].as_array().unwrap() {
            if binding {
                assert_eq!(row["status"], "derived");
                assert_eq!(
                    row["selectors"],
                    json!([{"call":"e","capability":"Extractor.extract","selects":"all_contexts"}])
                );
            } else {
                assert_eq!(row["status"], "not_derived");
                assert_eq!(row["reason"], "state_guards");
            }
        }
    }
}

#[test]
fn t13_binding_selectors_are_derived_but_case_guards_remain_not_derived() {
    framing_oracles(false);
}

#[test]
fn lineage_and_framing_metadata_are_preflighted_on_legacy_scaffolds() {
    lineage_oracles(true);
    framing_oracles(true);
}

#[test]
fn record_named_enum_stays_legacy_accepted_and_forward_record_declarations_work() {
    let s = r#"model Legacy version "0.2" { type record=enum {yes,no}
      observed case { x:record=yes }
      actor A { capacity 1; capability seed()->record { duration fixed 1min; result nondet {yes} }
      capability use(p:record)->record { duration fixed 1min; result nondet {yes} } }
      process P { a=A.seed(); b=A.use(a); end done }
      bounds {cases 1;live_tasks 1;clock 4min} verify safety no_resource_overcommit }"#;
    assert_eq!(
        serde_json::to_value(relca_core::report::check(s)).unwrap()["verdict"],
        "satisfied"
    );
    let s = hand::source("rec-default");
    let declaration = "type Entry = record { confidence: enum { high, low } }";
    let moved = hand::changed(&s, declaration, "");
    let moved = hand::changed(
        &moved,
        "verify safety",
        &format!("{declaration}\nverify safety"),
    );
    hand::report_oracle_for_source(
        "rec-default",
        &moved,
        &serde_json::to_value(relca_core::report::check(&moved)).unwrap(),
    );
}

#[test]
fn static_literals_declarations_and_whole_comparison_forward_lookahead() {
    let original = hand::source("rec-s20-missing");
    assert_eq!(
        serde_json::to_value(relca_core::report::check(&original)).unwrap()["verdict"],
        "satisfied"
    );
    for (from, to, code, phrase) in [
        (
            "Entry { confidence = high }, Entry { confidence = low }",
            "Entry { confidence = high }, Entry { confidence = high }",
            "U-SEM-0",
            "duplicate",
        ),
        (
            "e.confidence == high",
            "e.confidence == none",
            "E-TYPE-0",
            "",
        ),
        (
            "Entry { confidence = high }, Entry { confidence = low }",
            "Missing { confidence = high }",
            "E-NAME-0",
            "undefined record type",
        ),
        (
            "confidence: enum { high, low }",
            "confidence: Missing",
            "E-NAME-0",
            "",
        ),
        (
            "record { confidence: enum { high, low } }",
            "record { confidence enum { high, low } }",
            "E-SYN-0",
            "",
        ),
        (
            "Entry { confidence = high }, Entry { confidence = low }",
            "Entry { confidence: high }",
            "E-SYN-0",
            "",
        ),
    ] {
        let s = hand::changed(&original, from, to);
        let v = serde_json::to_value(relca_core::report::check(&s)).unwrap();
        let errors: Vec<_> = v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|d| d["severity"] == "error")
            .collect();
        assert_eq!(errors.len(), 1, "{from}: {v}");
        assert_eq!(errors[0]["code"], code);
        assert!(errors[0]["message"].as_str().unwrap().contains(phrase));
        assert_eq!(v["properties"], json!([]));
        assert_eq!(v["terminals"], json!([]));
    }
    let source = hand::source("rec-s11-whole-compare.broken-unsupported");
    let declaration = "type Entry = record { confidence: enum { high, low } }";
    let s = hand::changed(&source, declaration, "");
    let s = hand::changed(
        &s,
        "verify safety",
        &format!("{declaration}\nverify safety"),
    );
    let s = hand::changed(&s, "e == Entry {", "e ==\nEntry\n{");
    let ds = relca_core::syntax::parse(&s).unwrap_err();
    assert_eq!(ds.len(), 1);
    assert_eq!(ds[0].code, "U-SYN-0");
    assert!(ds[0].message.contains("record comparison is unsupported"));
    for name in [
        "rec-s01-latent-case.broken-unsupported",
        "rec-s26-distribution.broken-distribution",
    ] {
        let s = hand::source(name);
        let moved = hand::changed(&s, declaration, "");
        let moved = hand::changed(
            &moved,
            "verify safety",
            &format!("{declaration}\nverify safety"),
        );
        hand::report_oracle_for_source(
            name,
            &moved,
            &serde_json::to_value(relca_core::report::check(&moved)).unwrap(),
        );
    }
}

#[test]
fn record_value_order_is_canonical_and_duplicate_normal_results_are_not_distinct() {
    let s = r#"model Values version "0.2" {
      type Pair=record { color:enum {red,green,blue}; flag:bool }
      observed case {pair:Pair=Pair {flag=false,color=green}}
      actor Work {capacity 1;capability run()->Pair {duration fixed 1min; result nondet {Pair {flag=true,color=red},Pair {color=green,flag=false}};writes pair}}
      process P {p=Work.run();end done}
      bounds {cases 1;live_tasks 1;clock 2min} verify safety no_resource_overcommit
    }"#;
    let m = hand::model(s);
    let id = pair_type(&m);
    assert_eq!(
        m.types[id].members[3],
        "Pair { color = green, flag = false }"
    );
    let duplicate = hand::changed(
        s,
        "Pair {flag=true,color=red},Pair {color=green,flag=false}",
        "Pair {flag=false,color=green},Pair {color=green,flag=false}",
    );
    let v = serde_json::to_value(relca_core::report::check(&duplicate)).unwrap();
    assert_eq!(v["verdict"], "unsupported");
    assert_eq!(
        v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|d| d["severity"] == "error")
            .map(|d| d["code"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["U-SEM-0"]
    );
}
