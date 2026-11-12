//! ADR0025: hand oracle, not expectations harvested from the implementation.
use relca_core::{engine, lower, report, syntax};
use serde_json::{Value, json};
use std::path::PathBuf;
#[path = "support/completion_population.rs"]
mod completion_population;
#[path = "support/completion_oracle.rs"]
mod old_ir_oracle;
#[path = "support/responsibility_defaults_oracle.rs"]
mod responsibility_oracle;

pub const NAMES: [&str; 24] = completion_population::FIXTURES;
pub fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}
pub fn source(n: &str) -> String {
    std::fs::read_to_string(root().join(n).join("model.relca")).unwrap()
}
pub fn expected(n: &str) -> Value {
    serde_json::from_slice(&std::fs::read(root().join(n).join("expected.json")).unwrap()).unwrap()
}
pub fn subset(e: &Value, a: &Value) {
    match e {
        Value::Object(o) => {
            for (k, v) in o {
                if k != "absent_diagnostics" {
                    subset(v, a.get(k).unwrap_or_else(|| panic!("missing {k}: {a}")));
                }
            }
        }
        Value::Array(es) => {
            let aa = a.as_array().expect("array");
            assert_eq!(es.len(), aa.len(), "inventory: {a}");
            for (x, y) in es.iter().zip(aa) {
                subset(x, y)
            }
        }
        _ => assert_eq!(e, a),
    }
}
pub fn assert_fixture_report(name: &str, actual: &Value) {
    let model = lower::lower(&syntax::parse(&source(name)).unwrap()).unwrap();
    // ADR 0054 "範囲" / "意味": lexical unreachable commits still carry E.
    responsibility_oracle::assert_report(&model, actual);
    subset(
        &responsibility_oracle::with_diagnostics(&model, &expected(name)),
        actual,
    );
}

#[test]
fn completion_inventory_rejects_spurious_and_missing_responsibility() {
    let source = source("done-vacuity");
    let actual = json!(report::check(&source));
    assert_fixture_report("done-vacuity", &actual);
    let warning = actual["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["code"] == "W-RESP-0")
        .unwrap()
        .clone();
    assert_eq!(warning["message"], "Ledger has no declared responsibility");
    let mut missing = actual.clone();
    missing["diagnostics"]
        .as_array_mut()
        .unwrap()
        .retain(|d| d != &warning);
    assert!(std::panic::catch_unwind(|| assert_fixture_report("done-vacuity", &missing)).is_err());
    let mut spurious = json!(report::check(&self::source("done-call-kind")));
    assert_fixture_report("done-call-kind", &spurious);
    spurious["diagnostics"]
        .as_array_mut()
        .unwrap()
        .push(warning);
    assert!(
        std::panic::catch_unwind(|| assert_fixture_report("done-call-kind", &spurious)).is_err()
    );
}
pub fn legacy_source(s: &str) -> String {
    s.lines()
        .filter(|l| {
            ![
                "verify recoverable end",
                "verify terminating end",
                "verify almost_sure end",
                "verify probability reached end.",
            ]
            .iter()
            .any(|v| l.trim().starts_with(v))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn all_24_old_syntax_residuals_are_valid_before_new_parser_exists() {
    for n in NAMES {
        let src = legacy_source(&source(n));
        lower::lower(&syntax::parse(&src).unwrap_or_else(|e| panic!("{n}: {e:?}")))
            .unwrap_or_else(|e| panic!("{n}: {e:?}"));
    }
}

#[test]
fn twelve_fixture_pairs_are_exactly_the_single_declared_edit() {
    for (base, fault, from, to) in [
        (
            "done-vacuity",
            "enable-registration",
            "result nondet { bad }",
            "result nondet { good }",
        ),
        (
            "done-recovery",
            "zero-capacity",
            "actor Blocked { capacity 1",
            "actor Blocked { capacity 0",
        ),
        ("done-clock", "short-clock", "clock 6min", "clock 2min"),
        (
            "done-probability",
            "block-failure",
            "actor Blocked { capacity 1",
            "actor Blocked { capacity 0",
        ),
        (
            "done-nondet",
            "nondet-option",
            "result nondet { good }",
            "result nondet { good, bad }",
        ),
        (
            "done-within",
            "tight-deadline",
            "within 5min",
            "within 2min",
        ),
        (
            "done-branches",
            "enable-then",
            "result nondet { bad }",
            "result nondet { bad, good }",
        ),
        (
            "done-people",
            "remove-executor",
            "executors [p,q]",
            "executors [p]",
        ),
        (
            "done-support",
            "positive-support",
            "1:good; 0:bad",
            "0.9:good; 0.1:bad",
        ),
        (
            "done-end-vs-close",
            "pre-end-delay",
            "duration fixed 1min",
            "duration fixed 3min",
        ),
        (
            "done-arrivals",
            "arrival-after-clock",
            "[0min, 3min]",
            "[0min, 6min]",
        ),
        (
            "done-call-kind",
            "call-unused",
            "x = A.run()",
            "x = A.unused()",
        ),
    ] {
        let before = source(base);
        assert_eq!(before.matches(from).count(), 1, "{base}");
        assert_eq!(
            before.replacen(from, to, 1),
            source(&format!("{base}.broken-{fault}")),
            "{base}"
        );
    }
}

#[test]
fn all_24_old_residuals_preserve_the_pre_authored_time_work_and_old_properties() {
    fn without_probability(v: &Value) -> Value {
        match v {
            Value::Object(o) => Value::Object(
                o.iter()
                    .filter(|(k, _)| k.as_str() != "probability")
                    .map(|(k, v)| (k.clone(), without_probability(v)))
                    .collect(),
            ),
            Value::Array(a) => Value::Array(a.iter().map(without_probability).collect()),
            _ => v.clone(),
        }
    }
    for n in NAMES {
        eprintln!("old residual hand arithmetic {n}");
        let s = legacy_source(&source(n));
        let a = serde_json::to_value(report::check(&s)).unwrap();
        let e = expected(n);
        for p in a["properties"].as_array().unwrap() {
            let h = e["properties"]
                .as_array()
                .unwrap()
                .iter()
                .find(|h| h["id"] == p["id"])
                .unwrap();
            subset(&without_probability(h), &without_probability(p));
        }
        subset(
            &without_probability(&e["terminals"]),
            &without_probability(&a["terminals"]),
        );
        if let Some(c) = e.get("case_outcomes") {
            subset(
                &without_probability(c),
                &without_probability(&a["case_outcomes"]),
            );
        }
    }
}
#[test]
fn all_24_reports_match_hand_completion_probability_and_warning_inventories() {
    for n in NAMES {
        let actual = serde_json::to_value(report::check(&source(n))).unwrap();
        assert_fixture_report(n, &actual);
        assert_hand_warnings(n, &actual);
    }
}

// Target inventories are hand-authored, not learned from runtime warnings.
fn hand_warning_targets(n: &str) -> Vec<(&str, &str, &str)> {
    let broken = n.contains(".broken-");
    let base = n.split('.').next().unwrap();
    match (base, broken) {
        ("done-vacuity", false) => vec![
            ("W-UNREACH-0", "Ledger.post", "never_started"),
            ("W-UNREACH-1", "then", ""),
            ("W-UNREACH-2", "registered", ""),
        ],
        ("done-vacuity", true) => {
            vec![("W-UNREACH-1", "else", ""), ("W-UNREACH-2", "returned", "")]
        }
        ("done-recovery" | "done-probability", true) => {
            vec![("W-UNREACH-0", "Blocked.run", "never_started")]
        }
        ("done-nondet", false) => vec![
            ("W-UNREACH-0", "Blocked.run", "never_started"),
            ("W-UNREACH-1", "else", ""),
        ],
        ("done-nondet", true) => vec![("W-UNREACH-0", "Blocked.run", "never_started")],
        ("done-branches", false) => vec![
            ("W-UNREACH-0", "Helper.run", "never_started"),
            ("W-UNREACH-1", "then", ""),
            ("W-UNREACH-2", "special", ""),
        ],
        ("done-people", true) => vec![("W-UNREACH-3", "q", "")],
        ("done-support", false) => vec![
            ("W-UNREACH-0", "Blocked.run", "never_started"),
            ("W-UNREACH-1", "else", ""),
            ("W-UNREACH-2", "bad", ""),
        ],
        ("done-support", true) => vec![
            ("W-UNREACH-0", "Blocked.run", "never_started"),
            ("W-UNREACH-2", "bad", ""),
        ],
        ("done-call-kind", false) => vec![("W-UNREACH-0", "A.unused", "not_called")],
        ("done-call-kind", true) => vec![("W-UNREACH-0", "A.run", "not_called")],
        _ => vec![],
    }
}
fn hand_span(m: &relca_core::ir::Model, code: &str, target: &str) -> relca_core::diag::Span {
    use relca_core::ir::Stmt;
    match code {
        "W-UNREACH-0" => {
            m.capabilities
                .iter()
                .find(|c| format!("{}.{}", m.actors[c.actor.0].name, c.name) == target)
                .unwrap()
                .span
        }
        "W-UNREACH-3" => m.people.iter().find(|p| p.name == target).unwrap().span,
        "W-UNREACH-1" => {
            m.process
                .body
                .iter()
                .find(|s| matches!(s.kind, Stmt::If { .. }))
                .unwrap()
                .span
        }
        _ => {
            fn end(
                m: &relca_core::ir::Model,
                b: &relca_core::ir::Block,
                target: &str,
            ) -> Option<relca_core::diag::Span> {
                for s in b {
                    match &s.kind {
                        Stmt::Response(_, _) => {
                            panic!("completion source oracle does not cover retry end locations")
                        }
                        Stmt::End(e) if m.ends[e.0] == target => return Some(s.span),
                        Stmt::If {
                            then_body,
                            else_body,
                            ..
                        } => {
                            if let Some(v) =
                                end(m, then_body, target).or_else(|| end(m, else_body, target))
                            {
                                return Some(v);
                            }
                        }
                        _ => {}
                    }
                }
                None
            }
            end(m, &m.process.body, target).unwrap()
        }
    }
}
pub fn assert_hand_warnings(n: &str, actual: &Value) {
    let s = source(n);
    let m = lower::lower(&syntax::parse(&legacy_source(&s)).unwrap()).unwrap();
    let ds: Vec<_> = actual["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["code"].as_str().unwrap().starts_with("W-UNREACH-"))
        .collect();
    let hs = hand_warning_targets(n);
    assert_eq!(ds.len(), hs.len(), "hand warning inventory {n}");
    for (d, (code, target, reason)) in ds.iter().zip(hs) {
        let span = hand_span(&m, code, target);
        assert_eq!(d["code"], code);
        assert_eq!(d["severity"], "warning");
        assert_eq!(
            d["span"],
            json!({"start":span.start,"end":span.end}),
            "precise {n} {target}"
        );
        let msg = d["message"].as_str().unwrap();
        assert!(
            msg.contains("Flow") && msg.contains(target) && msg.contains(reason),
            "scope, target, reason {n}: {msg}"
        );
    }
}

#[test]
fn independent_old_ir_probes_agree_with_hand_warning_targets_before_implementation() {
    for n in NAMES {
        if expected(n)["bounds"]["truncated"] == true {
            continue;
        }
        let m = lower::lower(&syntax::parse(&legacy_source(&source(n))).unwrap()).unwrap();
        let ws = old_ir_oracle::warnings(&m);
        let hs = hand_warning_targets(n);
        assert_eq!(ws.len(), hs.len(), "old-IR hand oracle calibration {n}");
        for (w, (code, target, reason)) in ws.iter().zip(hs) {
            assert_eq!(w["code"], code, "{n}");
            assert_eq!(w["target"], target, "{n}");
            if !reason.is_empty() {
                assert_eq!(w["reason"], reason);
            }
            let span = hand_span(&m, code, target);
            assert_eq!(w["span"], json!({"start":span.start,"end":span.end}));
        }
    }
}

#[allow(dead_code)]
pub fn outcome_hand(n: &str, m: &relca_core::ir::Model, o: &relca_core::outcome::Outcome) {
    let e = expected(n);
    assert_eq!(o.truncated, e["bounds"]["truncated"], "cut {n}");
    assert_eq!(
        o.properties.len(),
        e["properties"].as_array().unwrap().len(),
        "property inventory {n}"
    );
    for (p, h) in o.properties.iter().zip(e["properties"].as_array().unwrap()) {
        assert_eq!(
            serde_json::to_value(p.verdict).unwrap(),
            h["verdict"],
            "hand verdict {n}: {}",
            h["id"]
        );
        if let Some(q) = h.get("probability") {
            let a = p.probability.as_ref().expect("required probability");
            assert_eq!(
                a.min.to_string(),
                q["min"].as_str().unwrap(),
                "hand min {n}"
            );
            assert_eq!(
                a.max.to_string(),
                q["max"].as_str().unwrap(),
                "hand max {n}"
            );
        }
        if let Some(cs) = h.get("cases") {
            let actual = p.cases.as_ref().expect("case properties");
            assert_eq!(actual.len(), 2);
            for (a, c) in actual.iter().zip(cs.as_array().unwrap()) {
                assert_eq!(
                    json!(a.case_id.0 + 1),
                    c["case_id"],
                    "hand case identity {n}"
                );
                assert_eq!(json!(a.arrival), c["arrival"], "hand case arrival {n}");
                assert_eq!(
                    serde_json::to_value(a.verdict).unwrap(),
                    c["verdict"],
                    "case {n}"
                );
                if let Some(q) = c.get("probability") {
                    let a = a.probability.as_ref().unwrap();
                    assert_eq!(a.min.to_string(), q["min"].as_str().unwrap());
                    assert_eq!(a.max.to_string(), q["max"].as_str().unwrap());
                }
            }
        }
    }
    let range = |r: relca_core::outcome::Range| json!({"min":r.min,"max":r.max});
    let terminals:Vec<_>=o.terminals.iter().map(|t|{
        let mut v=json!({"end":m.ends[t.end.0],"elapsed":range(t.elapsed),"work":range(t.work),"result_at":t.result_at.map(range)});
        if let Some(q)=&t.probability {v["probability"]=json!({"min":q.min.to_string(),"max":q.max.to_string()});}
        v
    }).collect();
    subset(&e["terminals"], &json!(terminals));
    if let Some(hand) = e.get("case_outcomes") {
        let cases:Vec<_>=o.case_outcomes.as_ref().expect("hand case outcomes").iter().map(|c|{
            let terminals:Vec<_>=c.terminals.iter().map(|t|{
                let (kind,end)=match t.kind {relca_core::outcome::CaseTerminalKind::ReceiptUnknown=>panic!("completion wire oracle does not cover receipt observations"),relca_core::outcome::CaseTerminalKind::End(e)=>("end",json!(m.ends[e.0])),relca_core::outcome::CaseTerminalKind::Rejected=>("rejected",Value::Null),relca_core::outcome::CaseTerminalKind::Overflow(e)=>("overflow",json!(m.ends[e.0])),relca_core::outcome::CaseTerminalKind::CutOff=>("cut_off",Value::Null)};
                let mut v=json!({"kind":kind,"end":end,"completed_at":t.completed_at.map(range),"elapsed":t.elapsed.map(range),"work":t.work.map(range),"result_at":t.result_at.map(range)});
                if let Some(q)=&t.probability{v["probability"]=json!({"min":q.min.to_string(),"max":q.max.to_string()});}
                v
            }).collect();
            json!({"case_id":c.case_id.0+1,"arrival":c.arrival,"terminals":terminals})
        }).collect();
        subset(hand, &json!(cases));
    }
    assert_hand_warnings(n, &json!({"diagnostics":o.diagnostics}));
}

#[test]
fn contract_only_vacuity_and_warning_only_success_preserve_exit_codes() {
    let s = source("done-vacuity");
    let s = s
        .lines()
        .filter(|l| !l.trim().starts_with("verify "))
        .collect::<Vec<_>>()
        .join("\n");
    let r = report::check(&s);
    assert_eq!(r.verdict, relca_core::Verdict::Vacuous);
    assert_eq!(r.verdict.exit_code(), 5);
    assert_eq!(
        r.diagnostics
            .iter()
            .filter(|d| d.code.starts_with("W-UNREACH-"))
            .count(),
        3
    );
    for n in ["done-call-kind", "done-call-kind.broken-call-unused"] {
        let r = report::check(&source(n));
        assert_eq!(r.verdict.exit_code(), 0);
    }
}

#[test]
fn adding_structural_questions_never_changes_old_runtime_outcome() {
    for n in NAMES {
        let src = source(n);
        let old = legacy_source(&src);
        let new = src
            .lines()
            .filter(|l| !l.trim().starts_with("verify probability reached end."))
            .collect::<Vec<_>>()
            .join("\n");
        let om = lower::lower(&syntax::parse(&old).unwrap()).unwrap();
        let nm = lower::lower(&syntax::parse(&new).unwrap()).unwrap();
        let a = engine::explore(&om);
        let mut b = engine::explore(&nm);
        b.properties.retain(|p| {
            !matches!(
                format!("{:?}", p.property).as_str(),
                "RecoverableEnd" | "TerminatingEnd" | "AlmostSureEnd"
            )
        });
        assert_eq!(
            a, b,
            "adding only structural questions must not enable terminal probability/census semantics: {n}"
        );
    }
}

#[test]
fn new_structural_questions_refuse_simulation_before_any_attempt() {
    for property in ["recoverable", "terminating", "almost_sure"] {
        let s=legacy_source(&source("done-within")).replace("  verify safety",&format!("  simulation replications 4 seed 7 confidence 0.5\n  verify {property} end\n  verify safety"));
        let r = serde_json::to_value(report::simulate(&s)).unwrap();
        assert_eq!(r["verdict"], "unsupported", "{property}: {r}");
        assert_eq!(r["simulation"]["attempted_replications"], 0);
        assert!(
            r["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["code"] == "U-SEM-0")
        );
        let m = lower::lower(&syntax::parse(&s).unwrap()).unwrap();
        let engine = engine::simulate(&m);
        let reference = relca_core::reference::simulate(&m).outcome;
        for out in [engine, reference] {
            assert_eq!(out.attempted_replications, 0);
            assert!(
                out.diagnostics
                    .iter()
                    .any(|d| d.code == "U-SEM-0" && d.span == Some(m.verify[0].span))
            );
        }
    }
}

#[test]
fn comparison_simulation_refuses_new_questions_in_every_plan_before_attempts() {
    let src=source("done-within").replace("  bounds","  cut C { include [A] }\n  configuration APlan { cut C; process Flow }\n  configuration BPlan { cut C; process Flow }\n  simulation replications 4 seed 7 confidence 0.5\n  bounds");
    let v = serde_json::to_value(relca_core::compare::compare(
        &src,
        &["APlan", "BPlan"],
        relca_core::compare::Analysis::Simulation,
    ))
    .unwrap();
    assert_eq!(v["verdict"], "unsupported");
    assert_eq!(v["plans"].as_array().unwrap().len(), 2);
    for plan in v["plans"].as_array().unwrap() {
        let r = &plan["report"];
        assert_eq!(r["verdict"], "unsupported");
        assert_eq!(r["simulation"]["attempted_replications"], 0);
        assert!(
            r["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["code"] == "U-SEM-0")
        );
    }
}

#[test]
fn end_deadline_simulation_has_hand_computed_outward_cp_endpoints() {
    for (deadline, interval, verdict) in [
        (
            5,
            json!({"lower":"741455/1048576","upper":"1"}),
            "inconclusive",
        ),
        (1, json!({"lower":"0","upper":"307121/1048576"}), "violated"),
    ] {
        let s =
            source("done-within").replace("duration {2min:0.75,5min:0.25}", "duration fixed 2min");
        let s = s
            .lines()
            .filter(|l| {
                ![
                    "verify recoverable",
                    "verify terminating",
                    "verify almost_sure",
                    "verify safety",
                    "verify reachable",
                ]
                .iter()
                .any(|p| l.trim().starts_with(p))
            })
            .collect::<Vec<_>>()
            .join("\n")
            .replace(
                "  bounds",
                "  simulation replications 4 seed 7 confidence 0.5\n  bounds",
            )
            .replace("within 5min", &format!("within {deadline}min"));
        let r = serde_json::to_value(report::simulate(&s)).unwrap();
        assert_eq!(r["properties"][0]["cases"][0]["interval"], interval);
        assert_eq!(r["properties"][0]["verdict"], verdict);
        assert_eq!(r["unchecked"], json!(["unreachable"]));
        assert!(
            !r["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["code"].as_str().unwrap().starts_with("W-UNREACH-"))
        );
    }
}
