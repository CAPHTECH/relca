//! ADR 0020 independent hand oracle. No private generation/comparison seams.
use relca_core::{mutation, report};
use serde_json::{Value, json};
use std::{collections::BTreeSet, path::Path};
#[path = "support/response_boundary_oracle.rs"]
pub mod response_boundary;

pub const FIXTURES: [&str; 12] = [
    "mut-empty",
    "mut-empty.broken-invalid",
    "mut-evidence",
    "mut-evidence.broken-no-independent-read",
    "mut-gate",
    "mut-gate.broken-unsupported",
    "mut-guard",
    "mut-guard.broken-no-contract",
    "mut-limit",
    "mut-limit.broken-many-sites",
    "mut-origin",
    "mut-origin.broken-route-only",
];
pub const KINDS: [&str; 10] = [
    "evidence_missing",
    "authority_expired",
    "hold_delayed",
    "command_resent",
    "summary_reused",
    "stale_definition_write",
    "receipt_unknown",
    "old_promise_denominator_removed",
    "refusal_path_cut",
    "original_access_removed",
];
pub const UNSUPPORTED: [(usize, &str); 2] = [
    (5, "rule_version_unavailable"),
    (7, "quality_denominator_unavailable"),
];

pub fn source(name: &str) -> String {
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .join(name)
            .join("model.relca"),
    )
    .unwrap()
}
pub fn run(s: &str) -> Value {
    serde_json::to_value(mutation::mutate(s)).unwrap()
}

/// Expected identities and outcomes are arithmetic/source-order, never harvested
/// from the generator. The report stays partial only for unstable prose/spans.
pub fn hand_rows(name: &str) -> Vec<Value> {
    let blocked = name == "mut-empty.broken-invalid" || name.starts_with("mut-gate");
    let reads: Vec<(&str, &str, &str, Vec<&str>)> = if blocked || name == "mut-empty" {
        vec![]
    } else if name.starts_with("mut-evidence") {
        vec![
            ("Reader", "enter", "Invoice", vec!["entry"]),
            (
                "Clerk",
                "match_po",
                if name.contains("broken") {
                    "Invoice"
                } else {
                    "PurchaseOrder"
                },
                vec!["po"],
            ),
        ]
    } else if name.starts_with("mut-guard") {
        vec![("Reader", "enter", "Invoice", vec!["entry"])]
    } else if name == "mut-origin" {
        vec![
            ("Reader", "enter", "Invoice", vec!["entry"]),
            ("Approver", "approve", "Invoice", vec!["approval"]),
        ]
    } else if name == "mut-origin.broken-route-only" {
        vec![
            ("Reader", "enter", "Invoice", vec!["entry"]),
            ("Clerk", "recheck", "Invoice", vec![]),
        ]
    } else {
        vec![]
    };
    let guards: Vec<(&str, &str)> = if name.starts_with("mut-guard") {
        vec![("body/2", "po"), ("body/4", "approval")]
    } else if name == "mut-origin" {
        vec![("body/2", "approval")]
    } else {
        vec![]
    };
    KINDS.iter().enumerate().map(|(index, kind)| {
        if let Some((_,reason)) = UNSUPPORTED.iter().find(|(i,_)| *i == index) {
            return json!({"kind":kind,"status":"unsupported","reason":reason,"mutants":[]});
        }
        if blocked { return json!({"kind":kind,"status":"blocked","reason":"original_not_decidable","mutants":[]}); }
        if matches!(index,3|6) {return response_boundary::row(&source(name),kind);}
        let mut mutants = vec![];
        if index == 0 || index == 9 {
            if name.starts_with("mut-limit") {
                let n = if name.contains("broken") {65} else {64};
                for i in 0..n {
                    let src = format!("S{i:02}");
                    let not_run = index == 9 && n == 65 && i >= 63;
                    mutants.push(json!({"id":format!("{kind}:Probe.read:reads:{src}"),"operator":"remove_read",
                        "site":{"actor":"Probe","capability":"read","source":src,"affected_bindings":[],"process":null,"statement_path":null,"retained_branch":null},
                        "result":if not_run {"not_run"} else {"undetected"},"reason":if not_run {"mutant_limit"} else {"no_assigned_check"}}));
                }
            } else {
                for (actor,cap,src,bindings) in &reads {
                    let detected = (index == 0 && name == "mut-evidence" && *actor == "Clerk")
                        || (index == 9 && name == "mut-origin" && *actor == "Approver");
                    mutants.push(json!({"id":format!("{kind}:{actor}.{cap}:reads:{src}"),"operator":"remove_read",
                        "site":{"actor":actor,"capability":cap,"source":src,"affected_bindings":bindings,"process":null,"statement_path":null,"retained_branch":null},
                        "result":if detected {"detected"} else {"undetected"}}));
                }
            }
        } else if index == 4 && name.starts_with("mut-guard") {
            mutants.push(json!({"id":"summary_reused:Flow:body/0:entry->approval","operator":"duplicate_summary",
                "site":{"process":"Flow","actor":"Reader","capability":"enter","source":null,"statement_path":"body/0","retained_branch":null,"affected_bindings":["entry","approval"],"duplicate_binding":"entry_dup"},"result":"detected","reason":null}));
        } else if index == 2 || index == 8 {
            for (path,binding) in &guards {
                let detected = index == 8 || (index == 2 && name == "mut-guard");
                mutants.push(json!({"id":format!("{kind}:Flow:{path}:keep_then"),"operator":"bypass_guard",
                    "site":{"process":"Flow","actor":null,"capability":null,"source":null,"statement_path":path,"retained_branch":"then","affected_bindings":[binding]},
                    "result":if detected {"detected"} else {"undetected"}}));
            }
        }
        json!({"kind":kind,"status":if mutants.is_empty(){"not_applicable"}else{"generated"},
            "reason":if mutants.is_empty(){json!("no_matching_site")}else{Value::Null},"mutants":mutants})
    }).collect()
}

pub fn partial(want: &Value, actual: &Value) {
    match want {
        Value::Object(o) => {
            for (k, v) in o {
                partial(
                    v,
                    actual
                        .get(k)
                        .unwrap_or_else(|| panic!("missing {k}: {actual}")),
                );
            }
        }
        Value::Array(a) => {
            let b = actual.as_array().expect("array");
            assert_eq!(a.len(), b.len());
            for (x, y) in a.iter().zip(b) {
                partial(x, y)
            }
        }
        _ => assert_eq!(want, actual),
    }
}
pub fn hand(name: &str, v: &Value) {
    partial(&json!(hand_rows(name)), &v["mutations"]);
    if name == "mut-empty.broken-invalid" || name.starts_with("mut-gate") {
        response_boundary::assert_blocked(&v["mutations"]);
    } else {
        response_boundary::assert_not_applicable(&source(name), &v["mutations"]);
    }
    let expected: Value = serde_json::from_str(
        &std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../fixtures")
                .join(name)
                .join("expected.json"),
        )
        .unwrap(),
    )
    .unwrap();
    partial(&expected, v);
    let mut ids = BTreeSet::new();
    for row in v["mutations"].as_array().unwrap() {
        for m in row["mutants"].as_array().unwrap() {
            assert!(ids.insert(m["id"].as_str().unwrap()));
            let worsened = m["assigned_changes"]
                .as_array()
                .unwrap()
                .iter()
                .any(|c| c["direction"] == "worsened");
            if m["result"] == "detected" {
                assert!(
                    worsened
                        || row["kind"] == "summary_reused"
                            && m["assigned_changes"].as_array().unwrap().len() == 3
                            && m["assigned_changes"]
                                .as_array()
                                .unwrap()
                                .iter()
                                .all(|c| c["direction"] == "held")
                );
            }
            if m["result"] == "undetected" {
                assert!(!worsened);
            }
            if m["result"] == "not_run" {
                assert!(m["report"].is_null());
                assert_eq!(m["assigned_changes"], json!([]));
                assert_eq!(m["other_changes"], json!([]));
            }
        }
    }
    let mutant = |i: usize, fragment: &str| -> &Value {
        v["mutations"][i]["mutants"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["id"].as_str().unwrap().contains(fragment))
            .unwrap()
    };
    if name.starts_with("mut-evidence") {
        let original = v["original"]["agency"]["lineage"]
            .as_array()
            .unwrap()
            .iter()
            .find(|l| l["call"] == "po")
            .unwrap();
        assert_eq!(original["subject_sources"], json!(["Invoice"]));
        assert_eq!(
            original["independent_sources"],
            if name == "mut-evidence" {
                json!(["PurchaseOrder"])
            } else {
                json!([])
            }
        );
        let m = mutant(0, "Clerk.match_po");
        let line = m["report"]["agency"]["lineage"]
            .as_array()
            .unwrap()
            .iter()
            .find(|l| l["call"] == "po")
            .unwrap();
        assert_eq!(line["independent_sources"], json!([]));
        if name == "mut-evidence" {
            assert!(
                m["assigned_changes"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|c| c["before"] == json!(["PurchaseOrder"])
                        && c["after"] == json!([])
                        && c["direction"] == "worsened")
            );
        } else {
            assert_eq!(m["reason"], "no_worsening");
        }
    }
    if name == "mut-origin" {
        let m = mutant(9, "Approver.approve");
        partial(
            &json!({"status":"present","required_sources":["Invoice"],"direct":["Invoice"],"missing_sources":[]}),
            &v["original"]["responsibilities"][0]["observation"],
        );
        partial(
            &json!({"status":"missing","required_sources":["Invoice"],"direct":[],"indirect":["Invoice"],"missing_sources":["Invoice"]}),
            &m["report"]["responsibilities"][0]["observation"],
        );
        assert!(
            m["report"]["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["code"] == "W-RESP-1")
        );
    }
    if name == "mut-origin.broken-route-only" {
        let m = mutant(9, "Clerk.recheck");
        assert_eq!(m["reason"], "no_assigned_check");
        assert_eq!(m["report"]["contest_paths"], v["original"]["contest_paths"]);
        #[path = "support/agency_viewpoints_legacy.rs"]
        mod agency_legacy;
        assert_eq!(
            agency_legacy::agency(&m["report"]["agency"]),
            agency_legacy::agency(&v["original"]["agency"])
        );
    }
    if name.starts_with("mut-guard") {
        for (n, path) in ["body/2", "body/4"].iter().enumerate() {
            let hold = mutant(2, path);
            let refusal = mutant(8, path);
            if name == "mut-guard" {
                assert_eq!(v["original"]["properties"][n + 1]["verdict"], "satisfied");
                assert_eq!(hold["report"]["properties"][n + 1]["verdict"], "violated");
            } else {
                assert_eq!(hold["reason"], "no_assigned_check");
                assert!(!hold["other_changes"].as_array().unwrap().is_empty());
            }
            let binding = if n == 0 { "po" } else { "approval" };
            let refusal_report = &refusal["report"]["agency"]["refusal"][0];
            let j = refusal_report["judgements"]
                .as_array()
                .unwrap()
                .iter()
                .find(|j| j["call"] == binding)
                .unwrap();
            assert_eq!(j["stops"], "none");
        }
    }
}

#[test]
fn m_twelve_fixture_rows_all_288_sites_286_trials_two_not_run() {
    let (mut sites, mut trials, mut pending) = (0, 0, 0);
    for name in FIXTURES {
        let s = source(name);
        let before = serde_json::to_value(report::check(&s)).unwrap();
        let v = run(&s);
        hand(name, &v);
        assert_eq!(v["original"], before);
        assert_eq!(before, serde_json::to_value(report::check(&s)).unwrap());
        assert_eq!(v, run(&s));
        assert_eq!(
            v,
            serde_json::to_value(mutation::mutate_set(&s, "bpr-default")).unwrap()
        );
        // The historical 288/286/2 census remains the four ADR0020 operators;
        // C6's separate population owns the newly generated rows.
        for i in [0, 2, 8, 9] {
            let row = &v["mutations"][i];
            for m in row["mutants"].as_array().unwrap() {
                sites += 1;
                if m["result"] == "not_run" {
                    pending += 1
                } else {
                    trials += 1
                }
            }
        }
    }
    assert_eq!((sites, trials, pending), (288, 286, 2));
}

/// Independently written edits, also used by the reference population. Each
/// tuple corresponds to a declaration read or one guard, not to a call count.
pub fn hand_edits(name: &str) -> Vec<String> {
    if name == "mut-empty.broken-invalid" || name.starts_with("mut-gate") {
        return vec![];
    }
    let s = source(name);
    let rows = hand_rows(name);
    rows.iter()
        .enumerate()
        .filter(|(i, _)| [0, 2, 8, 9].contains(i))
        .map(|(_, r)| r)
        .flat_map(|r| r["mutants"].as_array().unwrap())
        .filter(|m| m["result"] != "not_run")
        .map(|m| {
            if m["operator"] == "bypass_guard" {
                let b = m["site"]["affected_bindings"][0].as_str().unwrap();
                let from = format!("if {b} == pass {{ }} else {{ end returned }}");
                assert_eq!(s.matches(&from).count(), 1);
                s.replacen(&from, "", 1)
            } else {
                let actor = m["site"]["actor"].as_str().unwrap();
                let src = m["site"]["source"].as_str().unwrap();
                let begin = s.find(&format!("actor {actor} {{")).unwrap();
                let at = begin + s[begin..].find("reads [").unwrap();
                let end = at + s[at..].find(']').unwrap() + 1;
                let old = &s[at..end];
                let list = &old[7..old.len() - 1];
                let kept: Vec<_> = list.split(',').filter(|x| x.trim() != src).collect();
                assert_eq!(kept.len() + 1, list.split(',').count());
                let new = if kept.is_empty() {
                    String::new()
                } else {
                    format!("reads [{}]", kept.join(","))
                };
                format!("{}{}{}", &s[..at], new, &s[end..])
            }
        })
        .collect()
}

pub fn boundary_inputs() -> Vec<(&'static str, String)> {
    let base = "model Boundary version \"0.2\" { type V = enum { pass,fail } type R = enum { ok } actor Gate { capacity 1; capability check() -> V { duration fixed 1min; result nondet { fail } } } actor Worker { capacity 1; capability run() -> R { duration fixed 3min; result nondet { ok } } } process Flow { gate = Gate.check(); if gate == pass { } else { end returned } work = Worker.run(); end done } cut Scope { include [Gate,Worker] } bounds { cases 1; live_tasks 2; clock 2min } verify safety no_resource_overcommit }";
    vec![
        ("truncate", base.into()),
        (
            "invalid_mutant",
            base.replace("{ fail }", "{ pass,fail }")
                .replace("clock 2min", "clock 10min")
                .replace(
                    "verify safety no_resource_overcommit",
                    "verify safety no_resource_overcommit verify reachable end.returned",
                ),
        ),
    ]
}

#[test]
fn m_public_truncated_and_invalid_mutants_do_not_become_detected() {
    for (name, s) in boundary_inputs() {
        assert_eq!(report::check(&s).verdict, relca_core::Verdict::Satisfied);
        let v = run(&s);
        assert_eq!(v["verdict"], "unsupported");
        assert_eq!(v["run"]["completion"], "inconclusive");
        for i in [2, 8] {
            let ms = v["mutations"][i]["mutants"].as_array().unwrap();
            assert_eq!(ms.len(), 1);
            assert_eq!(
                ms[0]["result"],
                if name == "truncate" {
                    "inconclusive"
                } else {
                    "invalid_mutant"
                }
            );
            assert_eq!(
                ms[0]["reason"],
                if name == "truncate" {
                    "truncated"
                } else {
                    "static_invalid"
                }
            );
        }
    }
}

#[test]
fn m_unknown_set_is_invalid_not_an_unsupported_kind_fold() {
    let v = serde_json::to_value(mutation::mutate_set("not a model", "other")).unwrap();
    assert_eq!(v["verdict"], "invalid");
    assert!(v["original"].is_null());
    assert_eq!(v["run"]["reason"], "invalid_arguments");
}

pub fn site_inputs() -> Vec<(&'static str, String)> {
    let model = "model Sites version \"0.2\" { type V = enum { pass,fail } actor S { source document } actor T { source record } actor A { capacity 4; capability check() -> V { reads [S,T]; duration fixed 1min; result nondet { pass,fail } } capability unused() -> V { reads [S]; duration fixed 1min; result nondet { pass,fail } } } process Flow { x = A.check(); fork f { y = A.check(); z = A.check() } join all f; if x == pass { t = A.check(); end early } else { u = A.check() } end done; late = A.check(); end tail } cut Scope { include [S,T,A] } bounds { cases 1; live_tasks 4; clock 20min } verify safety no_resource_overcommit }";
    vec![
        ("calls", model.into()),
        (
            "renamed",
            model
                .replace("A.check", "Z.review")
                .replace("actor A ", "actor Z ")
                .replace("capability check(", "capability review(")
                .replace("[S,T,A]", "[S,T,Z]")
                .replace("pass", "yes")
                .replace("fail", "no"),
        ),
    ]
}

#[test]
fn m_reads_are_declaration_sites_all_bindings_fork_else_dead_suffix_and_unused() {
    for (label, s) in site_inputs() {
        assert_eq!(report::check(&s).verdict, relca_core::Verdict::Satisfied);
        let v = run(&s);
        let actor = if label == "renamed" { "Z" } else { "A" };
        let cap = if label == "renamed" {
            "review"
        } else {
            "check"
        };
        for i in [0, 9] {
            let ms = v["mutations"][i]["mutants"].as_array().unwrap();
            assert_eq!(ms.len(), 3);
            for (j, src) in ["S", "T"].iter().enumerate() {
                assert_eq!(
                    ms[j]["id"],
                    format!("{}:{actor}.{cap}:reads:{src}", KINDS[i])
                );
                assert_eq!(
                    ms[j]["site"]["affected_bindings"],
                    json!(["x", "y", "z", "t", "u", "late"])
                );
                let origin = &ms[j]["report"]["agency"]["origin"];
                let row = origin
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|r| r["actor"] == actor)
                    .unwrap();
                assert_eq!(
                    row["direct"],
                    if j == 0 { json!(["T"]) } else { json!(["S"]) },
                    "other read retained; second edit starts from original"
                );
            }
            assert_eq!(ms[2]["id"], format!("{}:{actor}.unused:reads:S", KINDS[i]));
            assert_eq!(ms[2]["site"]["affected_bindings"], json!([]));
        }
        for i in [2, 8] {
            let ms = v["mutations"][i]["mutants"].as_array().unwrap();
            assert_eq!(ms.len(), 1);
            assert_eq!(ms[0]["id"], format!("{}:Flow:body/2:keep_else", KINDS[i]));
        }
    }
}

pub fn gate_inputs() -> Vec<(&'static str, String, &'static str)> {
    let guard = source("mut-guard");
    let trunc = boundary_inputs()[0]
        .1
        .replace("if gate == pass { } else { end returned }", "");
    vec![
        (
            "violated",
            guard.replace("if po == pass { } else { end returned }", ""),
            "violated",
        ),
        (
            "violated_cut",
            guard
                .replace("if po == pass { } else { end returned }", "")
                .replace("clock 20min", "clock 3min"),
            "violated",
        ),
        ("inconclusive", trunc, "inconclusive"),
        (
            "syntax",
            source("mut-empty").replace("end done", "end done // not a comment"),
            "unsupported",
        ),
        ("configuration", source("config-diff-parallel"), "invalid"),
    ]
}

pub fn nested_inputs() -> Vec<String> {
    let s = source("mut-guard.broken-no-contract");
    ["||","&&"].iter().map(|op|s.replace("if po == pass { } else { end returned }",&format!("if po == pass {op} po == fail {{ if po == pass {{ end returned }} else {{ }} }} else {{ end returned }}"))).collect()
}

pub fn scope_inputs() -> Vec<(&'static str, String, usize)> {
    let state = source("state-policy");
    vec![
        ("pure_data", state.clone(), 0),
        (
            "state_guard",
            state.replace(
                "if difficult_seen == true { h = Human.enter() } else { a = AI.extract() }",
                "if difficult_seen == true { end returned } else { }; h = Human.enter()",
            ),
            1,
        ),
        ("join_if_none", source("any-drain"), 0),
        (
            "unknown_enum",
            source("mut-empty").replace(
                "process Flow",
                "type Receipt = enum { unknown }; process Flow",
            ),
            0,
        ),
    ]
}

#[test]
fn m_scope_state_guard_pure_data_join_if_none_and_ordinary_unknown_value() {
    for (label, s, n) in scope_inputs() {
        assert_eq!(
            report::check(&s).verdict,
            relca_core::Verdict::Satisfied,
            "{label}"
        );
        let v = run(&s);
        for i in [2, 8] {
            assert_eq!(
                v["mutations"][i]["mutants"].as_array().unwrap().len(),
                n,
                "{label}"
            );
        }
        if n == 1 {
            for i in [2, 8] {
                let m = &v["mutations"][i]["mutants"][0];
                assert_eq!(m["site"]["affected_bindings"], json!([]));
                assert_eq!(m["site"]["retained_branch"], "else");
            }
        }
        response_boundary::assert_not_applicable(&s, &v["mutations"]);
    }
}

#[test]
fn m_utf8_crlf_tabs_source_coordinates_and_stable_ids() {
    let original = source("mut-evidence");
    let shifted=original.replace("cut Scope { include [Invoice,PurchaseOrder,Reader,Clerk,Ledger] }", "")
        .replace("actor Clerk {", "\tcut Scope { question \"日本語\"; include [Invoice,PurchaseOrder,Reader,Clerk,Ledger] } actor Clerk {")
        .replace('\n',"\r\n");
    assert_eq!(
        report::check(&shifted).verdict,
        relca_core::Verdict::Satisfied
    );
    let (base, v) = (run(&original), run(&shifted));
    for i in [0, 9] {
        let ms = v["mutations"][i]["mutants"].as_array().unwrap();
        for (j, m) in ms.iter().enumerate() {
            assert_eq!(m["id"], base["mutations"][i]["mutants"][j]["id"]);
            let at = m["site"]["span"]["start"].as_u64().unwrap() as usize;
            let prefix = &shifted[..at];
            assert_eq!(
                m["site"]["line"].as_u64().unwrap() as usize,
                prefix.bytes().filter(|b| *b == b'\n').count() + 1
            );
            assert_eq!(
                m["site"]["column"].as_u64().unwrap() as usize,
                prefix.rsplit('\n').next().unwrap().chars().count() + 1
            );
        }
    }
}

#[test]
fn m_nested_then_else_both_directions_and_or_and_have_exact_guard_sites() {
    for s in nested_inputs() {
        assert_eq!(report::check(&s).verdict, relca_core::Verdict::Satisfied);
        let v = run(&s);
        for i in [2, 8] {
            let actual: Vec<_> = v["mutations"][i]["mutants"]
                .as_array()
                .unwrap()
                .iter()
                .map(|m| m["id"].as_str().unwrap().to_owned())
                .collect();
            let want: Vec<_> = [
                "body/2:keep_then",
                "body/2:keep_else",
                "body/2/then/0:keep_else",
                "body/4:keep_then",
            ]
            .iter()
            .map(|p| format!("{}:Flow:{p}", KINDS[i]))
            .collect();
            assert_eq!(
                actual, want,
                "source DFS includes dead terminal branches and both directions"
            );
        }
    }
}

#[test]
fn m_public_gate_violated_proceeds_undecidable_and_configuration_stop() {
    for (label, s, want) in gate_inputs() {
        let before = serde_json::to_value(report::check(&s)).unwrap();
        assert_eq!(before["verdict"], want, "{label}");
        let v = run(&s);
        assert_eq!(v["original"], before);
        if want == "violated" {
            assert_eq!(v["verdict"], "unsupported");
            assert_ne!(v["run"]["completion"], "blocked");
            if label == "violated_cut" {
                assert_eq!(v["original"]["bounds"]["truncated"], true);
                for i in [2, 8] {
                    for m in v["mutations"][i]["mutants"].as_array().unwrap() {
                        assert_eq!(m["result"], "inconclusive");
                    }
                }
            }
        } else {
            assert_eq!(v["verdict"], want);
            assert_eq!(v["run"]["completion"], "blocked");
            for i in [0, 2, 8, 9] {
                assert_eq!(v["mutations"][i]["status"], "blocked");
                assert_eq!(v["mutations"][i]["mutants"], json!([]));
            }
        }
    }
}
