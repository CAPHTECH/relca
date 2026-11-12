//! ADR 0021 independent acceptance: only public explain and report entry points.
use relca_core::{explain, report};
use serde_json::{Value, json};
#[path = "../../relca-cli/tests/support/ledger_shape.rs"]
mod ledger_shape;
use std::path::PathBuf;
#[path = "support/responsibility_defaults_oracle.rs"]
mod responsibility_oracle;

const NAMES: [&str; 12] = [
    "explain-deadlock",
    "explain-deadlock.broken-zero-capacity",
    "explain-executors",
    "explain-executors.broken-shared-worker",
    "explain-invariant",
    "explain-invariant.broken-cut-guard",
    "explain-once",
    "explain-once.broken-repeat-result",
    "explain-requires",
    "explain-requires.broken-cut-guard",
    "explain-response",
    "explain-response.broken-late-finish",
];
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}
fn source(name: &str) -> String {
    std::fs::read_to_string(root().join(name).join("model.relca")).unwrap()
}
fn explain(src: &str) -> Value {
    serde_json::to_value(explain::explain(src)).unwrap()
}

// Arrays are exact, ordered and exhaustive, not subset/keyed matching.
fn matches(e: &Value, a: &Value) {
    match e {
        Value::Object(o) => {
            for (k, v) in o {
                matches(v, a.get(k).unwrap_or_else(|| panic!("missing {k}")));
            }
        }
        Value::Array(es) => {
            let as_ = a.as_array().unwrap();
            assert_eq!(es.len(), as_.len());
            for (x, y) in es.iter().zip(as_) {
                matches(x, y);
            }
        }
        _ => assert_eq!(e, a),
    }
}
fn location(src: &str, start: usize, end: usize) -> Value {
    fn pos(s: &str) -> (usize, usize) {
        (
            s.bytes().filter(|&b| b == b'\n').count() + 1,
            s.rsplit('\n').next().unwrap().chars().count() + 1,
        )
    }
    let (line, column) = pos(&src[..start]);
    let (end_line, end_column) = pos(&src[..end]);
    json!({"span":{"start":start,"end":end},"line":line,"column":column,"end_line":end_line,"end_column":end_column})
}
fn fragment(src: &str, text: &str) -> Value {
    let start = src.find(text).unwrap();
    location(src, start, start + text.len())
}
fn check_locations(src: &str, v: &Value) {
    match v {
        Value::Object(o) => {
            if o.contains_key("end_column") {
                let start = v["span"]["start"].as_u64().unwrap() as usize;
                let end = v["span"]["end"].as_u64().unwrap() as usize;
                assert!(
                    start <= end
                        && end <= src.len()
                        && src.is_char_boundary(start)
                        && src.is_char_boundary(end)
                );
                assert_eq!(*v, location(src, start, end));
            }
            for x in o.values() {
                check_locations(src, x);
            }
        }
        Value::Array(a) => {
            for x in a {
                check_locations(src, x);
            }
        }
        _ => {}
    }
}

#[test]
fn twelve_hand_paths_and_first_violation_locations() {
    let mut count = 0;
    for name in NAMES {
        let src = source(name);
        let e: Value = serde_json::from_str(
            &std::fs::read_to_string(root().join(name).join("expected.json")).unwrap(),
        )
        .unwrap();
        let v = explain(&src);
        let model = relca_core::lower::lower(&relca_core::syntax::parse(&src).unwrap()).unwrap();
        responsibility_oracle::assert_report(&model, &v);
        matches(&responsibility_oracle::with_diagnostics(&model, &e), &v);
        check_locations(&src, &v["explanation"]);
        for c in v["explanation"]["counterexamples"].as_array().unwrap() {
            count += 1;
            let id = c["id"].as_str().unwrap();
            if id.starts_with("contract ") {
                let kind = id.split_whitespace().last().unwrap();
                let clause = src
                    .lines()
                    .find_map(|line| {
                        let line = line.trim();
                        if line.starts_with(kind) {
                            Some(line.to_string())
                        } else if line.starts_with("contract Controls { ") {
                            Some(
                                line.trim_start_matches("contract Controls { ")
                                    .trim_end_matches(" }")
                                    .to_string(),
                            )
                        } else {
                            None
                        }
                    })
                    .unwrap();
                assert_eq!(c["property_location"], fragment(&src, &clause));
            } else {
                assert!(c["property_location"].is_null());
            }
            for s in c["steps"].as_array().unwrap() {
                if let Some(binding) = s["binding"].as_str()
                    && ["start", "finish", "cancel_ack"].contains(&s["kind"].as_str().unwrap())
                {
                    let call = match binding {
                        "approval" => "approval = Approver.check()",
                        "posted" => "posted = Ledger.post()",
                        "gate" => "gate = Gate.check()",
                        "entry" => "entry = Clerk.enter()",
                        "y" => "y = Ledger.post()",
                        "x" if name.starts_with("explain-once") => "x = Ledger.post()",
                        "x" => "x = Worker.run()",
                        _ => panic!("unexpected binding {binding}"),
                    };
                    assert_eq!(s["location"], fragment(&src, call));
                    let (actor, capability) = call
                        .split(" = ")
                        .nth(1)
                        .unwrap()
                        .trim_end_matches("()")
                        .split_once('.')
                        .unwrap();
                    assert_eq!(s["actor"], actor);
                    assert_eq!(s["capability"], capability);
                    assert_eq!(s["case_id"], 1);
                    let expected_executor = if name.starts_with("explain-executors") {
                        json!("p")
                    } else {
                        Value::Null
                    };
                    assert_eq!(s["executor"], expected_executor);
                    if s["kind"] == "start" {
                        assert_eq!(s["result"], Value::Null);
                    }
                    assert!(!s["capability_location"].is_null());
                    let cap_start = src.find(&format!("capability {capability}(")).unwrap();
                    let body = cap_start + src[cap_start..].find('{').unwrap();
                    let mut depth = 0;
                    let mut cap_end = None;
                    for (i, ch) in src[body..].char_indices() {
                        if ch == '{' {
                            depth += 1;
                        }
                        if ch == '}' {
                            depth -= 1;
                            if depth == 0 {
                                cap_end = Some(body + i + 1);
                                break;
                            }
                        }
                    }
                    let cap_end = cap_end.unwrap();
                    assert_eq!(s["capability_location"], location(&src, cap_start, cap_end));
                    let holders = if s["kind"] == "start" {
                        json!([{"case_id":1,"binding":binding,"executor":expected_executor}])
                    } else {
                        json!([])
                    };
                    let capacity = if actor == "Worker" && name.starts_with("explain-executors") {
                        2
                    } else {
                        1
                    };
                    assert_eq!(
                        s["resource"],
                        json!({"actor":actor,"capacity":capacity,"anonymous":!name.starts_with("explain-executors"),"holders":holders})
                    );
                }
                if s["kind"] == "finish" {
                    let events = s["events"].as_array().unwrap();
                    let kinds: Vec<_> =
                        events.iter().map(|e| e["kind"].as_str().unwrap()).collect();
                    let expected = if s["actor"] == "Ledger" {
                        vec!["finish", "commit"]
                    } else {
                        vec!["finish"]
                    };
                    assert_eq!(kinds, expected);
                    for event in events {
                        for key in [
                            "case_id",
                            "binding",
                            "actor",
                            "capability",
                            "executor",
                            "result",
                            "location",
                        ] {
                            assert_eq!(event[key], s[key], "{name}/{key}");
                        }
                    }
                }
                if ["queue", "advance", "join"].contains(&s["kind"].as_str().unwrap()) {
                    assert!(s["resource"].is_null());
                }
                if ["initial", "advance", "join", "branch", "deadlock"]
                    .contains(&s["kind"].as_str().unwrap())
                {
                    for key in [
                        "binding",
                        "actor",
                        "capability",
                        "executor",
                        "result",
                        "capability_location",
                    ] {
                        assert!(s[key].is_null(), "{name}/{key}");
                    }
                }
                assert_eq!(s["values"], json!({"observed":[],"latent":[]}));
            }
        }
    }
    assert_eq!(
        count, 8,
        "five broken single paths plus three executor paths"
    );
}

fn branch_source(order: &str, long: bool) -> String {
    format!(
        r#"model M version "0.2" {{
  type T = enum {{ left, right }}
  type R = enum {{ ok }}
  observed case {{ good: bool = false }}
  actor Gate {{ capacity 1; capability run() -> T {{ duration fixed 1min; result nondet {{ {order} }} }} }}
  actor Extra {{ capacity 1; capability run() -> R {{ duration fixed 1min; result nondet {{ ok }} }} }}
  actor Bad {{ capacity 1; capability run() -> R {{ duration fixed 1min; result nondet {{ ok }} }} }}
  process Flow {{
    choice = Gate.run()
    if choice == left {{ short = Bad.run() }} else {{ {} other = Bad.run() }}
    end done
  }}
  bounds {{ cases 1; live_tasks 2; clock 10min }}
  cut Scope {{ include [Gate, Extra, Bad] }}
  contract Controls {{ invariant Bad.run.finish -> good == true }}
}}"#,
        if long { "extra = Extra.run();" } else { "" }
    )
}

#[test]
fn shortest_not_first_dfs_parent_and_ties_use_generation_order() {
    for (order, long, binding, result) in [
        ("left, right", true, "short", "left"),
        ("left, right", false, "short", "left"),
        ("right, left", false, "other", "right"),
    ] {
        let src = branch_source(order, long);
        let a = explain(&src);
        let b = explain(&src);
        assert_eq!(
            serde_json::to_vec(&a).unwrap(),
            serde_json::to_vec(&b).unwrap()
        );
        let c = &a["explanation"]["counterexamples"][0];
        assert_eq!(c["transition_count"], 10);
        assert_eq!(c["steps"][4]["result"], result);
        assert_eq!(c["steps"][10]["binding"], binding);
        assert_eq!(c["replay"]["status"], "verified");
    }
}

#[test]
fn span_unicode_tabs_and_crlf_are_source_based_not_byte_columns() {
    let src = "model M version \"0.2\" {\n  type T = enum { ok }\n  actor A {\n    capacity 1\n    capability run() -> T {\n      duration fixed 1min\n      result nondet { ok }\n    }\n  }\n  process P {\n    x = A.run()\n    end done\n  }\n  observed case { good: bool = false }\n  bounds { cases 1; live_tasks 2; clock 10min }\n  cut Scope { question \"a\"; include [A] }\n  contract Controls { invariant A.run.finish -> good == true }\n}\n";
    assert_eq!(
        fragment(src, "x = A.run()"),
        json!({"span":{"start":183,"end":194},"line":11,"column":5,"end_line":11,"end_column":16})
    );
    let ascii = src
        .replace("  cut Scope { question \"a\"; include [A] }\n", "")
        .replace(
            "  process P {",
            "  cut Scope { question \"a\"; include [A] }\n  process P {",
        );
    let unicode = ascii.replace("question \"a\"", "question \"あ\"");
    assert_eq!(
        unicode.find("x = A.run()").unwrap(),
        ascii.find("x = A.run()").unwrap() + 2
    );
    let inline_ascii = src
        .replace("  cut Scope { question \"a\"; include [A] }\n", "")
        .replace(
            "  process P {\n    x = A.run()\n    end done\n  }",
            "  cut Scope { question \"a\"; include [A] }; process P { x = A.run(); end done }",
        );
    let inline_unicode = inline_ascii.replace("question \"a\"", "question \"あ\"");
    for text in [
        src.to_string(),
        ascii,
        unicode,
        inline_ascii,
        inline_unicode,
        src.replace("    x =", "\tx ="),
        src.replace('\n', "\r\n"),
    ] {
        // Unicode belongs in a descriptive string, not the language version.
        let v = explain(&text);
        assert_eq!(v["verdict"], "violated");
        let s = &v["explanation"]["counterexamples"][0]["steps"][4];
        assert_eq!(s["location"], fragment(&text, "x = A.run()"));
        check_locations(&text, &v["explanation"]);
    }
}

#[test]
fn latent_values_are_analyst_evidence_not_observations() {
    let src=source("explain-response").replace("  process Flow {","  latent case { truth: bool = false }\n  process Flow {").replace("response case.received -> one_of(end.done) by 5min after case.received otherwise unresolved","invariant Worker.run.finish -> truth == true");
    let v = explain(&src);
    let c = &v["explanation"]["counterexamples"][0];
    assert_eq!(c["transition_count"], 4);
    for i in [0, 4] {
        let values = &c["steps"][i]["values"];
        assert_eq!(values["observed"], json!([]));
        matches(
            &json!([{"name":"truth","value":"false","visibility":"latent"}]),
            &values["latent"],
        );
    }
    let good = explain(&src.replace("truth: bool = false", "truth: bool = true"));
    assert_eq!(good["verdict"], "satisfied");
    assert_eq!(good["explanation"]["counterexamples"], json!([]));
}

#[test]
fn root_and_exit_contract_survive_unavailable_zero_duration_replay() {
    let src = source("explain-invariant.broken-cut-guard").replace("fixed 1min", "fixed 0min");
    let mut v = explain(&src);
    assert_eq!(v["verdict"], "violated");
    let e = v.as_object_mut().unwrap().remove("explanation").unwrap();
    assert_eq!(
        ledger_shape::original_report(&v),
        ledger_shape::original_report(&serde_json::to_value(report::check(&src)).unwrap())
    );
    assert_eq!(e["status"], "unsupported");
    assert_eq!(
        e["counterexamples"][0]["reason"],
        "zero_duration_reference_unavailable"
    );
    assert_eq!(e["counterexamples"][0]["steps"], json!([]));
    assert_eq!(e["counterexamples"][0]["transition_count"], Value::Null);
    assert_eq!(e["diagnostics"][0]["code"], "U-SEM-0");
    let unused=source("explain-invariant.broken-cut-guard").replace("  process Flow {","  actor Unused { capacity 1; capability zero() -> Ack { duration fixed 0min; result nondet { ok } } }\n  process Flow {").replace("include [Approver, Ledger]","include [Approver, Ledger, Unused]");
    assert_eq!(
        explain(&unused)["explanation"]["counterexamples"][0]["status"],
        "verified"
    );
}

#[test]
fn response_inclusive_end_deadlock_and_clock_boundaries() {
    let src = source("explain-response");
    let exact = explain(&src.replace("fixed 4min", "fixed 5min"));
    assert_eq!(exact["verdict"], "satisfied");
    assert_eq!(exact["explanation"]["counterexamples"], json!([]));
    for s in [
        src.replace("end done", "if x == ok { end returned } else { end done }"),
        src.replace("capacity 1", "capacity 0"),
    ] {
        let v = explain(&s);
        assert_eq!(v["verdict"], "violated");
        assert!(
            v["explanation"]["counterexamples"]
                .as_array()
                .unwrap()
                .iter()
                .any(|c| c["id"] == "contract Controls #1 response")
        );
    }
    let late = src.replace("fixed 4min", "fixed 6min");
    let before = explain(&late.replace("clock 10min", "clock 4min"));
    assert_eq!(before["verdict"], "inconclusive");
    assert_eq!(before["explanation"]["counterexamples"], json!([]));
    let at = explain(&late.replace("clock 10min", "clock 5min"));
    assert_eq!(at["verdict"], "violated");
    assert_eq!(at["explanation"]["shortest_scope"], "explored_graph");
    assert_eq!(
        at["explanation"]["counterexamples"][0]["steps"]
            .as_array()
            .unwrap()
            .last()
            .unwrap()["kind"],
        "deadline"
    );
}

#[test]
fn static_failures_are_blocked_without_changing_check() {
    for src in [
        "not a model".to_string(),
        source("explain-response").replace("Worker.run()", "Missing.run()"),
        source("explain-executors").replace("people [p,q]", "people [p]"),
        source("explain-response").replace(
            "  process Flow {",
            "  latent case { truth: bool = false }\n  process Flow { if truth == true { } else { }",
        ),
    ] {
        let mut v = explain(&src);
        let e = v.as_object_mut().unwrap().remove("explanation").unwrap();
        assert_eq!(
            ledger_shape::original_report(&v),
            ledger_shape::original_report(&serde_json::to_value(report::check(&src)).unwrap())
        );
        assert_eq!(e["status"], "blocked");
        assert_eq!(e["counterexamples"], json!([]));
    }
}

#[test]
fn known_clock_cut_preserves_a_short_violation_from_another_branch() {
    let src = branch_source("left, right", true).replace(
        "actor Extra { capacity 1; capability run() -> R { duration fixed 1min",
        "actor Extra { capacity 1; capability run() -> R { duration fixed 20min",
    );
    let v = explain(&src);
    assert_eq!(v["verdict"], "violated");
    assert_eq!(v["bounds"]["truncated"], true);
    let e = &v["explanation"];
    assert_eq!(e["shortest_scope"], "explored_graph");
    assert_eq!(e["counterexamples"][0]["transition_count"], 10);
    assert_eq!(e["counterexamples"][0]["steps"][10]["binding"], "short");
    assert_eq!(e["counterexamples"][0]["replay"]["status"], "verified");
}

#[test]
fn arrivals_keep_global_prefix_and_choose_first_case_on_equal_distance() {
    let src=source("explain-executors.broken-shared-worker")
        .replace("    entry = Clerk.enter()\n", "")
        .replace("  bounds { cases 1; live_tasks 2; clock 10min }", "  arrivals fixed [0min, 0min]\n  policy fifo tie_by case_id\n  bounds { cases 2; live_tasks 2; clock 10min }")
        .replace("executors [p,q]", "executors [p]")
        .replace("    separation Clerk.enter.finish, Worker.run.finish\n", "");
    let v = explain(&src);
    let c = &v["explanation"]["counterexamples"][0];
    assert_eq!(c["id"], "contract Controls #1 permitted");
    assert_eq!(c["case_id"], 1);
    assert_eq!(c["replay"]["status"], "verified");
    let steps = c["steps"].as_array().unwrap();
    let last = steps.last().unwrap();
    assert_eq!(last["kind"], "start");
    assert_eq!(last["time"], 0);
    assert_eq!(last["executor"], "p");
    assert_eq!(last["location"], fragment(&src, "x = Worker.run()"));
    let received: Vec<_> = steps
        .iter()
        .flat_map(|s| s["events"].as_array().unwrap())
        .filter(|e| e["kind"] == "received" || e["kind"] == "receive")
        .map(|e| e["case_id"].clone())
        .collect();
    assert_eq!(received, json!([1, 2]).as_array().unwrap().clone());
    check_locations(&src, &v["explanation"]);
}

#[test]
fn arrivals_case_two_keeps_case_one_resource_release_in_global_prefix() {
    let src = source("arrivals-contracts.broken-short-response");
    let v = explain(&src);
    let c = &v["explanation"]["counterexamples"][0];
    assert_eq!(c["case_id"], 2);
    assert_eq!(c["id"], "contract Controls #3 response");
    assert_eq!(c["replay"]["status"], "verified");
    let events: Vec<_> = c["steps"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|s| s["events"].as_array().unwrap())
        .filter(|e| ["start", "finish"].contains(&e["kind"].as_str().unwrap_or("")))
        .collect();
    let before_second = events
        .iter()
        .position(|e| e["kind"] == "start" && e["case_id"] == 2)
        .unwrap();
    assert!(
        events[..before_second]
            .iter()
            .any(|e| e["kind"] == "start" && e["case_id"] == 1)
    );
    assert!(
        events[..before_second]
            .iter()
            .any(|e| e["kind"] == "finish" && e["case_id"] == 1)
    );
    for event in events {
        assert_eq!(event["location"], fragment(&src, "r = A.run()"));
    }
    check_locations(&src, &v["explanation"]);
}

#[test]
fn same_task_finish_and_commit_are_one_violating_bundle() {
    let src = source("explain-executors")
        .replace(
            "capability run() -> T { duration",
            "capability run() -> T { commits; duration",
        )
        .replace(
            "separation Clerk.enter.finish, Worker.run.finish",
            "separation Worker.run.finish, Worker.run.commit",
        );
    let v = explain(&src);
    let c = &v["explanation"]["counterexamples"][0];
    assert_eq!(c["id"], "contract Controls #1 separation");
    assert_eq!(c["transition_count"], 9);
    let last = c["steps"].as_array().unwrap().last().unwrap();
    assert_eq!(last["kind"], "finish");
    assert_eq!(last["executor"], "q");
    let events = last["events"].as_array().unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0]["kind"], "finish");
    assert_eq!(events[1]["kind"], "commit");
    assert_eq!(events[0]["executor"], events[1]["executor"]);
}

#[test]
fn positive_duration_support_path_has_real_finish_and_no_future_result_on_start() {
    let src = source("duration-parallel")
        .replace("3min:0.5,5min:0.5", "1min:0.5,2min:0.5")
        .replace(
            "  verify safety",
            "  contract Controls { invariant A.left.finish -> leftDone == false }\n  verify safety",
        );
    let v = explain(&src);
    assert_eq!(v["verdict"], "violated");
    let c = &v["explanation"]["counterexamples"][0];
    assert_eq!(c["replay"]["status"], "verified");
    let steps = c["steps"].as_array().unwrap();
    let last = steps.last().unwrap();
    assert_eq!(last["kind"], "finish");
    assert_eq!(last["binding"], "l");
    assert_eq!(last["result"], "ok");
    assert_eq!(last["location"], fragment(&src, "l = A.left()"));
    for s in steps {
        if s["kind"] == "start" {
            assert!(s["result"].is_null());
        }
    }
    assert!(steps.iter().any(|s| s["kind"] == "duration_gate"));
    assert!(
        v["explanation"]["omitted"]
            .as_array()
            .unwrap()
            .iter()
            .any(|o| o["reason"] == "probabilistic_property")
    );
    check_locations(&src, &v["explanation"]);
}

#[test]
fn cancel_ack_is_not_finish_commit_or_result_publication() {
    // A Slow.start-triggered obligation selects an actually started, cancelled
    // task, rather than inventing a finish for an unstarted loser.
    let src=source("exec-cancel").replace("  contract Controls {", "  contract Controls {\n    response Slow.work.start -> one_of(Slow.work.finish) by 10min after Slow.work.start otherwise unresolved");
    let v = explain(&src);
    let c = v["explanation"]["counterexamples"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "contract Controls #1 response")
        .unwrap();
    assert_eq!(c["replay"]["status"], "verified");
    let steps = c["steps"].as_array().unwrap();
    let ack = steps
        .iter()
        .find(|s| s["kind"] == "cancel_ack" && s["binding"] == "slow")
        .unwrap();
    assert_eq!(ack["time"], 2);
    assert_eq!(ack["result"], Value::Null);
    assert_eq!(ack["location"], fragment(&src, "slow = Slow.work()"));
    assert!(
        !steps
            .iter()
            .flat_map(|s| s["events"].as_array().unwrap())
            .any(|e| e["binding"] == "slow" && (e["kind"] == "finish" || e["kind"] == "commit"))
    );
    check_locations(&src, &v["explanation"]);
    // Original satisfied/vacuous pair and drain variant; no phase count oracle.
    for name in ["exec-cancel", "exec-cancel.broken-ignore-cancel"] {
        let src = source(name);
        let v = explain(&src);
        if name == "exec-cancel" {
            assert_eq!(v["explanation"]["counterexamples"], json!([]));
        } else {
            let cs = v["explanation"]["counterexamples"].as_array().unwrap();
            assert_eq!(cs.len(), 2);
            for c in cs {
                assert_eq!(c["replay"]["status"], "verified");
                let steps = c["steps"].as_array().unwrap();
                assert!(!steps.iter().any(|s| s["kind"] == "cancel_ack"));
                let slow = steps
                    .iter()
                    .find(|s| s["kind"] == "finish" && s["binding"] == "slow")
                    .unwrap();
                assert_eq!(slow["time"], 5);
                assert_eq!(slow["result"], "ok");
                assert_eq!(slow["location"], fragment(&src, "slow = Slow.work()"));
            }
        }
        check_locations(&src, &v["explanation"]);
    }
}
