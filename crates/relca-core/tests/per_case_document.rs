//! ADR 0030: each runtime against hand arithmetic before differential equality.
use relca_core::{engine, ir, lower, reference, report, syntax};
use serde_json::json;
#[path = "support/percase_acceptance.rs"]
mod p;

#[test]
fn population_34_runtime29_static5_complete29_no_exclusions() {
    let mut names: Vec<_> = std::fs::read_dir(p::root())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("pcase-"))
        .collect();
    names.sort();
    assert_eq!(names, p::NAMES);
    let (mut runtime, mut rejected, mut complete) = (0, 0, 0);
    for name in p::NAMES {
        let s = p::source(name);
        assert!(!s.contains("//") && !s.contains('#'), "no DSL comments");
        let h = p::expected(name);
        if ["unsupported", "invalid"].contains(&h["verdict"].as_str().unwrap()) {
            rejected += 1;
            let ds = syntax::parse(&s)
                .and_then(|a| lower::lower(&a))
                .expect_err("static refusal");
            assert!(
                ds.iter().any(|d| d.code == h["diagnostics"][0]["code"]
                    && d.severity == relca_core::diag::Severity::Error),
                "{name}: {ds:?}"
            );
        } else {
            runtime += 1;
            let m = p::model(&s);
            let e = engine::explore(&m);
            let r = reference::explore(&m);
            assert_eq!(
                r.stop,
                reference::StopReason::Complete,
                "{name}: reference cannot silently exclude"
            );
            complete += 1;
            p::hand_outcome(name, &m, &e);
            p::hand_outcome(name, &m, &r.outcome);
            assert_eq!(e.properties, r.outcome.properties, "{name}");
            assert_eq!(e.case_outcomes, r.outcome.case_outcomes, "{name}");
            assert_eq!(e.admission, r.outcome.admission, "{name}");
            assert!(r.outcome.diagnostics.starts_with(&m.diagnostics));
            assert_eq!(
                e.diagnostics,
                r.outcome.diagnostics[m.diagnostics.len()..],
                "runtime diagnostics only; do not erase lower diagnostics"
            );
            assert_eq!(json!(r.verdict()), h["verdict"], "{name}");
        }
        p::subset(&h, &serde_json::to_value(report::check(&s)).unwrap());
    }
    assert_eq!((runtime, rejected, complete), (29, 5, 29));
}

#[test]
fn all_34_unmarked_single_controls_have_valid_existing_grammar() {
    for name in p::NAMES {
        p::model(&p::grammar_control(&p::source(name)));
    }
}

#[test]
fn seventeen_pairs_are_exactly_one_declared_transformation() {
    let pairs: Vec<[String; 4]> =
        serde_json::from_str(include_str!("support/percase_pairs.json")).unwrap();
    assert_eq!(pairs.len(), 17);
    for [name, fault, from, to] in pairs {
        let s = p::source(&format!("pcase-{name}"));
        assert_eq!(
            s.matches(&from).count(),
            if name == "write-ir" { 2 } else { 1 },
            "one rule change {name}"
        );
        assert_eq!(
            s.replace(&from, &to),
            p::source(&format!("pcase-{name}.broken-{fault}"))
        );
    }
}

#[test]
fn legacy_single_controls_validate_hand_oracles_before_new_syntax_exists() {
    for name in p::NAMES {
        let mut h = p::expected(name);
        if ["invalid", "unsupported"].contains(&h["verdict"].as_str().unwrap()) {
            continue;
        }
        let s = p::grammar_control(&p::source(name));
        let v = serde_json::to_value(report::check(&s)).unwrap();
        if let Some(props) = h
            .get_mut("properties")
            .and_then(serde_json::Value::as_array_mut)
        {
            for prop in props {
                if let Some(cases) = prop
                    .get_mut("cases")
                    .and_then(serde_json::Value::as_array_mut)
                {
                    cases.truncate(1);
                }
            }
        }
        // Future case2 was removed from the control, so its cut is not retained.
        if name == "pcase-late.broken-clock" {
            h["verdict"] = json!("satisfied");
            h["bounds"]["truncated"] = json!(false);
        }
        p::subset(&h, &v);
    }
}

#[test]
fn instance_words_remain_contextual_identifiers_in_existing_models() {
    let s = p::grammar_control(&p::source("pcase-marginal"));
    for word in ["instance", "per_case"] {
        for src in [
            s.replace("Reader", word),
            s.replace("r = Reader", &format!("{word} = Reader")),
        ] {
            let v = serde_json::to_value(report::check(&src)).unwrap();
            assert_eq!(v["verdict"], "satisfied", "{word}: {v}");
            assert_eq!(
                v["properties"][0]["cases"][0]["probability"],
                json!({"min":"23/500","max":"23/500"})
            );
        }
    }
}

#[test]
fn f2_single_case_quality_matches_each_arrival_without_interference() {
    for name in ["pcase-marginal", "pcase-marginal.broken-error-row"] {
        let s = p::source(name)
            .replace("cases 2", "cases 1")
            .replace("[0min, 10min]", "[0min]");
        let m = p::model(&s);
        let fraction = if name.contains("broken") {
            "13/500"
        } else {
            "23/500"
        };
        let r = reference::explore(&m);
        assert_eq!(r.stop, reference::StopReason::Complete);
        for out in [&engine::explore(&m), &r.outcome] {
            p::probability(out, 0, 1, fraction, fraction);
        }
    }
}

#[test]
fn instance_syntax_matrix_and_forbidden_clause_spans() {
    let s = p::source("pcase-capacity");
    for (from, to, code, fragment) in [
        ("instance per_case; ", "instance ; ", "E-SYN-0", ";"),
        ("instance per_case", "instance shared", "U-SYN-0", "shared"),
        (
            "instance per_case",
            "instance per_process",
            "U-SYN-0",
            "per_process",
        ),
        (
            "instance per_case; ",
            "instance per_case; instance per_case; ",
            "U-SEM-0",
            "instance per_case",
        ),
        (
            "grain document",
            "capacity 1; grain document",
            "U-SEM-0",
            "capacity 1",
        ),
        (
            "grain document",
            "observed case { obs: bool = true }; grain document",
            "U-SYN-0",
            "observed",
        ),
    ] {
        let changed = s.replacen(from, to, 1);
        assert_ne!(s, changed, "syntax matrix replacement must take effect");
        let ds = syntax::parse(&changed)
            .and_then(|a| lower::lower(&a))
            .unwrap_err();
        let d = ds
            .iter()
            .find(|d| d.code == code)
            .unwrap_or_else(|| panic!("{code}: {ds:?}"));
        let span = d.span.unwrap();
        assert!(
            changed[span.start..span.end].contains(fragment),
            "{fragment}: {d:?}"
        );
        if to.contains("instance per_case; instance per_case") {
            assert!(
                span.start
                    >= changed.find("instance per_case").unwrap() + "instance per_case; ".len()
            );
        }
    }
    for name in [
        "pcase-capacity.broken-capacity",
        "pcase-executors.broken-executors",
        "pcase-capability.broken-capability",
    ] {
        let s = p::source(name);
        for source in [
            s.clone(),
            s.replace("instance per_case; ", "")
                .replace("grain document", "instance per_case; grain document"),
        ] {
            let ds = syntax::parse(&source)
                .and_then(|a| lower::lower(&a))
                .unwrap_err();
            let d = ds.iter().find(|d| d.code == "U-SEM-0").unwrap();
            let span = d.span.unwrap();
            let fragment = if name.contains("executors") {
                "executors"
            } else if name.contains("capability") {
                "capability"
            } else {
                "capacity"
            };
            assert!(
                source[span.start..span.end].contains(fragment),
                "forbidden clause, not whole model: {d:?}"
            );
        }
    }
}

#[test]
fn optin_never_exempts_other_unmarked_actor_or_turns_labels_into_ownership() {
    let s = p::source("pcase-joint");
    for declaration in [
        "actor Shared { latent hidden: bool }",
        "actor Shared { latent hidden: bool ~ { true:0.5,false:0.5 } }",
        "kernel W { * -> { 1: ok with hidden = false } }; actor Shared { latent hidden: bool = true; capacity 1; capability mutate() -> T { writes hidden; duration fixed 1min; outcomes kernel W } }",
    ] {
        let mixed = s
            .replace(
                "  process Flow",
                &format!("  {declaration}\n  process Flow"),
            )
            .replace("include [Invoice]", "include [Invoice,Shared]");
        let ds = syntax::parse(&mixed)
            .and_then(|a| lower::lower(&a))
            .unwrap_err();
        assert!(
            ds.iter()
                .any(|d| d.code == "U-SEM-0" && d.message.contains("fixed and read-only")),
            "{ds:?}"
        );
    }
    for src in [
        s.replace("grain document", "grain ledger"),
        s.replace("source document", "source record"),
    ] {
        let m = p::model(&src);
        p::hand_outcome("pcase-joint", &m, &engine::explore(&m));
        let r = reference::explore(&m);
        assert_eq!(r.stop, reference::StopReason::Complete);
        p::hand_outcome("pcase-joint", &m, &r.outcome);
    }
    let plain = s.replace("instance per_case; ", "");
    let ds = syntax::parse(&plain)
        .and_then(|a| lower::lower(&a))
        .unwrap_err();
    assert!(ds.iter().any(|d| d.code == "U-SEM-0"
        && d.message == "multi-case Actor latent state must be fixed and read-only"));
}

#[test]
fn common_causes_and_assumptions_are_owned_not_independence_evidence() {
    for name in [
        "pcase-common",
        "pcase-common.broken-one-reader",
        "pcase-shared-fixed",
        "pcase-shared-fixed.broken-marker",
    ] {
        let s = p::source(name);
        let v = serde_json::to_value(report::check(&s)).unwrap();
        assert_eq!(v["verdict"], "satisfied");
        let both = name == "pcase-shared-fixed.broken-marker";
        p::assumptions(
            &v,
            if both {
                &["Invoice", "Shared"]
            } else {
                &["Invoice"]
            },
            name.starts_with("pcase-common"),
        );
        if name == "pcase-common.broken-one-reader" {
            assert!(v["agency"].get("common_causes").is_none());
            continue;
        }
        let causes = v["agency"]["common_causes"]["causes"].as_array().unwrap();
        assert_eq!(
            causes.len(),
            if name.starts_with("pcase-shared") {
                2
            } else {
                1
            }
        );
        for row in causes {
            assert_eq!(
                row["targets"].as_array().unwrap().len(),
                2,
                "lexical calls, not case expansion"
            );
            if row["actor"] == "Invoice" || both {
                assert_eq!(row["instance"], "per_case");
            } else {
                assert!(row.get("instance").is_none());
            }
        }
        assert_eq!(
            v["agency"]["unchecked"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|v| **v == "multi_case_interference")
                .count(),
            1
        );
    }
    let old = include_str!("../../../fixtures/lin-single.broken-second-call/model.relca");
    let marked = old.replace(
        "actor S { source document }",
        "actor S { instance per_case; source document }",
    );
    assert_ne!(
        old, marked,
        "the control must actually mark the source Actor"
    );
    let warning_calls = |s: &str| {
        let v = serde_json::to_value(report::check(s)).unwrap();
        v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|d| d["code"] == "W-LIN-2")
            .map(|d| d["message"].clone())
            .collect::<Vec<_>>()
    };
    assert!(!warning_calls(old).is_empty());
    assert_eq!(
        warning_calls(old),
        warning_calls(&marked),
        "instance cannot certify independent evidence"
    );
}

fn write_model(single: bool, arrivals: bool, broken: bool, repeat: bool) -> ir::Model {
    write_model_inner(single, arrivals, broken, repeat, true)
}
fn write_model_inner(
    single: bool,
    arrivals: bool,
    broken: bool,
    repeat: bool,
    marked: bool,
) -> ir::Model {
    let mut src = p::source("pcase-write-ir");
    if !marked {
        src = src.replace("instance per_case; ", "");
    }
    if single {
        src = src
            .replace("cases 2", "cases 1")
            .replace("[0min, 0min]", "[0min]");
    }
    if !arrivals {
        src = src
            .replace("  arrivals fixed [0min]\n", "")
            .replace("  policy fifo tie_by case_id\n", "");
    }
    if repeat {
        src = src.replace(
            "a = Reader.read(); end done",
            "a = Reader.read(); b = Reader.read(); end done",
        );
    }
    src=src.replace("verify probability reached scratch == true <= 1","verify probability reached Invoice.difficulty == difficult <= 1\n  verify probability reached scratch == true <= 1");
    let mut m = p::model(&src);
    let doc = ir::StateVarId(
        m.state_variables
            .iter()
            .position(|v| matches!(v.kind, ir::StateKind::ActorLatent(_)))
            .unwrap(),
    );
    let ty = m.state_variables[doc.0].base;
    let scratch = ir::StateVarId(
        m.state_variables
            .iter()
            .position(|v| v.name == "scratch")
            .unwrap(),
    );
    let bool_ty = m.state_variables[scratch.0].base;
    let bool_values = m.types[bool_ty.0].members.clone();
    let target = ir::StateValue::Value(ir::ValueId(
        m.types[ty.0]
            .members
            .iter()
            .position(|n| n == if broken { "routine" } else { "difficult" })
            .unwrap(),
    ));
    for cap in &mut m.capabilities {
        if let Some(kernel) = &mut cap.kernel {
            cap.kernel_writes = vec![doc];
            for row in &mut kernel.rows {
                let bad = row.values[0] == ir::StateValue::Value(ir::ValueId(1));
                let seen_bad = ir::StateValue::Value(ir::ValueId(
                    bool_values
                        .iter()
                        .position(|n| n == if bad { "true" } else { "false" })
                        .unwrap(),
                ));
                for branch in &mut row.branches {
                    branch.overrides = vec![(doc, target), (scratch, seen_bad)];
                }
            }
        }
    }
    m
}

#[test]
fn unmarked_single_public_ir_write_builder_control_is_valid() {
    for arrivals in [false, true] {
        for broken in [false, true] {
            for repeat in [false, true] {
                let m = write_model_inner(true, arrivals, broken, repeat, false);
                let e = engine::explore(&m);
                let r = reference::explore(&m);
                assert_eq!(
                    r.stop,
                    reference::StopReason::Complete,
                    "arrivals={arrivals} broken={broken} repeat={repeat}: {:?}",
                    r.outcome.diagnostics
                );
                for out in [&e, &r.outcome] {
                    assert!(
                        !out.diagnostics
                            .iter()
                            .any(|d| d.severity == relca_core::diag::Severity::Error),
                        "valid builder control: {:?}",
                        out.diagnostics
                    );
                    let written = if broken { "0" } else { "1" };
                    p::probability(out, 0, 1, written, written);
                    let bad = if repeat && !broken { "1" } else { "0" };
                    p::probability(out, 1, 1, bad, bad);
                }
            }
        }
    }
}

#[test]
fn f11_public_ir_external_write_stays_in_case_and_next_given_reads_updated_value() {
    for (single, arrivals) in [(true, false), (true, true), (false, true)] {
        for broken in [false, true] {
            for repeat in [false, true] {
                let m = write_model(single, arrivals, broken, repeat);
                let e = engine::explore(&m);
                let r = reference::explore(&m);
                assert_eq!(r.stop, reference::StopReason::Complete);
                for out in [&e, &r.outcome] {
                    assert!(
                        !out.diagnostics
                            .iter()
                            .any(|d| d.severity == relca_core::diag::Severity::Error),
                        "valid public IR must run: {:?}",
                        out.diagnostics
                    );
                    for c in 1..=if single { 1 } else { 2 } {
                        p::probability(
                            out,
                            0,
                            c,
                            if broken { "0" } else { "1" },
                            if broken { "0" } else { "1" },
                        );
                        let bad = if repeat && !broken { "1" } else { "0" };
                        p::probability(out, 1, c, bad, bad);
                    }
                }
                // The independent sampled audit exposes first/second results
                // without adding a cohort query or a qualified DSL write.
                let mut sample = m.clone();
                sample.simulation = Some(ir::SimulationSpec {
                    replications: 1,
                    seed: 7,
                    confidence: relca_core::probability::Rational::decimal("0.95").unwrap(),
                    span: sample.process.span,
                    replications_span: sample.process.span,
                    seed_span: sample.process.span,
                    confidence_span: sample.process.span,
                });
                let a = engine::simulate(&sample);
                let b = reference::simulate(&sample);
                assert_eq!(b.stop, reference::StopReason::Complete);
                assert_eq!(a.replications, b.outcome.replications);
                assert_eq!(e.properties, r.outcome.properties);
            }
        }
    }
}

#[test]
fn public_ir_entry_points_reject_resource_and_capability_forgery_without_panics() {
    let good = p::model(&p::source("pcase-capacity"));
    let doc = good
        .actors
        .iter()
        .position(|a| a.name == "Invoice")
        .unwrap();
    for capacity in [0, 1] {
        let mut m = good.clone();
        m.actors[doc].capacity = Some(capacity);
        m.actors[doc].metadata.capacity_declared = true;
        reject_ir(&m);
    }
    let mut m = good.clone();
    m.actors[doc].executors = Some(vec![ir::PersonId(0)]);
    reject_ir(&m);
    let mut m = write_model(false, true, false, false);
    m.capabilities[0].actor = ir::ActorId(doc);
    reject_ir(&m);
    let mut m = good.clone();
    m.state_variables[0].kind = ir::StateKind::ActorLatent(ir::ActorId(usize::MAX));
    reject_ir(&m);
    let mut m = write_model(false, true, false, false);
    m.capabilities[0].kernel_writes = vec![ir::StateVarId(usize::MAX)];
    reject_ir(&m);
}

#[test]
fn all_report_exits_keep_declaration_and_independence_assumptions() {
    let s = p::source("pcase-common");
    let simulated = s.replace(
        "  bounds",
        "  simulation replications 4 seed 7 confidence 0.95\n  bounds",
    );
    let check = serde_json::to_value(report::check(&s)).unwrap();
    let explain = serde_json::to_value(relca_core::explain::explain(&s)).unwrap();
    let simulate = serde_json::to_value(report::simulate(&simulated)).unwrap();
    for v in [&check, &explain, &simulate] {
        p::assumptions(v, &["Invoice"], true);
    }
    let mutation = serde_json::to_value(relca_core::mutation::mutate(&s)).unwrap();
    p::assumptions(&mutation["original"], &["Invoice"], true);
    for row in mutation["mutations"].as_array().unwrap() {
        if row["report"]["bounds"]["cases"].is_u64() {
            p::assumptions(&row["report"], &["Invoice"], true);
        }
    }
    let plans=simulated.replace("  bounds","  process Other { a = Reader.first(); b = Reader.second(); end done }\n  configuration A { cut Scope; process Flow }\n  configuration B { cut Scope; process Other }\n  bounds");
    let diff = serde_json::to_value(relca_core::diff::diff(&plans, "A", "B")).unwrap();
    for side in ["from", "to"] {
        p::assumptions(&diff[side]["report"], &["Invoice"], true);
        assert_eq!(
            diff[side]["report"]["agency"]["common_causes"]["causes"][0]["instance"],
            "per_case"
        );
    }
}
fn reject_ir(m: &ir::Model) {
    let e = engine::explore(m);
    let r = reference::explore(m);
    for out in [&e, &r.outcome] {
        assert!(
            out.diagnostics.iter().any(|d| d.code == "U-SEM-0"),
            "forged IR must fail closed: {:?}",
            out.diagnostics
        );
    }
}
