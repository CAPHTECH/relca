//! Hand-authored inputs/expectations for ADR 0057, not captured CLI output.
use serde_json::{Value, json};

#[derive(Clone, Debug, PartialEq)]
pub struct Expected {
    pub conformance: &'static str,
    pub verdict: &'static str,
    pub exit: i32,
    pub code: Option<&'static str>,
    pub reason: Option<&'static str>,
    pub unmapped: Option<&'static str>,
    pub evidence: &'static [&'static str],
    pub limit: Option<&'static str>,
}
pub fn ok() -> Expected {
    Expected {
        conformance: "conformant",
        verdict: "satisfied",
        exit: 0,
        code: None,
        reason: None,
        unmapped: None,
        evidence: &[],
        limit: None,
    }
}
pub fn unknown(code: &'static str, reason: &'static str) -> Expected {
    Expected {
        conformance: "unknown",
        verdict: "inconclusive",
        exit: 2,
        code: Some(code),
        reason: Some(reason),
        ..ok()
    }
}
pub fn violation(reason: &'static str, evidence: &'static [&'static str]) -> Expected {
    Expected {
        conformance: "violating",
        verdict: "violated",
        exit: 1,
        code: Some("E-LOG-2"),
        reason: Some(reason),
        evidence,
        ..ok()
    }
}
fn invalid(code: &'static str) -> Expected {
    Expected {
        conformance: "unknown",
        verdict: "invalid",
        exit: 3,
        code: Some(code),
        ..ok()
    }
}
fn unsupported(code: &'static str) -> Expected {
    Expected {
        conformance: "unknown",
        verdict: "unsupported",
        exit: 4,
        code: Some(code),
        ..ok()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Edit {
    Replace(&'static str, Value),
    RemoveEvent(&'static str),
    RemoveTask(&'static str),
    AppendTask(Vec<Value>),
    ReplaceTask {
        task: &'static str,
        events: Vec<Value>,
    },
    MoveTask {
        task: &'static str,
        case: &'static str,
    },
    RenameTask {
        task: &'static str,
        to: &'static str,
    },
    AssignTask {
        task: &'static str,
        to: &'static str,
    },
    ActorTask {
        task: &'static str,
        to: &'static str,
    },
    Contract(&'static str),
    Model(&'static str, &'static str),
    RawLine(usize, String),
    Args(Vec<&'static str>),
    Scope(Option<&'static str>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Case {
    pub name: &'static str,
    pub model: String,
    pub records: Value,
    pub raw_line: Option<(usize, String)>,
    pub extra_args: Vec<&'static str>,
    pub scope: Option<&'static str>,
    pub expected: Expected,
    pub parent: Option<&'static str>,
    pub edit: Option<Edit>,
}
impl Case {
    pub fn log(&self) -> String {
        self.records
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
            .map(|(i, row)| {
                let line = self
                    .raw_line
                    .as_ref()
                    .filter(|(n, _)| *n == i)
                    .map(|(_, s)| s.clone())
                    .unwrap_or_else(|| row.to_string());
                format!("{line}\n")
            })
            .collect()
    }
    pub fn apply(&mut self, edit: &Edit) {
        match edit {
            Edit::Replace(path, value) => {
                *self
                    .records
                    .pointer_mut(path)
                    .unwrap_or_else(|| panic!("bad edit: {path}")) = value.clone();
            }
            Edit::RemoveEvent(id) => self
                .records
                .as_array_mut()
                .unwrap()
                .retain(|r| r["event_id"] != *id),
            Edit::RemoveTask(task) => {
                let removed: Vec<String> = self
                    .records
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|r| r["task_id"] == *task)
                    .map(|r| r["event_id"].as_str().unwrap().to_owned())
                    .collect();
                self.records
                    .as_array_mut()
                    .unwrap()
                    .retain(|r| r["task_id"] != *task);
                for r in self.records.as_array_mut().unwrap().iter_mut().skip(1) {
                    r["after"]
                        .as_array_mut()
                        .unwrap()
                        .retain(|id| !removed.iter().any(|s| id == s));
                }
            }
            Edit::AppendTask(rows) => self.records.as_array_mut().unwrap().extend(rows.clone()),
            Edit::ReplaceTask { task, events } => {
                self.records
                    .as_array_mut()
                    .unwrap()
                    .retain(|r| r["task_id"] != *task);
                self.records.as_array_mut().unwrap().extend(events.clone());
            }
            Edit::MoveTask { task, case } => {
                self.records[0]["cases"]
                    .as_array_mut()
                    .unwrap()
                    .push(scope(case));
                for r in self.records.as_array_mut().unwrap().iter_mut().skip(1) {
                    if r["task_id"] == *task {
                        r["case_id"] = json!(case);
                    }
                }
            }
            Edit::RenameTask { task, to } => {
                for r in self.records.as_array_mut().unwrap().iter_mut().skip(1) {
                    if r["task_id"] == *task {
                        r["task_id"] = json!(to);
                    }
                }
            }
            Edit::AssignTask { task, to } => {
                for r in self.records.as_array_mut().unwrap().iter_mut().skip(1) {
                    if r["task_id"] == *task {
                        r["executor_id"] = json!(to);
                    }
                }
            }
            Edit::ActorTask { task, to } => {
                for r in self.records.as_array_mut().unwrap().iter_mut().skip(1) {
                    if r["task_id"] == *task {
                        r["actor_id"] = json!(to);
                    }
                }
            }
            Edit::Contract(clause) => {
                self.model = self.model.replacen(
                    "bounds {",
                    &format!("contract Extra {{ {clause} }}\nbounds {{"),
                    1,
                );
            }
            Edit::Model(from, to) => {
                assert!(self.model.contains(from));
                self.model = self.model.replacen(from, to, 1);
            }
            Edit::RawLine(i, line) => self.raw_line = Some((*i, line.clone())),
            Edit::Args(args) => self.extra_args = args.clone(),
            Edit::Scope(scope) => self.scope = *scope,
        }
    }
}

pub fn model(contract: &str) -> String {
    format!(
        r#"model M version "0.2" {{
type Verdict = enum {{ pass, fail }}
type Ack = enum {{ committed }}
people [p,q]
actor Clerk {{ executors [p,q]; capability enter() -> Verdict {{ duration fixed 1min; result nondet {{ pass, fail }} }} }}
actor Approver {{ executors [p,q]; capability approve() -> Verdict {{ duration fixed 1min; result nondet {{ pass, fail }} }} }}
actor Poster {{ executors [p,q]; capability post() -> Ack {{ commits; duration fixed 1min; result nondet {{ committed }} }} }}
process Flow {{ e = Clerk.enter(); a = Approver.approve(); z = Poster.post(); end done }}
cut C {{ include [Clerk,Approver,Poster] }}
{contract}
arrivals fixed [0min,0min]
bounds {{ cases 2; live_tasks 2; clock 20000min }}
}}"#
    )
}
pub fn scope(id: &str) -> Value {
    json!({"case_id": id, "from_case_start": true, "through_case_end": true})
}
fn header() -> Value {
    json!({"kind":"header", "schema":"relca-log/v1", "clock":"shared-test-clock",
        "time_unit":"min", "window":{"from":0,"to":20000}, "cases":[scope("c")],
        "actor_map":[{"source_id":"clerk","actor":"Clerk"},
                     {"source_id":"approver","actor":"Approver"},
                     {"source_id":"poster","actor":"Poster"}],
        "executor_map":[{"source_id":"person-p","person":"p"},
                        {"source_id":"person-q","person":"q"}]})
}
pub fn event(task: &str, op: &str, lifecycle: &str, at: u64, who: &str) -> Value {
    let actor = match op.split('.').next().unwrap() {
        "Clerk" => "clerk",
        "Approver" => "approver",
        "Poster" => "poster",
        _ => unreachable!(),
    };
    json!({"kind":"event", "event_id":format!("{task}-{lifecycle}"), "case_id":"c", "task_id":task,
        "operation":op, "lifecycle":lifecycle, "cancel_phase":null,
        "time":{"earliest":at,"latest":at}, "after":[], "actor_id":actor,
        "executor_id":who,"input_version":"input-v1",
        "output_version":if lifecycle == "finish" {json!("output-v1")} else {Value::Null},
        "result":if lifecycle == "finish" {
            json!(if op == "Poster.post" {"committed"} else {"pass"})
        } else {Value::Null}})
}
fn task(id: &str, op: &str, start: u64, who: &str) -> Vec<Value> {
    vec![
        event(id, op, "start", start, who),
        event(id, op, "finish", start + 1, who),
    ]
}
fn base(name: &'static str, contract: &str, events: Vec<Value>) -> Case {
    let mut records = vec![header()];
    records.extend(events);
    Case {
        name,
        model: model(if contract.is_empty() {
            "contract ScopeOnly { excludes Approver.approve.start, Poster.post.start }"
        } else {
            contract
        }),
        records: json!(records),
        raw_line: None,
        extra_args: vec![],
        scope: Some("event-contracts"),
        expected: ok(),
        parent: None,
        edit: None,
    }
}
fn changed(name: &'static str, parent: &'static str, edit: Edit, expected: Expected) -> Case {
    let mut c = case(parent);
    c.name = name;
    c.parent = Some(parent);
    c.edit = Some(edit.clone());
    c.apply(&edit);
    c.expected = expected;
    c
}
const REQUIRES: &str =
    "contract Controls { requires_event Poster.post.start after Approver.approve.finish(pass) }";
const SEPARATION: &str = "separation Clerk.enter.finish, Approver.approve.finish";
const ONCE: &str = "contract Controls { at_most_once Poster.post.commit per case.id }";
const EXCLUDES: &str = "contract Controls { excludes Clerk.enter.finish, Approver.approve.start }";
const RESPONSE: &str =
    "response case.received -> one_of(end.done) by 5min after case.received otherwise unresolved";
fn serial() -> Case {
    let mut rows = task("e", "Clerk.enter", 0, "person-p");
    rows.extend(task("a", "Approver.approve", 1, "person-q"));
    rows.extend(task("z", "Poster.post", 2, "person-p"));
    rows[2]["after"] = json!(["e-finish"]);
    rows[4]["after"] = json!(["a-finish"]);
    base("l01_serial_ok", REQUIRES, rows)
}
fn cancel() -> Case {
    let mut rows = vec![event("a", "Approver.approve", "start", 0, "person-q")];
    for (phase, at) in [("request", 1), ("ack", 2)] {
        let mut e = event("a", "Approver.approve", "cancel", at, "person-q");
        e["event_id"] = json!(format!("a-{phase}"));
        e["cancel_phase"] = json!(phase);
        if phase == "ack" {
            e["after"] = json!(["a-request"]);
        }
        rows.push(e);
    }
    let mut c = base("l17_cancel_ok", "", rows);
    c.model = c.model.replacen(
        "capability approve() -> Verdict {",
        "capability approve() -> Verdict { cancel cooperative ack_within 1min;",
        1,
    );
    c
}
fn limit_events() -> Case {
    // 5000 complete tasks, 10000 events, a unique temporal order. No contract.
    let events = (0..5000)
        .flat_map(|i| task(&format!("t{i}"), "Poster.post", i * 2, "person-p"))
        .collect();
    base(
        "l23_event_boundary",
        "contract ScopeOnly { excludes Poster.post.start, Clerk.enter.start }",
        events,
    )
}
fn limit_line() -> Case {
    let mut c = case("l09_once_ok");
    c.name = "l23_line_boundary";
    c.parent = None;
    c.edit = None;
    let original = c.records[1].to_string().len();
    let current = c.records[1]["input_version"].as_str().unwrap().len();
    // One multibyte character makes byte length, not Unicode scalar count, material.
    c.records[1]["input_version"] =
        json!(format!("あ{}", "x".repeat(65536 - original + current - 3)));
    assert_eq!(c.records[1].to_string().len(), 65536);
    c
}

pub fn case(name: &'static str) -> Case {
    use Edit::*;
    match name {
        "l01_serial_ok" => serial(),
        "l02_missing_start" => changed(
            name,
            "l01_serial_ok",
            RemoveEvent("e-start"),
            unknown("W-LOG-0", "missing_start"),
        ),
        "l03_missing_finish" => changed(
            name,
            "l01_serial_ok",
            RemoveEvent("z-finish"),
            unknown("W-LOG-0", "missing_finish"),
        ),
        "l04_time_precision" => changed(
            name,
            "l01_serial_ok",
            Replace("/1/time", json!({"earliest":0,"latest":1})),
            unknown("W-LOG-1", "time_precision"),
        ),
        "l05_partial_start" => changed(
            name,
            "l01_serial_ok",
            Replace("/0/cases/0/from_case_start", json!(false)),
            unknown("W-LOG-2", "partial_start"),
        ),
        "l05_unspecified_start" => changed(
            name,
            "l01_serial_ok",
            Replace("/0/cases/0/from_case_start", Value::Null),
            unknown("W-LOG-2", "scope_unspecified"),
        ),
        "l06_partial_end" => changed(
            name,
            "l01_serial_ok",
            Replace("/0/cases/0/through_case_end", json!(false)),
            unknown("W-LOG-2", "partial_end"),
        ),
        "l07_missing_approval" => changed(
            name,
            "l01_serial_ok",
            RemoveTask("a"),
            unknown("W-LOG-2", "no_observed_trigger"),
        ),
        "l07_failed_approval" => changed(
            name,
            "l01_serial_ok",
            Replace("/4/result", json!("fail")),
            unknown("W-LOG-2", "no_observed_trigger"),
        ),
        "l08_separation_ok" => changed(name, "l01_serial_ok", Contract(SEPARATION), ok()),
        "l08_same_person" => changed(
            name,
            "l08_separation_ok",
            Replace("/4/executor_id", json!("person-p")),
            unknown("W-LOG-0", "task_identity_mismatch"),
        ),
        "l08_distinct_person_control" => {
            let mut rows = task("e", "Clerk.enter", 0, "person-p");
            rows.extend(task("a", "Approver.approve", 1, "person-q"));
            base(name, &format!("contract Controls {{ {SEPARATION} }}"), rows)
        }
        "l08_same_person_witness" => changed(
            name,
            "l08_distinct_person_control",
            AssignTask {
                task: "a",
                to: "person-p",
            },
            violation("separation", &["e-finish", "a-finish"]),
        ),
        "l09_once_ok" => base(name, ONCE, task("t1", "Poster.post", 0, "person-p")),
        "l09_duplicate_commit" => changed(
            name,
            "l09_once_ok",
            AppendTask(task("t2", "Poster.post", 1, "person-p")),
            violation("at_most_once", &["t1-finish", "t2-finish"]),
        ),
        "l10_distinct_cases" => changed(
            name,
            "l09_duplicate_commit",
            MoveTask {
                task: "t2",
                case: "d",
            },
            ok(),
        ),
        "l10_reused_task_id" => changed(
            name,
            "l10_distinct_cases",
            RenameTask {
                task: "t2",
                to: "t1",
            },
            ok(),
        ),
        "l11_duplicate_finish" => {
            let mut f = event("t1", "Poster.post", "finish", 1, "person-p");
            f["event_id"] = json!("t1-finish-redelivery");
            changed(
                name,
                "l09_once_ok",
                AppendTask(vec![f]),
                unknown("W-LOG-0", "duplicate_phase"),
            )
        }
        "l12_excludes_violation" => {
            let mut rows = task("e", "Clerk.enter", 0, "person-p");
            rows.extend(task("a", "Approver.approve", 2, "person-q"));
            let mut c = base(name, EXCLUDES, rows);
            c.expected = violation("excludes", &["e-finish", "a-start"]);
            c
        }
        "l13_ambiguous" => {
            let mut rows = task("e", "Clerk.enter", 0, "person-p");
            rows.extend(task("a", "Approver.approve", 1, "person-q"));
            let mut c = base(name, EXCLUDES, rows);
            c.expected = unknown("W-LOG-1", "ambiguous_order");
            c
        }
        "l14_forward" => changed(
            name,
            "l13_ambiguous",
            Replace("/3/after", json!(["e-finish"])),
            violation("excludes", &["e-finish", "a-start"]),
        ),
        "l14_reverse" => changed(
            name,
            "l13_ambiguous",
            Replace("/2/after", json!(["a-start"])),
            ok(),
        ),
        "l15_unmapped_actor" => changed(
            name,
            "l01_serial_ok",
            Replace(
                "/0/actor_map",
                json!([
            {"source_id":"clerk","actor":"Clerk"},{"source_id":"poster","actor":"Poster"}]),
            ),
            Expected {
                unmapped: Some("no_actor_mapping"),
                evidence: &["a-start", "a-finish"],
                ..unknown("W-LOG-3", "unmapped_actor")
            },
        ),
        "l16_unmapped_person" => changed(
            name,
            "l01_serial_ok",
            Replace(
                "/0/executor_map",
                json!([
            {"source_id":"person-p","person":"p"}]),
            ),
            Expected {
                unmapped: Some("no_executor_mapping"),
                evidence: &["a-start", "a-finish"],
                ..unknown("W-LOG-3", "unmapped_actor")
            },
        ),
        "l17_cancel_ok" => cancel(),
        "l17_request_only" => changed(
            name,
            "l17_cancel_ok",
            RemoveEvent("a-ack"),
            unknown("W-LOG-0", "missing_cancel_ack"),
        ),
        "l17_missing_request" => changed(
            name,
            "l17_cancel_without_explicit_edge",
            RemoveEvent("a-request"),
            unknown("W-LOG-0", "missing_cancel_request"),
        ),
        "l17_cancel_without_explicit_edge" => {
            changed(name, "l17_cancel_ok", Replace("/3/after", json!([])), ok())
        }
        "l17_finish_approval_control" => {
            let mut rows = task("a", "Approver.approve", 0, "person-q");
            rows.extend(task("z", "Poster.post", 3, "person-p"));
            let mut c = base(name, REQUIRES, rows);
            c.model = c.model.replacen(
                "capability approve() -> Verdict {",
                "capability approve() -> Verdict { cancel cooperative ack_within 1min;",
                1,
            );
            c
        }
        "l17_ack_not_approval" => {
            let events = case("l17_cancel_ok").records.as_array().unwrap()[1..].to_vec();
            changed(
                name,
                "l17_finish_approval_control",
                ReplaceTask { task: "a", events },
                unknown("W-LOG-2", "no_observed_trigger"),
            )
        }
        "l18_forbidden_ack" => changed(
            name,
            "l17_cancel_ok",
            Model(
                "cancel cooperative ack_within 1min;",
                "cancel forbidden_after_start;",
            ),
            violation("cancel_boundary", &["a-start", "a-ack"]),
        ),
        "l19_bad_mapping" => changed(
            name,
            "l01_serial_ok",
            Replace("/0/actor_map/1/actor", json!("Ghost")),
            invalid("E-LOG-0"),
        ),
        "l20_future_schema" => changed(
            name,
            "l01_serial_ok",
            Replace("/0/schema", json!("relca-log/v2")),
            unsupported("U-LOG-0"),
        ),
        "l20_unknown_flag" => changed(
            name,
            "l01_serial_ok",
            Args(vec!["--noise", "true"]),
            invalid("E-LOG-0"),
        ),
        "l21_response_unsupported" => changed(
            name,
            "l01_serial_ok",
            Contract(RESPONSE),
            unsupported("U-LOG-1"),
        ),
        "l21_independent_violation" => changed(
            name,
            "l08_same_person_witness",
            Contract(RESPONSE),
            Expected {
                code: Some("U-LOG-1"),
                ..violation("separation", &["e-finish", "a-finish"])
            },
        ),
        "l22_empty" => {
            let mut c = base(name, "", vec![]);
            c.records[0]["cases"] = json!([]);
            c.expected = unknown("W-LOG-2", "no_evidence");
            c
        }
        "l23_duration_violation" => changed(
            name,
            "l09_once_ok",
            Replace("/2/time", json!({"earliest":2,"latest":2})),
            violation("duration", &["t1-start", "t1-finish"]),
        ),
        "l23_event_boundary" => limit_events(),
        "l23_event_limit" => changed(
            name,
            "l23_event_boundary",
            AppendTask(vec![event(
                "overflow",
                "Poster.post",
                "start",
                10000,
                "person-p",
            )]),
            Expected {
                limit: Some("event_limit"),
                ..unknown("W-LOG-4", "event_limit")
            },
        ),
        "l23_line_boundary" => limit_line(),
        "l23_line_limit" => {
            let c = case("l23_line_boundary");
            let v = format!("{}x", c.records[1]["input_version"].as_str().unwrap());
            changed(
                name,
                "l23_line_boundary",
                Replace("/1/input_version", json!(v)),
                Expected {
                    limit: Some("line_limit"),
                    ..unknown("W-LOG-4", "line_limit")
                },
            )
        }
        "wire_missing_field" => {
            let mut e = case("l09_once_ok").records[1].clone();
            e.as_object_mut().unwrap().remove("task_id");
            changed(name, "l09_once_ok", Replace("/1", e), invalid("E-LOG-0"))
        }
        "wire_unknown_field" => {
            let mut e = case("l09_once_ok").records[1].clone();
            e["private_payload"] = json!("do-not-echo-private-sentinel");
            changed(name, "l09_once_ok", Replace("/1", e), invalid("E-LOG-0"))
        }
        "wire_duplicate_key" => {
            let row = case("l09_once_ok").records[1].to_string();
            let raw = row.replacen('{', "{\"task_id\":\"untrusted-duplicate\",", 1);
            changed(name, "l09_once_ok", RawLine(1, raw), invalid("E-LOG-0"))
        }
        "wire_duplicate_event_id" => changed(
            name,
            "l09_once_ok",
            Replace("/2/event_id", json!("t1-start")),
            invalid("E-LOG-0"),
        ),
        "wire_after_unknown" => changed(
            name,
            "l09_once_ok",
            Replace("/2/after", json!(["absent-event"])),
            invalid("E-LOG-0"),
        ),
        "wire_after_cycle" => changed(
            name,
            "l09_once_ok",
            Replace("/1/after", json!(["t1-start"])),
            invalid("E-LOG-0"),
        ),
        "wire_after_time_contradiction" => changed(
            name,
            "l09_once_ok",
            Replace("/1/after", json!(["t1-finish"])),
            invalid("E-LOG-1"),
        ),
        "wire_reversed_lifecycle" => changed(
            name,
            "l09_once_ok",
            Replace("/1/time", json!({"earliest":2,"latest":2})),
            violation("lifecycle", &["t1-start", "t1-finish"]),
        ),
        "wire_outcome_domain" => changed(
            name,
            "l09_once_ok",
            Replace("/2/result", json!("not-in-Ack")),
            violation("result_domain", &["t1-finish"]),
        ),
        "wire_result_type" => changed(
            name,
            "l09_once_ok",
            Replace("/2/result", json!({"pass":true})),
            invalid("E-LOG-0"),
        ),
        "wire_window" => changed(
            name,
            "l09_once_ok",
            Replace("/0/window/to", json!(0)),
            invalid("E-LOG-0"),
        ),
        "wire_unknown_operation" => changed(
            name,
            "l09_once_ok",
            Replace("/2/operation", json!("Poster.unknown")),
            unknown("W-LOG-3", "unmapped_operation"),
        ),
        "wire_cut_undeclared" => changed(
            name,
            "l01_serial_ok",
            Model("cut C { include [Clerk,Approver,Poster] }", ""),
            Expected {
                code: Some("W-CUT-0"),
                ..ok()
            },
        ),
        "wire_null_version" => changed(
            name,
            "l09_once_ok",
            Replace("/1/input_version", Value::Null),
            ok(),
        ),
        "wire_equal_time_requires" => changed(
            name,
            "l01_serial_ok",
            Replace("/5/after", json!([])),
            unknown("W-LOG-1", "ambiguous_order"),
        ),
        "wire_future_scope" => changed(
            name,
            "l01_serial_ok",
            Scope(Some("whole-process")),
            unsupported("U-LOG-0"),
        ),
        "wire_missing_scope" => changed(name, "l01_serial_ok", Scope(None), invalid("E-LOG-0")),
        "wire_future_unit" => changed(
            name,
            "l01_serial_ok",
            Replace("/0/time_unit", json!("s")),
            unsupported("U-LOG-0"),
        ),
        "wire_no_requests" => changed(name, "l09_once_ok", Model(ONCE, ""), unsupported("U-SEM-0")),
        "wire_explicit_task_edge" => changed(
            name,
            "l09_once_ok",
            Replace("/2/after", json!(["t1-start"])),
            ok(),
        ),
        "wire_source_cycle" => changed(
            name,
            "wire_explicit_task_edge",
            Replace("/1/after", json!(["t1-finish"])),
            invalid("E-LOG-1"),
        ),
        "wire_no_implicit_actor_match" => changed(
            name,
            "l15_unmapped_actor",
            ActorTask {
                task: "a",
                to: "Approver",
            },
            Expected {
                unmapped: Some("no_actor_mapping"),
                evidence: &["a-start", "a-finish"],
                ..unknown("W-LOG-3", "unmapped_actor")
            },
        ),
        "wire_no_implicit_person_match" => changed(
            name,
            "l16_unmapped_person",
            AssignTask { task: "a", to: "q" },
            Expected {
                unmapped: Some("no_executor_mapping"),
                evidence: &["a-start", "a-finish"],
                ..unknown("W-LOG-3", "unmapped_actor")
            },
        ),
        "wire_operation_actor_mismatch" => changed(
            name,
            "l01_serial_ok",
            Replace("/0/actor_map/1/actor", json!("Clerk")),
            Expected {
                unmapped: Some("operation_actor_mismatch"),
                evidence: &["a-start", "a-finish"],
                ..unknown("W-LOG-3", "unmapped_actor")
            },
        ),
        _ => panic!("unknown acceptance input {name}"),
    }
}
