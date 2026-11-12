//! ADR 0012 seventh population: hand oracle first, interpreters second.
//! No state IR constructors or private monitor helpers build the expectations.
use relca_core::{
    Verdict, engine, ir, lower,
    outcome::Outcome,
    reference::{self, StopReason},
    report, syntax,
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs, path::Path};

const FIXTURES: [&str; 16] = [
    "state-agency",
    "state-agency.broken-state-guard",
    "state-cancel",
    "state-cancel.broken-ignore-cancel",
    "state-invoice-write",
    "state-invoice-write.broken-failed-approval",
    "state-latent-product",
    "state-latent-product.broken-partial-assertion",
    "state-name-alias",
    "state-name-alias.broken-missing-write",
    "state-observed-range",
    "state-observed-range.broken-fixed-route",
    "state-policy",
    "state-policy.broken-latent-difficulty",
    "state-writer-order",
    "state-writer-order.broken-parallel-writers",
];
const PRODUCT: &str = include_str!("../../../fixtures/state-latent-product/model.relca");
const INVOICE: &str = include_str!("../../../fixtures/state-invoice-write/model.relca");
const ORDER: &str = include_str!("../../../fixtures/state-writer-order/model.relca");
const CANCEL: &str = include_str!("../../../fixtures/state-cancel/model.relca");

fn terminals(model: &ir::Model, out: &Outcome) -> BTreeMap<String, Value> {
    let rows: BTreeMap<_,_> = out.terminals.iter().map(|t| {
        let name = model.ends[t.end.0].clone();
        (name.clone(), json!({"end":name,"elapsed":{"min":t.elapsed.min,"max":t.elapsed.max},"work":{"min":t.work.min,"max":t.work.max},"result_at":t.result_at.map(|r|json!({"min":r.min,"max":r.max}))}))
    }).collect();
    assert_eq!(rows.len(), out.terminals.len());
    rows
}

fn hand_outcome(label: &str, model: &ir::Model, actual: &Outcome, expected: &Value) {
    if let Some(truncated) = expected["bounds"]["truncated"].as_bool() {
        assert_eq!(actual.truncated, truncated, "{label}");
    }
    let hand: BTreeMap<_, _> = expected["terminals"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| (r["end"].as_str().unwrap().to_owned(), r.clone()))
        .collect();
    assert_eq!(hand.len(), expected["terminals"].as_array().unwrap().len());
    assert_eq!(terminals(model, actual), hand, "{label}: hand metrics");
    let properties = expected["properties"].as_array().unwrap();
    assert_eq!(
        actual.properties.len(),
        properties.len(),
        "{label}: inventory"
    );
    assert_eq!(model.verify.len(), properties.len());
    for ((out, verify), hand) in actual.properties.iter().zip(&model.verify).zip(properties) {
        assert_eq!(out.property, verify.property, "{label}");
        let verdict: Verdict = serde_json::from_value(hand["verdict"].clone()).unwrap();
        assert_eq!(out.verdict, verdict, "{label}: {}", hand["id"]);
    }
    for code in expected["absent_diagnostics"]
        .as_array()
        .into_iter()
        .flatten()
    {
        assert!(
            !actual
                .diagnostics
                .iter()
                .any(|d| d.code == code.as_str().unwrap()),
            "{label}: forbidden {code}"
        );
    }
}

fn runtime_diagnostics(out: &Outcome) -> Vec<Value> {
    // Backend explanatory messages differ by design; compare the complete
    // runtime code/severity/span inventory, not a selected list of codes.
    out.diagnostics
        .iter()
        .map(|d| json!({"code":d.code,"severity":d.severity,"span":d.span}))
        .collect()
}

fn compare_hand(label: &str, source: &str, selection: Option<&str>, expected: &Value) {
    let ast = syntax::parse(source).unwrap_or_else(|d| panic!("{label}: parse {d:?}"));
    let model = match selection {
        None => lower::lower(&ast),
        Some(s) => lower::lower_configuration(&ast, s).map(|s| s.model),
    }
    .unwrap_or_else(|d| panic!("{label}: lower {d:?}"));
    let reference = reference::explore(&model);
    let abort = expected["bounds"]["truncated"].is_null();
    assert_eq!(
        reference.stop,
        if abort {
            StopReason::SemanticAbort
        } else {
            StopReason::Complete
        },
        "{label}: no reference exclusion"
    );
    assert_eq!(
        reference.truncated(),
        expected["bounds"]["truncated"].as_bool(),
        "{label}"
    );
    let engine = engine::explore(&model);
    // Neither interpreter can be the other's oracle.
    hand_outcome(
        &format!("{label}/reference"),
        &model,
        &reference.outcome,
        expected,
    );
    hand_outcome(&format!("{label}/engine"), &model, &engine, expected);
    assert_eq!(
        serde_json::to_value(reference.verdict()).unwrap(),
        expected["verdict"],
        "{label}"
    );
    let public = serde_json::to_value(match selection {
        None => report::check(source),
        Some(s) => report::check_configuration(source, s),
    })
    .unwrap();
    assert_eq!(public["verdict"], expected["verdict"], "{label}");
    assert_eq!(
        public["properties"], expected["properties"],
        "{label}: public IDs"
    );
    assert_eq!(
        public["bounds"]["truncated"],
        expected["bounds"]["truncated"]
    );
    assert_eq!(
        engine.properties, reference.outcome.properties,
        "{label}: differential properties"
    );
    assert_eq!(
        terminals(&model, &engine),
        terminals(&model, &reference.outcome)
    );
    assert_eq!(engine.truncated, reference.outcome.truncated);
    assert!(
        reference
            .outcome
            .diagnostics
            .starts_with(&model.diagnostics)
    );
    let mut ref_runtime = reference.outcome.clone();
    ref_runtime.diagnostics.drain(..model.diagnostics.len());
    assert_eq!(
        runtime_diagnostics(&engine),
        runtime_diagnostics(&ref_runtime),
        "{label}: runtime diagnostics"
    );
}

#[test]
fn state_population_fixed_16_directories_16_inputs_13_accepted_3_rejected_zero_exclusions() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let mut dirs: Vec<_> = fs::read_dir(&root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| {
            p.is_dir()
                && p.file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .starts_with("state-")
        })
        .collect();
    dirs.sort();
    assert_eq!(
        dirs.iter()
            .map(|p| p.file_name().unwrap().to_str().unwrap())
            .collect::<Vec<_>>(),
        FIXTURES
    );
    let (mut inputs, mut accepted, mut rejected, mut compared) = (0, 0, 0, 0);
    let mut errors = Vec::new();
    let mut exclusions = Vec::new();
    for dir in dirs {
        let name = dir.file_name().unwrap().to_str().unwrap();
        for file in ["model.relca", "calc.md", "expected.json"] {
            assert!(dir.join(file).is_file());
        }
        let source = fs::read_to_string(dir.join("model.relca")).unwrap();
        let expected: Value =
            serde_json::from_str(&fs::read_to_string(dir.join("expected.json")).unwrap()).unwrap();
        inputs += 1;
        let parsed = syntax::parse(&source).and_then(|ast| lower::lower(&ast));
        let reject = match name {
            "state-policy.broken-latent-difficulty" => Some(("E-POL-LATENT", "invalid")),
            "state-name-alias.broken-missing-write"
            | "state-writer-order.broken-parallel-writers" => Some(("U-SEM-0", "unsupported")),
            _ => None,
        };
        if let Some((code, verdict)) = reject {
            rejected += 1;
            match parsed {
                Ok(_) => errors.push(format!("{name}: expected static rejection {code}")),
                Err(d) => {
                    if !d.iter().any(|d| d.code == code) {
                        errors.push(format!("{name}: wrong static diagnostic {d:?}"));
                    }
                }
            }
            let public = serde_json::to_value(report::check(&source)).unwrap();
            if public["verdict"] != verdict {
                errors.push(format!("{name}: wrong verdict {public}"));
            }
            assert_eq!(public["properties"], json!([]));
            assert_eq!(public["terminals"], json!([]));
            assert_eq!(public["agency"], Value::Null);
            for key in ["cases", "live_tasks", "clock", "truncated"] {
                assert_eq!(public["bounds"][key], Value::Null);
            }
            continue;
        }
        match parsed {
            Err(d) => errors.push(format!("{name}: unexpected static rejection {d:?}")),
            Ok(model) => {
                accepted += 1;
                let run = reference::explore(&model);
                if run.stop != StopReason::Complete {
                    exclusions.push(format!("{name}: {:?}", run.stop));
                }
                compare_hand(name, &source, None, &expected);
                compared += 1;
            }
        }
    }
    assert_eq!(inputs, 16);
    assert_eq!(rejected, 3);
    assert!(errors.is_empty(), "{}", errors.join("\n"));
    assert_eq!(accepted, 13);
    assert_eq!(compared, 13);
    assert!(
        exclusions.is_empty(),
        "zero reference exclusions: {}",
        exclusions.join("\n")
    );
}

#[test]
fn s_init_room_public_budget_zero_seed_one_and_legacy_empty_one() {
    let model = lower::lower(&syntax::parse(PRODUCT).unwrap()).unwrap();
    for budget in [0, 1, 4] {
        let out = engine::explore_with_budget(&model, budget);
        assert!(out.truncated);
        assert!(out.terminals.is_empty());
        assert!(
            out.properties
                .iter()
                .all(|p| p.verdict == Verdict::Inconclusive)
        );
        assert!(out.diagnostics.iter().any(|d| d.code == "W-BUDGET-0"));
    }
    let source = r#"model Empty version "0.2" {
      type One = enum { only }
      process P { end done }
      bounds { cases 1; live_tasks 2; clock 20min }
      verify safety no_resource_overcommit
    }"#;
    for source in [
        source.to_owned(),
        source.replace("process P", "latent case { truth: One = only }; process P"),
    ] {
        let model = lower::lower(&syntax::parse(&source).unwrap()).unwrap();
        let out = engine::explore_with_budget(&model, 1);
        assert!(
            !out.truncated,
            "empty/fixed state must not gain a seed transition"
        );
        assert_eq!(out.terminals.len(), 1);
        assert_eq!(out.properties[0].verdict, Verdict::Satisfied);
    }
    let nondet = source.replace("process P", "latent case { truth: One }; process P");
    let model = lower::lower(&syntax::parse(&nondet).unwrap()).unwrap();
    let out = engine::explore_with_budget(&model, 1);
    assert!(
        out.truncated,
        "a singleton nondet domain still requires a seed successor"
    );
    assert!(out.terminals.is_empty());
}

fn hand(kinds: &[(&str, &str)], verdict: &str, e: u64, w: u64, result: Option<u64>) -> Value {
    let mut properties = vec![json!({"id":"safety no_resource_overcommit","verdict":"satisfied"})];
    for (i, (kind, verdict)) in kinds.iter().enumerate() {
        properties
            .push(json!({"id":format!("contract Controls #{} {kind}",i+1),"verdict":verdict}));
    }
    json!({"verdict":verdict,"bounds":{"truncated":false},"properties":properties,
        "terminals":[{"end":"done","elapsed":{"min":e,"max":e},"work":{"min":w,"max":w},"result_at":result.map(|r|json!({"min":r,"max":r}))}],
        "absent_diagnostics":["E-POL-LATENT","E-CON-0","U-SEM-0","W-DEADLOCK-0","W-BUDGET-0"]})
}

macro_rules! exclude_tuple {
    ($name:ident, $truth:literal, $difficulty:literal) => {
        #[test] fn $name() {
            // The formula is false at exactly one of the four initial tuples.
            let source = PRODUCT.replace("(truth == true || truth == false) && (Invoice.difficulty == routine || Invoice.difficulty == difficult)", concat!("truth == ", $truth, " || Invoice.difficulty == ", $difficulty));
            compare_hand(stringify!($name), &source, None, &hand(&[("invariant","violated")], "violated", 1, 1, None));
        }
    };
}
exclude_tuple!(
    s_product_exclude_true_routine_both_interpreters,
    "false",
    "difficult"
);
exclude_tuple!(
    s_product_exclude_true_difficult_both_interpreters,
    "false",
    "routine"
);
exclude_tuple!(
    s_product_exclude_false_routine_both_interpreters,
    "true",
    "difficult"
);
exclude_tuple!(
    s_product_exclude_false_difficult_both_interpreters,
    "true",
    "routine"
);

#[test]
fn s_bundle_postwrite_and_prewrite_start_both_interpreters() {
    let source = INVOICE.replace("contract Controls {", "contract Controls { invariant Approver.approve.start -> approval == none; invariant Ledger.post.start -> registered == false; invariant Ledger.post.commit -> registered == true;");
    compare_hand(
        "postwrite bundle",
        &source,
        None,
        &hand(&[("invariant", "satisfied"); 5], "satisfied", 2, 2, None),
    );
}

#[test]
fn s_snapshot_and_reverse_snapshot_both_interpreters() {
    let source = ORDER.replace("p = Probe.check(); ", "").replace(
        "invariant Probe.check.finish -> flag == false",
        "invariant A.set.finish -> flag == true; invariant B.clear.finish -> flag == false",
    );
    for source in [
        source.clone(),
        source.replace("a = A.set(); b = B.clear()", "b = B.clear(); a = A.set()"),
    ] {
        compare_hand(
            "event-local snapshot",
            &source,
            None,
            &hand(&[("invariant", "satisfied"); 2], "satisfied", 2, 2, None),
        );
    }
}

#[test]
fn s_same_tick_readwrite_and_reverse_predicate_both_interpreters() {
    // Both start0; A.finish then Probe.finish sees true, reverse order sees false.
    let source = ORDER.replace(
        "a = A.set(); b = B.clear(); p = Probe.check()",
        "fork f { a = A.set(); p = Probe.check() }; join all f",
    );
    for source in [
        source.clone(),
        source.replace("flag == false", "flag == true"),
    ] {
        compare_hand(
            "same-tick reader/writer",
            &source,
            None,
            &hand(&[("invariant", "violated")], "violated", 1, 2, None),
        );
    }
}

#[test]
fn s_cancel_normal_ack_tie_and_initial_true_both_interpreters() {
    compare_hand(
        "normal/ack tie",
        &CANCEL.replace("fixed 5min", "fixed 2min"),
        None,
        &hand(&[("invariant", "violated")], "violated", 3, 4, Some(1)),
    );
    let source = CANCEL
        .replace("changed: bool = false", "changed: bool = true")
        .replace("changed = true }", "changed = false }")
        .replace("changed == false", "changed == true");
    compare_hand(
        "ack preserves previous nondefault value",
        &source,
        None,
        &hand(&[("invariant", "satisfied")], "satisfied", 3, 4, Some(1)),
    );
}

#[test]
fn s_cut_keeps_violation_and_fulfilled_is_inconclusive_both_interpreters() {
    for (result, verdict) in [("pass", "inconclusive"), ("fail", "violated")] {
        let source = INVOICE
            .replace(
                "result nondet { pass }",
                &format!("result nondet {{ {result} }}"),
            )
            .replace(
                "capability post() -> Ack { commits; duration fixed 1min",
                "capability post() -> Ack { commits; duration fixed 5min",
            )
            .replace("clock 20min", "clock 3min");
        let mut expected = hand(
            &[("invariant", verdict), ("invariant", "inconclusive")],
            verdict,
            0,
            0,
            None,
        );
        expected["properties"][0]["verdict"] = "inconclusive".into();
        expected["terminals"] = json!([]);
        expected["bounds"]["truncated"] = true.into();
        compare_hand("clock cut after approval write", &source, None, &expected);
    }
}

#[test]
fn s_abort_same_bundle_state_invariant_each_order_both_interpreters() {
    let response = "response Approver.approve.finish -> one_of(end.done) by 18446744073709551615min after Approver.approve.finish otherwise unresolved";
    let invariant = "invariant Approver.approve.finish -> approval == fail";
    let (prefix, _) = INVOICE.split_once("  contract Controls {").unwrap();
    for (text, kinds) in [
        (
            format!("{response}; {invariant}"),
            [("response", "unsupported"), ("invariant", "violated")],
        ),
        (
            format!("{invariant}; {response}"),
            [("invariant", "violated"), ("response", "unsupported")],
        ),
    ] {
        let source = format!("{prefix} contract Controls {{ {text} }} }}");
        let mut expected = hand(&kinds, "violated", 0, 0, None);
        expected["properties"][0]["verdict"] = "unsupported".into();
        expected["bounds"]["truncated"] = Value::Null;
        expected["terminals"] = json!([]);
        expected["absent_diagnostics"] =
            json!(["E-POL-LATENT", "E-CON-0", "W-DEADLOCK-0", "W-BUDGET-0"]);
        compare_hand(
            "overflow does not erase same-bundle invariant",
            &source,
            None,
            &expected,
        );
    }
}
