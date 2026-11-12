//! ADR 0011 sixth population: hand oracle first, independent interpreters second.
//! No new contract/people IR types or engine monitor helpers build expectations.
use relca_core::{
    Verdict, engine, ir, lower,
    outcome::Outcome,
    reference::{self, StopReason},
    report, syntax,
};
use serde_json::{Value, json};
use std::{collections::BTreeSet, fs, path::Path};

const FIXTURES: [&str; 14] = [
    "exec-anonymous",
    "exec-anonymous.broken-no-executor",
    "exec-authority",
    "exec-authority.broken-missing-grant",
    "exec-cancel",
    "exec-cancel.broken-ignore-cancel",
    "exec-permitted",
    "exec-permitted.broken-narrow-permit",
    "exec-separation",
    "exec-separation.broken-shared-approver",
    "exec-shared-person",
    "exec-shared-person.broken-reused-person",
    "exec-vacuous",
    "exec-vacuous.broken-shared-approver",
];

fn lower_selected(source: &str, selection: Option<&str>) -> ir::Model {
    let ast = syntax::parse(source).unwrap_or_else(|d| panic!("hand input parse rejected: {d:?}"));
    match selection {
        None => lower::lower(&ast),
        Some(s) => lower::lower_configuration(&ast, s).map(|s| s.model),
    }
    .unwrap_or_else(|d| panic!("hand input lower rejected: {d:?}"))
}

fn terminals(model: &ir::Model, out: &Outcome) -> Value {
    let mut rows:Vec<_>=out.terminals.iter().map(|t|json!({"end":model.ends[t.end.0],"elapsed":{"min":t.elapsed.min,"max":t.elapsed.max},"work":{"min":t.work.min,"max":t.work.max},"result_at":t.result_at.map(|r|json!({"min":r.min,"max":r.max}))})).collect();
    rows.sort_by(|a, b| a["end"].as_str().cmp(&b["end"].as_str()));
    json!(rows)
}

fn codes(out: &Outcome) -> BTreeSet<&str> {
    out.diagnostics.iter().map(|d| d.code.as_str()).collect()
}

fn hand_outcome(label: &str, model: &ir::Model, out: &Outcome, expected: &Value) {
    if let Some(truncated) = expected["bounds"]["truncated"].as_bool() {
        assert_eq!(out.truncated, truncated, "{label}");
    }
    assert_eq!(
        terminals(model, out),
        expected["terminals"],
        "{label}: hand metrics"
    );
    let properties = expected["properties"].as_array().unwrap();
    assert_eq!(
        out.properties.len(),
        properties.len(),
        "{label}: hand inventory"
    );
    assert_eq!(model.verify.len(), properties.len());
    for ((actual, verify), expected) in out.properties.iter().zip(&model.verify).zip(properties) {
        assert_eq!(actual.property, verify.property, "{label}");
        let verdict: Verdict = serde_json::from_value(expected["verdict"].clone()).unwrap();
        assert_eq!(actual.verdict, verdict, "{label}: hand {}", expected["id"]);
    }
    for code in expected["absent_diagnostics"]
        .as_array()
        .into_iter()
        .flatten()
    {
        assert!(
            !codes(out).contains(code.as_str().unwrap()),
            "{label}: {code}"
        );
    }
}

fn compare_hand(label: &str, source: &str, selection: Option<&str>, expected: &Value) {
    let model = lower_selected(source, selection);
    let reference = reference::explore(&model);
    let abort = expected["bounds"]["truncated"].is_null();
    assert_eq!(
        reference.stop,
        if abort {
            StopReason::SemanticAbort
        } else {
            StopReason::Complete
        },
        "{label}: no unexpected reference exclusion"
    );
    assert_eq!(
        reference.truncated(),
        expected["bounds"]["truncated"].as_bool()
    );
    let engine = engine::explore(&model);
    // Both must independently pass the oracle before any equality assertion.
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
        "{label}: differential clauses"
    );
    assert_eq!(
        terminals(&model, &engine),
        terminals(&model, &reference.outcome)
    );
    assert!(
        reference
            .outcome
            .diagnostics
            .starts_with(&model.diagnostics)
    );
    let runtime: BTreeSet<_> = reference.outcome.diagnostics[model.diagnostics.len()..]
        .iter()
        .map(|d| d.code.as_str())
        .collect();
    assert_eq!(codes(&engine), runtime, "{label}: runtime diagnostic seam");
    assert_eq!(engine.truncated, reference.outcome.truncated);
}

#[test]
fn executor_population_fixed_14_directories_16_inputs_15_accepted_1_rejected_zero_exclusions() {
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
                    .starts_with("exec-")
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
    let (mut inputs, mut accepted, mut rejected, mut compared) = (0, 0, 0, 0);
    let mut errors = Vec::new();
    let mut exclusions = Vec::new();
    for directory in directories {
        let name = directory.file_name().unwrap().to_str().unwrap();
        for file in ["model.relca", "calc.md", "expected.json"] {
            assert!(directory.join(file).is_file());
        }
        let source = fs::read_to_string(directory.join("model.relca")).unwrap();
        let expected: Value =
            serde_json::from_str(&fs::read_to_string(directory.join("expected.json")).unwrap())
                .unwrap();
        let selections: &[Option<&str>] = if name.starts_with("exec-vacuous") {
            &[Some("AsIs"), Some("ToBeA")]
        } else {
            &[None]
        };
        for &selection in selections {
            inputs += 1;
            let label = format!("{name}[{selection:?}]");
            let expected = match selection {
                Some("AsIs") => &expected["from"]["report"],
                Some("ToBeA") => &expected["to"]["report"],
                _ => &expected,
            };
            let parsed = syntax::parse(&source).and_then(|ast| match selection {
                None => lower::lower(&ast),
                Some(s) => lower::lower_configuration(&ast, s).map(|s| s.model),
            });
            if name == "exec-anonymous.broken-no-executor" {
                rejected += 1;
                let diagnostics = parsed.expect_err("anonymous clauses must reject statically");
                assert!(
                    diagnostics.iter().any(|d| d.code == "U-SEM-0"),
                    "{label}: {diagnostics:?}"
                );
                let public = serde_json::to_value(report::check(&source)).unwrap();
                assert_eq!(public["verdict"], "unsupported");
                assert_eq!(public["properties"], json!([]));
                assert_eq!(public["terminals"], json!([]));
                assert_eq!(public["bounds"]["truncated"], Value::Null);
                continue;
            }
            match parsed {
                Err(d) => {
                    errors.push(format!("{label}: unexpected static rejection {d:?}"));
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
            compare_hand(&label, &source, selection, expected);
            compared += 1;
        }
    }
    assert_eq!(inputs, 16);
    assert_eq!(rejected, 1);
    assert!(errors.is_empty(), "{}", errors.join("\n"));
    assert_eq!(accepted, 15);
    assert_eq!(compared, 15);
    assert!(
        exclusions.is_empty(),
        "zero reference exclusions: {}",
        exclusions.join("\n")
    );
}

fn expected(
    kinds: &[(&str, &str)],
    root: &str,
    min: u64,
    max: u64,
    work: u64,
    result: Option<u64>,
) -> Value {
    let mut properties = vec![json!({"id":"safety no_resource_overcommit","verdict":"satisfied"})];
    for (i, (kind, verdict)) in kinds.iter().enumerate() {
        properties
            .push(json!({"id":format!("contract Controls #{} {kind}",i+1),"verdict":verdict}));
    }
    json!({"verdict":root,"bounds":{"truncated":false},"properties":properties,"terminals":[{"end":"done","elapsed":{"min":min,"max":max},"work":{"min":work,"max":work},"result_at":result.map(|r|json!({"min":r,"max":r}))}],"absent_diagnostics":["W-DEADLOCK-0","W-BUDGET-0","E-CON-0","U-SEM-0"]})
}

fn model(process: &str, clauses: &str) -> String {
    format!(
        r#"model M version "0.2" {{
      type T = enum {{ ok,bad }}
      people [p,q]
      actor A {{ executors [p,q]; capability a() -> T {{ commits; duration fixed 1min; result nondet {{ ok }} }} }}
      actor B {{ executors [p]; capability b() -> T {{ duration fixed 1min; result nondet {{ ok }} }} }}
      authority G {{ basis "table"; grants p to A.a }}
      process P {{ {process} }}
      bounds {{ cases 1; live_tasks 2; clock 20min }}
      verify safety no_resource_overcommit
      contract Controls {{ {clauses} }}
    }}"#
    )
}

#[test]
fn e_allassignments_time_both_executor_orders_and_anonymous_independence() {
    let base =
        include_str!("../../../fixtures/exec-shared-person.broken-reused-person/model.relca");
    for order in ["[p,q]", "[q,p]"] {
        let source = base.replace(
            "actor A { executors [p]",
            &format!("actor A {{ executors {order}"),
        );
        // A=p first serializes: e8. A=q/B=p starts both at0: e5. Work8.
        compare_hand(
            "assignment-dependent time",
            &source,
            None,
            &expected(&[], "satisfied", 5, 8, 8, None),
        );
    }
    compare_hand(
        "anonymous independent",
        &base.replace("actor A { executors [p]", "actor A { capacity 1"),
        None,
        &expected(&[], "satisfied", 5, 5, 8, None),
    );
}

#[test]
fn e_separation_allpairs_path_local_and_bundle_both_interpreters() {
    let source = model(
        "x = A.a(); z = A.a(); y = B.b(); end done",
        "separation A.a.finish, B.b.finish",
    );
    // (p,p,p),(p,q,p),(q,p,p) violate; (q,q,p) satisfies. e3/w3.
    for source in [
        source.clone(),
        source.replace(
            "separation A.a.finish, B.b.finish",
            "separation B.b.finish, A.a.finish",
        ),
    ] {
        compare_hand(
            "allpairs",
            &source,
            None,
            &expected(&[("separation", "violated")], "violated", 3, 3, 3, None),
        );
    }
    let source=model("g = Gate.go(); if g == ok { x = A.a() } else { y = B.b() }; end done","separation A.a.finish, B.b.finish")
        .replace("actor A", "actor Gate { capacity 1; capability go() -> T { duration fixed 1min; result nondet { ok,bad } } }; actor A");
    compare_hand(
        "path-local antecedent",
        &source,
        None,
        &expected(&[("separation", "vacuous")], "vacuous", 2, 2, 2, None),
    );
    for (clause, verdict) in [
        ("separation A.a.finish, A.a.commit", "violated"),
        ("separation A.a.finish, A.a.finish(ok)", "violated"),
        ("separation A.a.finish(ok), A.a.finish(bad)", "vacuous"),
    ] {
        compare_hand(
            "finish bundle",
            &model("x = A.a(); end done", clause),
            None,
            &expected(&[("separation", verdict)], verdict, 1, 1, 1, None),
        );
    }
}

#[test]
fn e_named_due_ack_and_permitted_start_cancel_both_interpreters() {
    let base = include_str!("../../../fixtures/exec-cancel/model.relca");
    let tied=base.replace("fixed 5min","fixed 2min").replace("contract Controls {","contract Controls { response Fast.win.finish -> one_of(Slow.work.finish) by 1min after Fast.win.finish otherwise unresolved;");
    // Normal Slow finish2 violates Authority/separation; ack2 instead violates response.
    compare_hand(
        "normal/ack tie",
        &tied,
        None,
        &expected(
            &[
                ("response", "violated"),
                ("permitted", "satisfied"),
                ("authority_basis", "violated"),
                ("separation", "violated"),
            ],
            "violated",
            3,
            3,
            4,
            Some(1),
        ),
    );
    compare_hand(
        "start before cancel",
        &base.replace("process Flow", "permit q to Slow.work; process Flow"),
        None,
        &expected(
            &[
                ("permitted", "violated"),
                ("authority_basis", "vacuous"),
                ("separation", "vacuous"),
            ],
            "violated",
            3,
            3,
            4,
            Some(1),
        ),
    );
}

#[test]
fn e_authority_no_commit_other_authority_and_other_capability_both_interpreters() {
    let correct = include_str!("../../../fixtures/exec-authority/model.relca");
    let broken = include_str!("../../../fixtures/exec-authority.broken-missing-grant/model.relca");
    for (source, verdict) in [(correct, "satisfied"), (broken, "violated")] {
        compare_hand(
            "without commits",
            &source.replace("commits;", ""),
            None,
            &expected(&[("authority_basis", verdict)], verdict, 1, 1, 1, None),
        );
    }
    let other = broken.replace(
        "process Flow",
        "authority H { basis \"other\"; grants q to Runner.run }; process Flow",
    );
    compare_hand(
        "wrong Authority",
        &other,
        None,
        &expected(
            &[("authority_basis", "violated")],
            "violated",
            1,
            1,
            1,
            None,
        ),
    );
    let other=broken.replace("capability run()","capability other() -> T { duration fixed 1min; result nondet { ok } }; capability run()")
        .replace("grants p to Runner.run;","grants p to Runner.run; grants q to Runner.other;");
    compare_hand(
        "wrong capability",
        &other,
        None,
        &expected(
            &[("authority_basis", "violated")],
            "violated",
            1,
            1,
            1,
            None,
        ),
    );
}

#[test]
fn e_cut_retains_and_fulfilled_still_inconclusive_both_interpreters() {
    let source = model(
        "x = A.a(); y = B.b(); end done",
        "permitted A.a; authority_basis A.a G; separation A.a.start, B.b.finish",
    )
    .replace("executors [p,q]", "executors [p]")
    .replace(
        "capability a() -> T { commits; duration fixed 1min",
        "capability a() -> T { commits; duration fixed 2min",
    )
    .replace("clock 20min", "clock 1min");
    for (source, verdict) in [
        (source.clone(), "inconclusive"),
        (
            source.replace("process P", "permit q to A.a; process P"),
            "violated",
        ),
    ] {
        let mut hand = expected(
            &[
                ("permitted", verdict),
                ("authority_basis", "inconclusive"),
                ("separation", "inconclusive"),
            ],
            verdict,
            0,
            0,
            0,
            None,
        );
        hand["terminals"] = json!([]);
        hand["bounds"]["truncated"] = json!(true);
        hand["properties"][0]["verdict"] = json!("inconclusive");
        compare_hand("clock1 prefix", &source, None, &hand);
    }
    let source = model("x = A.a(); y = B.b(); end done", "authority_basis A.a G")
        .replace("executors [p,q]", "executors [p]")
        .replace(
            "capability b() -> T { duration fixed 1min",
            "capability b() -> T { duration fixed 5min",
        )
        .replace("clock 20min", "clock 3min");
    let mut hand = expected(
        &[("authority_basis", "inconclusive")],
        "inconclusive",
        0,
        0,
        0,
        None,
    );
    hand["terminals"] = json!([]);
    hand["bounds"]["truncated"] = json!(true);
    hand["properties"][0]["verdict"] = json!("inconclusive");
    compare_hand("finished A before cut", &source, None, &hand);
}

#[test]
fn e_abort_same_bundle_both_interpreters() {
    let response = "response A.a.finish -> one_of(end.done) by 18446744073709551615min after A.a.finish otherwise unresolved";
    let sep = "separation A.a.finish, A.a.commit";
    let auth = "authority_basis A.a G";
    for (clauses, kinds) in [
        (
            format!("{response}; {sep}; {auth}"),
            vec![
                ("response", "unsupported"),
                ("separation", "violated"),
                ("authority_basis", "violated"),
            ],
        ),
        (
            format!("{auth}; {sep}; {response}"),
            vec![
                ("authority_basis", "violated"),
                ("separation", "violated"),
                ("response", "unsupported"),
            ],
        ),
    ] {
        let source = model("x = A.a(); end done", &clauses)
            .replace("executors [p,q]", "executors [p]")
            .replace("grants p to A.a", "grants q to A.a");
        let mut hand = expected(&kinds, "violated", 0, 0, 0, None);
        hand["terminals"] = json!([]);
        hand["bounds"]["truncated"] = Value::Null;
        hand["properties"][0]["verdict"] = json!("unsupported");
        hand["absent_diagnostics"] = json!(["W-DEADLOCK-0", "W-BUDGET-0", "E-CON-0"]);
        compare_hand("overflow same finish bundle", &source, None, &hand);
    }
}
