//! ADR 0010 independent finite-contract population: 19 directories / 21 inputs.
//! Engine and reference each meet the hand oracle BEFORE they are compared.
//! No contract IR type names, monitor helpers, report fold, or engine helpers
//! are used to build expectations. New hand order/ack examples are separate.

use relca_core::{
    Verdict, engine, ir, lower,
    outcome::Outcome,
    reference::{self, StopReason},
    report, syntax,
};
use serde_json::{Value, json};
use std::{collections::BTreeSet, fs, path::Path};

const FIXTURES: [&str; 19] = [
    "contract-cancel",
    "contract-cancel.broken-ignore-cancel",
    "contract-cut",
    "contract-cut.broken-late-target",
    "contract-diff",
    "contract-diff.broken-zero-deadline",
    "contract-invariant",
    "contract-invariant.broken-cut-guard",
    "contract-once",
    "contract-once.broken-repeat-result",
    "contract-order",
    "contract-order.broken-parallel",
    "contract-requires",
    "contract-requires.broken-cut-guard",
    "contract-response",
    "contract-response.broken-late-finish",
    "contract-response.broken-short-clock",
    "contract-vacuous",
    "contract-vacuous.broken-enable-branch",
];

fn lower_selected(source: &str, configuration: Option<&str>) -> ir::Model {
    let ast = syntax::parse(source)
        .unwrap_or_else(|d| panic!("parse rejected accepted hand input: {d:?}"));
    match configuration {
        None => lower::lower(&ast),
        Some(name) => lower::lower_configuration(&ast, name).map(|s| s.model),
    }
    .unwrap_or_else(|d| panic!("lower rejected accepted hand input: {d:?}"))
}

fn terminals(model: &ir::Model, outcome: &Outcome) -> Value {
    let mut result: Vec<_> = outcome
        .terminals
        .iter()
        .map(|t| {
            json!({
                "end":model.ends[t.end.0],
                "elapsed":{"min":t.elapsed.min,"max":t.elapsed.max},
                "work":{"min":t.work.min,"max":t.work.max},
                "result_at":t.result_at.map(|r| json!({"min":r.min,"max":r.max}))
            })
        })
        .collect();
    result.sort_by(|a, b| a["end"].as_str().cmp(&b["end"].as_str()));
    json!(result)
}

fn sorted_terminals(value: &Value) -> Value {
    let mut result = value.as_array().unwrap().clone();
    result.sort_by(|a, b| a["end"].as_str().cmp(&b["end"].as_str()));
    json!(result)
}

fn codes(out: &Outcome) -> BTreeSet<&str> {
    out.diagnostics.iter().map(|d| d.code.as_str()).collect()
}

fn hand_outcome(label: &str, model: &ir::Model, out: &Outcome, expected: &Value) {
    assert_eq!(
        out.truncated,
        expected["bounds"]["truncated"].as_bool().unwrap(),
        "{label}"
    );
    assert_eq!(
        terminals(model, out),
        sorted_terminals(&expected["terminals"]),
        "{label}: hand metrics"
    );
    let expected_properties = expected["properties"].as_array().unwrap();
    assert_eq!(
        out.properties.len(),
        expected_properties.len(),
        "{label}: property inventory"
    );
    assert_eq!(model.verify.len(), expected_properties.len());
    // Every fixture deliberately places safety first, then its clauses in source order.
    for ((actual, verify), expected) in out
        .properties
        .iter()
        .zip(&model.verify)
        .zip(expected_properties)
    {
        assert_eq!(
            actual.property, verify.property,
            "{label}: outcome request identity"
        );
        let verdict: Verdict = serde_json::from_value(expected["verdict"].clone()).unwrap();
        assert_eq!(
            actual.verdict, verdict,
            "{label}: hand clause {}",
            expected["id"]
        );
    }
    for code in ["W-DEADLOCK-0", "W-BUDGET-0", "E-CON-0", "U-SEM-0"] {
        assert!(!codes(out).contains(code), "{label}: unexpected {code}");
    }
}

/// ADR 0035 U12(意味の注記の条件と性質の行の形): atomic の check-and-insert を
/// 持たない旧いモデルでは、at_most_once の行にだけ notes ["mechanism_unmodeled"] が
/// 足される。他の行・verdict・cases は旧い手計算の期待のまま。at_most_once 以外の
/// 行に notes が付けば、この期待と一致しないので落ちる。
fn with_at_most_once_notes(source: &str, properties: &Value) -> Value {
    assert!(!source.contains("atomic"), "pre-A3 oracle only");
    let mut rows = properties.clone();
    for row in rows.as_array_mut().unwrap() {
        assert!(row.get("notes").is_none(), "hand oracle carries no notes");
        if row["id"].as_str().unwrap().ends_with(" at_most_once") {
            row["notes"] = json!(["mechanism_unmodeled"]);
        }
    }
    rows
}
fn compare_hand(label: &str, source: &str, selection: Option<&str>, expected: &Value) {
    let model = lower_selected(source, selection);
    let reference = reference::explore(&model);
    assert_eq!(
        reference.stop,
        StopReason::Complete,
        "{label}: zero reference exclusions"
    );
    let engine = engine::explore(&model);
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
    // Public report is a third observation, never the oracle for either interpreter.
    let report = match selection {
        None => report::check(source),
        Some(name) => report::check_configuration(source, name),
    };
    let report = serde_json::to_value(report).unwrap();
    assert_eq!(
        report["verdict"], expected["verdict"],
        "{label}: report root"
    );
    assert_eq!(
        report["properties"],
        with_at_most_once_notes(source, &expected["properties"]),
        "{label}: public ids"
    );
    assert_eq!(
        engine.properties, reference.outcome.properties,
        "{label}: differential clauses"
    );
    assert_eq!(
        terminals(&model, &engine),
        terminals(&model, &reference.outcome)
    );
    // Same seam as differential.rs: the reference outcome starts with the
    // lowered model's static diagnostics; engine::explore carries runtime only.
    assert!(
        reference
            .outcome
            .diagnostics
            .starts_with(&model.diagnostics),
        "{label}: reference static prefix"
    );
    let runtime: BTreeSet<&str> = reference.outcome.diagnostics[model.diagnostics.len()..]
        .iter()
        .map(|d| d.code.as_str())
        .collect();
    assert_eq!(
        codes(&engine),
        runtime,
        "{label}: exact runtime diagnostic code sets"
    );
    assert_eq!(engine.truncated, reference.outcome.truncated);
}

#[test]
fn contract_population_matches_hand_oracle_in_both_interpreters() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let mut directories: Vec<_> = fs::read_dir(&root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| {
            p.is_dir()
                && p.file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .starts_with("contract-")
        })
        .collect();
    directories.sort();
    assert_eq!(
        directories
            .iter()
            .map(|p| p.file_name().unwrap().to_str().unwrap())
            .collect::<Vec<_>>(),
        FIXTURES
    );
    let mut inputs = 0;
    let mut accepted = 0;
    let mut rejected = Vec::new();
    let mut exclusions = Vec::new();
    for directory in directories {
        let name = directory.file_name().unwrap().to_str().unwrap();
        for file in ["model.relca", "expected.json", "calc.md"] {
            assert!(directory.join(file).is_file());
        }
        let source = fs::read_to_string(directory.join("model.relca")).unwrap();
        let expected: Value =
            serde_json::from_str(&fs::read_to_string(directory.join("expected.json")).unwrap())
                .unwrap();
        let selections: &[Option<&str>] = if name.starts_with("contract-diff") {
            &[Some("Before"), Some("After")]
        } else {
            &[None]
        };
        for &selection in selections {
            inputs += 1;
            let label = format!("{name}[{selection:?}]");
            let parsed = syntax::parse(&source).and_then(|ast| match selection {
                None => lower::lower(&ast),
                Some(s) => lower::lower_configuration(&ast, s).map(|s| s.model),
            });
            match parsed {
                Err(d) => {
                    rejected.push(format!("{label}: {d:?}"));
                    continue;
                }
                Ok(model) => {
                    accepted += 1;
                    let run = reference::explore(&model);
                    if run.stop != StopReason::Complete {
                        exclusions.push(format!("{label}: {:?}", run.stop));
                    }
                }
            }
            let expected = match selection {
                Some("Before") => &expected["from"]["report"],
                Some("After") => &expected["to"]["report"],
                _ => &expected,
            };
            compare_hand(&label, &source, selection, expected);
        }
    }
    assert_eq!(inputs, 21, "19 directories / 21 executable inputs");
    assert_eq!(
        rejected.len(),
        0,
        "no static rejection exclusions: {}",
        rejected.join("\n")
    );
    assert_eq!(accepted, 21);
    assert_eq!(
        exclusions.len(),
        0,
        "no reference exclusions: {}",
        exclusions.join("\n")
    );
}

#[test]
fn c_order_both_directions_reference_has_no_fixed_due_order() {
    let source = include_str!("../../../fixtures/contract-order.broken-parallel/model.relca")
        .replace("by 1min", "by 0min")
        .replace("contract Controls {", "contract Controls { response B.b.finish -> one_of(A.a.finish) by 0min after B.b.finish otherwise unresolved;");
    let expected = json!({"verdict":"violated","bounds":{"truncated":false},
        "properties":[{"id":"safety no_resource_overcommit","verdict":"satisfied"},{"id":"contract Controls #1 response","verdict":"violated"},{"id":"contract Controls #2 response","verdict":"violated"}],
        "terminals":[{"end":"done","elapsed":{"min":3,"max":3},"work":{"min":4,"max":4},"result_at":null}]});
    // Start AB/BA at0, finish AB/BA at1. Either direction has one bad finish order.
    compare_hand(
        "complementary zero-deadline response",
        &source,
        None,
        &expected,
    );
}

#[test]
fn c_normal_ack_tie_reference_keeps_both_branches() {
    let source = include_str!("../../../fixtures/contract-cancel/model.relca")
        .replace("fixed 3min", "fixed 2min")
        .replace("contract Controls {", "contract Controls { response Fast.win.finish -> one_of(Slow.work.finish) by 1min after Fast.win.finish otherwise unresolved;");
    // Fast1, adoption1, Slow normal2 or ack2. Both metrics coincide: e2/w3/r1.
    // Normal makes original clauses active; ack makes the new response violated.
    let expected = json!({"verdict":"violated","bounds":{"truncated":false},
        "properties":[{"id":"safety no_resource_overcommit","verdict":"satisfied"},{"id":"contract Controls #1 response","verdict":"violated"},{"id":"contract Controls #2 invariant","verdict":"satisfied"},{"id":"contract Controls #3 at_most_once","verdict":"satisfied"},{"id":"contract Controls #4 response","verdict":"satisfied"}],
        "terminals":[{"end":"done","elapsed":{"min":2,"max":2},"work":{"min":3,"max":3},"result_at":{"min":1,"max":1}}]});
    compare_hand("normal/ack tie", &source, None, &expected);
}
