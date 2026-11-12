//! ADR 0061 synthetic observations. Expectations are hand constants, never captures.
use serde_json::{Value, json};

pub const MODEL: &str = r#"model Approval version "0.2" {
type Proposal = enum { yes, no }
type Decision = enum { approved, returned, pending }
type Checked = enum { checked }
people [p,q]
actor P { executors [p,q]; capability propose() -> Proposal {
 duration fixed 1min; result nondet { yes, no }; cancel cooperative ack_within 1min
} }
actor A { executors [p,q]; capability decide() -> Decision {
 duration fixed 1min; result nondet { approved, returned, pending }; cancel cooperative ack_within 1min
} }
actor R { executors [p,q]; capability check() -> Checked {
 duration fixed 2min; result nondet { checked }; cancel cooperative ack_within 1min
} capability scan() -> Checked {
 duration fixed 3min; result nondet { checked }; cancel cooperative ack_within 1min
} }
process Flow { x = P.propose(); y = R.check(); z = A.decide(); end done }
cut C { include [P,A,R] }
contract Scope { excludes P.propose.finish, P.propose.start }
arrivals fixed [0min,0min,0min]
bounds { cases 3; live_tasks 3; clock 30000min }
}"#;

#[derive(Clone, Debug)]
pub struct Numbers {
    pub n: u64,
    pub d: u64,
    pub rate: Option<&'static str>,
    pub ineligible: u64,
    pub unknown: u64,
    pub no_approval: u64,
    pub total: Option<&'static str>,
    pub mean: Option<&'static str>,
    pub known_time: u64,
    pub unknown_time: u64,
}
impl Default for Numbers {
    fn default() -> Self {
        Self {
            n: 2,
            d: 3,
            rate: Some("2/3"),
            ineligible: 0,
            unknown: 0,
            no_approval: 0,
            total: Some("7"),
            mean: Some("7/3"),
            known_time: 3,
            unknown_time: 0,
        }
    }
}
#[derive(Clone, Debug)]
pub struct Expected {
    pub exit: i32,
    pub code: Option<&'static str>,
    pub reason: Option<&'static str>,
    pub numbers: Option<Numbers>,
    pub no_metrics: bool,
    pub truncated: bool,
}
impl Default for Expected {
    fn default() -> Self {
        Self {
            exit: 0,
            code: None,
            reason: None,
            numbers: Some(Numbers::default()),
            no_metrics: false,
            truncated: false,
        }
    }
}
#[derive(Clone, Debug)]
pub struct Case {
    pub name: &'static str,
    pub model: String,
    pub records: Vec<Value>,
    pub spec: Value,
    pub raw_spec: Option<String>,
    pub raw_log: Option<String>,
    pub hash_mismatch: bool,
    pub expected: Expected,
}
impl Case {
    pub fn log(&self) -> String {
        self.raw_log
            .clone()
            .unwrap_or_else(|| self.records.iter().map(|v| format!("{v}\n")).collect())
    }
    pub fn event_mut(&mut self, case: &str, task: &str, phase: &str) -> &mut Value {
        self.records
            .iter_mut()
            .skip(1)
            .find(|e| e["case_id"] == case && e["task_id"] == task && e["lifecycle"] == phase)
            .unwrap()
    }
    pub fn episode_mut(&mut self, case: &str) -> &mut Value {
        self.spec["episodes"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|e| e["case_id"] == case)
            .unwrap()
    }
    pub fn remove_event(&mut self, id: &str) {
        self.records.retain(|e| e["event_id"] != id);
        for e in self.records.iter_mut().skip(1) {
            e["after"].as_array_mut().unwrap().retain(|a| a != id);
        }
    }
    pub fn remove_task(&mut self, case: &str, task: &str) {
        let ids: Vec<_> = self
            .records
            .iter()
            .filter(|e| e["case_id"] == case && e["task_id"] == task)
            .map(|e| e["event_id"].as_str().unwrap().to_owned())
            .collect();
        for id in ids {
            self.remove_event(&id);
        }
    }
    fn unknown(&mut self, code: &'static str, reason: &'static str) {
        self.expected.exit = 2;
        self.expected.code = Some(code);
        self.expected.reason = Some(reason);
    }
    fn refuse(&mut self, exit: i32, code: &'static str, reason: &'static str) {
        self.expected = Expected {
            exit,
            code: Some(code),
            reason: Some(reason),
            numbers: None,
            no_metrics: true,
            truncated: false,
        };
    }
    fn numbers(&mut self) -> &mut Numbers {
        self.expected.numbers.as_mut().unwrap()
    }
    fn unknown_time_c1(&mut self) {
        let n = self.numbers();
        n.known_time = 2;
        n.unknown_time = 1;
        n.total = Some("5");
        n.mean = Some("5/2");
    }
    fn unknown_both_c1(&mut self) {
        self.unknown_time_c1();
        let n = self.numbers();
        n.n = 1;
        n.d = 2;
        n.rate = Some("1/2");
        n.unknown = 1;
    }
}
pub fn scope(id: &str) -> Value {
    json!({"case_id":id,"from_case_start":true,"through_case_end":true})
}
pub fn profile() -> Value {
    json!({"approval_operation":"A.decide","proposal_operation":"P.propose",
        "proposal_approve_results":["yes"],"approval_decision_results":["approved","returned"],
        "approval_approve_results":["approved"],"reinvestigation_operations":["R.check","R.scan"]})
}
pub fn episode(case: &str, approval: &str, rechecks: Value) -> Value {
    json!({"case_id":case,"approval_task_id":approval,"proposal_task_id":"proposal","reinvestigation_task_ids":rechecks})
}
pub fn task(
    case: &str,
    id: &str,
    op: &str,
    start: u64,
    stop: u64,
    person: &str,
    result: &str,
) -> Vec<Value> {
    [ ("start", start), ("finish", stop) ].into_iter().map(|(phase, time)| {
        json!({"kind":"event","event_id":format!("{case}-{id}-{phase}"),"case_id":case,"task_id":id,
          "operation":op,"lifecycle":phase,"cancel_phase":null,"time":{"earliest":time,"latest":time},
          "after":if phase=="finish" {json!([format!("{case}-{id}-start")])} else {json!([])},
          "actor_id":op.split('.').next().unwrap(),"executor_id":person,
          "input_version":format!("{case}-v"),
          "output_version":if phase=="finish" {json!(format!("{case}-v"))} else {Value::Null},
          "result":if phase=="finish" {json!(result)} else {Value::Null}})
    }).collect()
}
pub fn base(name: &'static str) -> Case {
    let h = json!({"kind":"header","schema":"relca-log/v1","clock":"synthetic-clock","time_unit":"min",
      "window":{"from":0,"to":30000},"cases":[scope("c1"),scope("c2"),scope("c3")],
      "actor_map":[{"source_id":"P","actor":"P"},{"source_id":"A","actor":"A"},{"source_id":"R","actor":"R"}],
      "executor_map":[{"source_id":"p","person":"p"},{"source_id":"q","person":"q"}]});
    let mut records = vec![h];
    for case in ["c1", "c2", "c3"] {
        records.extend(task(case, "proposal", "P.propose", 0, 1, "p", "yes"));
        let at = if case == "c3" { 12 } else { 10 };
        records.extend(task(
            case,
            "approval",
            "A.decide",
            at,
            at + 1,
            "q",
            if case == "c2" { "returned" } else { "approved" },
        ));
    }
    records.extend(task("c1", "r1", "R.check", 2, 4, "p", "checked"));
    records.extend(task("c3", "r1", "R.check", 5, 7, "p", "checked"));
    records.extend(task("c3", "r2", "R.scan", 7, 10, "q", "checked"));
    Case {
        name,
        model: MODEL.into(),
        records,
        spec: json!({"schema":"relca-approval-spec/v1",
      "model_sha256":"filled-by-harness","log_sha256":"filled-by-harness","profile":profile(),
      "episodes":[episode("c1","approval",json!(["r1"])),episode("c2","approval",json!([])),episode("c3","approval",json!(["r1","r2"]))]}),
        raw_spec: None,
        raw_log: None,
        hash_mismatch: false,
        expected: Expected::default(),
    }
}
fn single(name: &'static str) -> Case {
    let mut c = base(name);
    c.records[0]["cases"] = json!([scope("c1")]);
    c.records
        .retain(|e| e["kind"] == "header" || e["case_id"] == "c1");
    c.spec["episodes"].as_array_mut().unwrap().truncate(1);
    c.expected.numbers = Some(Numbers {
        n: 1,
        d: 1,
        rate: Some("1"),
        total: Some("2"),
        mean: Some("2"),
        known_time: 1,
        ..Numbers::default()
    });
    c
}

pub const NAMES: &[&str] = &[
    "a01_baseline",
    "a01_approved_control",
    "a02_ineligible",
    "a02_eligible_control",
    "a03_unobserved_case",
    "a04_null",
    "a04_empty_control",
    "a05_missing_start",
    "a05_missing_finish",
    "a06_precision",
    "a07_same_tick",
    "a07_after_control",
    "a08_same_person_union",
    "a08_two_person_control",
    "a08_alias_control",
    "a09_missing_approval_start",
    "a09_cancel_ack",
    "a10_two_decisions",
    "a10_duplicate_finish",
    "a10_case_key_control",
    "a11_missing_link",
    "a12_version_mismatch",
    "a12_missing_version",
    "a13_zero_denominator",
    "a14_reverse",
    "a14_rename",
    "a15_unmapped",
    "a15_cut_outside",
    "a15_partial_start",
    "a15_partial_end",
    "a15_endpoint_null",
    "a15_header_only",
    "a15_orphan_recheck",
    "a16_duplicate_key",
    "a16_unknown_field",
    "a16_missing_field",
    "a16_duplicate_episode",
    "a16_double_assignment",
    "a16_hash_mismatch",
    "a16_future_schema",
    "a16_missing_task",
    "a16_multiple_profiles",
    "a16_bad_result",
    "a16_bad_operation",
    "a17_spec_boundary",
    "a17_spec_limit",
    "a17_episode_boundary",
    "a17_episode_limit",
    "a17_event_boundary",
    "a17_event_limit",
    "a17_line_boundary",
    "a17_line_limit",
    "a17_order_limit",
    "a18_independent_violation",
    "a18_q1_unsupported",
    "a18_defer",
    "a18_cross_episode_overlap",
];

pub fn case(name: &'static str) -> Case {
    let mut c = base(name);
    match name {
        "a01_baseline" => {}
        "a01_approved_control" => {
            c.event_mut("c2", "approval", "finish")["result"] = json!("approved");
            let n = c.numbers();
            n.n = 3;
            n.rate = Some("1");
        }
        "a02_ineligible" | "a02_eligible_control" => {
            c.records[0]["cases"]
                .as_array_mut()
                .unwrap()
                .push(scope("c4"));
            c.records.extend(task(
                "c4",
                "proposal",
                "P.propose",
                0,
                1,
                "p",
                if name == "a02_ineligible" {
                    "no"
                } else {
                    "yes"
                },
            ));
            c.records
                .extend(task("c4", "approval", "A.decide", 10, 11, "q", "returned"));
            c.spec["episodes"]
                .as_array_mut()
                .unwrap()
                .push(episode("c4", "approval", json!([])));
            let n = c.numbers();
            n.known_time = 4;
            n.mean = Some("7/4");
            if name == "a02_ineligible" {
                n.ineligible = 1;
            } else {
                n.d = 4;
                n.rate = Some("1/2");
            }
        }
        "a03_unobserved_case" => {
            c.records[0]["cases"]
                .as_array_mut()
                .unwrap()
                .push(scope("c5"));
            c.numbers().no_approval = 1;
            c.unknown("W-LOG-2", "no_observed_approval");
        }
        "a04_null" | "a04_empty_control" => {
            c.remove_task("c1", "r1");
            c.episode_mut("c1")["reinvestigation_task_ids"] = if name == "a04_null" {
                Value::Null
            } else {
                json!([])
            };
            if name == "a04_null" {
                c.unknown_time_c1();
                c.unknown("W-LOG-0", "missing_reinvestigation_observation");
            } else {
                let n = c.numbers();
                n.total = Some("5");
                n.mean = Some("5/3");
            }
        }
        "a05_missing_start" | "a05_missing_finish" => {
            c.remove_event(if name == "a05_missing_start" {
                "c1-r1-start"
            } else {
                "c1-r1-finish"
            });
            c.unknown_time_c1();
            c.unknown(
                "W-LOG-0",
                if name == "a05_missing_start" {
                    "missing_start"
                } else {
                    "missing_finish"
                },
            );
        }
        "a06_precision" => {
            c.event_mut("c1", "r1", "start")["time"]["latest"] = json!(3);
            c.unknown_time_c1();
            c.unknown("W-LOG-1", "time_precision");
        }
        "a07_same_tick" | "a07_after_control" => {
            c.event_mut("c1", "approval", "start")["time"] = json!({"earliest":4,"latest":4});
            c.event_mut("c1", "approval", "finish")["time"] = json!({"earliest":5,"latest":5});
            if name == "a07_after_control" {
                c.event_mut("c1", "approval", "start")["after"] = json!(["c1-r1-finish"]);
            } else {
                c.unknown_time_c1();
                c.unknown("W-LOG-1", "ambiguous_approval_order");
            }
        }
        "a08_same_person_union" | "a08_two_person_control" | "a08_alias_control" => {
            c = single(name);
            let who = if name == "a08_two_person_control" {
                "q"
            } else if name == "a08_alias_control" {
                "alias-p"
            } else {
                "p"
            };
            if name == "a08_alias_control" {
                c.records[0]["executor_map"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!({"source_id":"alias-p","person":"p"}));
            }
            c.records
                .extend(task("c1", "r2", "R.scan", 3, 6, who, "checked"));
            c.episode_mut("c1")["reinvestigation_task_ids"] = json!(["r1", "r2"]);
            let n = c.numbers();
            n.total = Some(if name == "a08_two_person_control" {
                "5"
            } else {
                "4"
            });
            n.mean = n.total;
        }
        "a09_missing_approval_start" | "a09_cancel_ack" => {
            c = single(name);
            if name == "a09_missing_approval_start" {
                c.remove_event("c1-approval-start");
            } else {
                c.remove_event("c1-approval-finish");
                let mut request =
                    task("c1", "approval", "A.decide", 10, 11, "q", "approved")[0].clone();
                request["event_id"] = json!("c1-approval-request");
                request["lifecycle"] = json!("cancel");
                request["cancel_phase"] = json!("request");
                request["after"] = json!(["c1-approval-start"]);
                let mut ack = request.clone();
                ack["event_id"] = json!("c1-approval-ack");
                ack["cancel_phase"] = json!("ack");
                ack["time"] = json!({"earliest":11,"latest":11});
                ack["after"] = json!(["c1-approval-request"]);
                c.records.extend([request, ack]);
            }
            c.expected.numbers = Some(Numbers {
                n: 0,
                d: 0,
                rate: None,
                unknown: 1,
                total: None,
                mean: None,
                known_time: 0,
                unknown_time: 1,
                ..Numbers::default()
            });
            c.unknown(
                "W-LOG-0",
                if name == "a09_missing_approval_start" {
                    "missing_start"
                } else {
                    "task_correspondence"
                },
            );
        }
        "a10_two_decisions" => {
            c = single(name);
            c.records
                .extend(task("c1", "approval2", "A.decide", 14, 15, "q", "returned"));
            c.spec["episodes"]
                .as_array_mut()
                .unwrap()
                .push(episode("c1", "approval2", json!([])));
            let n = c.numbers();
            n.d = 2;
            n.rate = Some("1/2");
            n.known_time = 2;
            n.mean = Some("1");
        }
        "a10_duplicate_finish" => {
            let mut ev = c.event_mut("c1", "approval", "finish").clone();
            ev["event_id"] = json!("c1-approval-finish-copy");
            c.records.push(ev);
            c.unknown_both_c1();
            c.unknown("W-LOG-0", "duplicate_phase");
        }
        "a10_case_key_control" => {} // base deliberately reuses proposal, approval, r1 across cases
        "a11_missing_link" => {
            c.spec["episodes"].as_array_mut().unwrap().remove(0);
            c.unknown_both_c1();
            c.unknown("W-LOG-0", "missing_episode_link");
        }
        "a12_version_mismatch" | "a12_missing_version" => {
            c.event_mut("c1", "approval", "start")["input_version"] =
                if name == "a12_version_mismatch" {
                    json!("different-v")
                } else {
                    Value::Null
                };
            c.unknown_both_c1();
            c.unknown(
                "W-LOG-0",
                if name == "a12_version_mismatch" {
                    "version_mismatch"
                } else {
                    "missing_version"
                },
            );
        }
        "a13_zero_denominator" => {
            for e in c.records.iter_mut().skip(1) {
                if e["operation"] == "P.propose" && e["lifecycle"] == "finish" {
                    e["result"] = json!("no");
                }
            }
            let n = c.numbers();
            n.n = 0;
            n.d = 0;
            n.rate = None;
            n.ineligible = 3;
            c.unknown("W-LOG-2", "zero_denominator");
        }
        "a14_reverse" => {
            c.records[1..].reverse();
            c.spec["episodes"].as_array_mut().unwrap().reverse();
        }
        "a14_rename" => {
            // Change model names, explicit mappings, operations, and profile together.
            c.model = c
                .model
                .replace("P.propose", "Zeta.z")
                .replace("A.decide", "Beta.a")
                .replace("R.check", "Gamma.k")
                .replace("R.scan", "Gamma.s")
                .replace("actor P ", "actor Zeta ")
                .replace("actor A ", "actor Beta ")
                .replace("actor R ", "actor Gamma ")
                .replace("capability propose()", "capability z()")
                .replace("capability decide()", "capability a()")
                .replace("capability check()", "capability k()")
                .replace("capability scan()", "capability s()")
                .replace("[P,A,R]", "[Zeta,Beta,Gamma]");
            for e in c.records.iter_mut().skip(1) {
                let op = e["operation"]
                    .as_str()
                    .unwrap()
                    .replace("P.propose", "Zeta.z")
                    .replace("A.decide", "Beta.a")
                    .replace("R.check", "Gamma.k")
                    .replace("R.scan", "Gamma.s");
                e["operation"] = json!(op);
            }
            for m in c.records[0]["actor_map"].as_array_mut().unwrap() {
                m["actor"] = json!(match m["actor"].as_str().unwrap() {
                    "P" => "Zeta",
                    "A" => "Beta",
                    _ => "Gamma",
                });
            }
            c.spec["profile"]["proposal_operation"] = json!("Zeta.z");
            c.spec["profile"]["approval_operation"] = json!("Beta.a");
            c.spec["profile"]["reinvestigation_operations"] = json!(["Gamma.k", "Gamma.s"]);
        }
        "a15_unmapped" | "a15_cut_outside" => {
            for e in c.records.iter_mut().skip(1) {
                if e["case_id"] == "c1" && e["task_id"] == "approval" {
                    e["actor_id"] = json!("outside");
                }
            }
            if name == "a15_cut_outside" {
                c.model=c.model.replace("cut C { include [P,A,R] }","actor Outside { capacity 1 } cut C { include [P,A,R]; exclude Outside reason \"outside\" loses \"not checked\" }");
                c.records[0]["actor_map"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!({"source_id":"outside","actor":"Outside"}));
            }
            c.unknown_both_c1();
            c.unknown("W-LOG-3", "unmapped_actor");
        }
        "a15_partial_start" | "a15_partial_end" | "a15_endpoint_null" => {
            let key = if name == "a15_partial_end" {
                "through_case_end"
            } else {
                "from_case_start"
            };
            c.records[0]["cases"][0][key] = if name == "a15_endpoint_null" {
                Value::Null
            } else {
                json!(false)
            };
            c.unknown(
                "W-LOG-2",
                if name == "a15_endpoint_null" {
                    "scope_unspecified"
                } else if name == "a15_partial_end" {
                    "partial_end"
                } else {
                    "partial_start"
                },
            );
        }
        "a15_header_only" => {
            c.records.truncate(1);
            c.spec["episodes"] = json!([]);
            c.expected.numbers = Some(Numbers {
                n: 0,
                d: 0,
                rate: None,
                no_approval: 3,
                total: None,
                mean: None,
                known_time: 0,
                ..Numbers::default()
            });
            c.unknown("W-LOG-2", "no_observed_approval");
        }
        "a15_orphan_recheck" => {
            c.episode_mut("c1")["reinvestigation_task_ids"] = json!([]);
            c.unknown_time_c1();
            c.unknown("W-LOG-0", "unassigned_reinvestigation_task");
        }
        "a16_duplicate_key" => {
            c.raw_spec = Some(
                "{\"schema\":\"relca-approval-spec/v1\",\"schema\":\"relca-approval-spec/v1\"}"
                    .into(),
            );
            c.refuse(3, "E-LOG-0", "approval_spec_duplicate_key");
        }
        "a16_unknown_field" => {
            c.spec["do-not-echo-private-sentinel"] = json!("private");
            c.refuse(3, "E-LOG-0", "approval_spec_bad_field");
        }
        "a16_missing_field" => {
            c.spec["episodes"][0]
                .as_object_mut()
                .unwrap()
                .remove("reinvestigation_task_ids");
            c.refuse(3, "E-LOG-0", "approval_spec_bad_field");
        }
        "a16_duplicate_episode" => {
            let e = c.spec["episodes"][0].clone();
            c.spec["episodes"].as_array_mut().unwrap().push(e);
            c.refuse(3, "E-LOG-0", "approval_spec_duplicate_assignment");
        }
        "a16_double_assignment" => {
            c.records
                .extend(task("c1", "a2", "A.decide", 14, 15, "q", "approved"));
            c.spec["episodes"]
                .as_array_mut()
                .unwrap()
                .push(episode("c1", "a2", json!(["r1"])));
            c.refuse(3, "E-LOG-0", "approval_spec_duplicate_assignment");
        }
        "a16_hash_mismatch" => {
            c.hash_mismatch = true;
            c.refuse(3, "E-LOG-0", "approval_spec_hash_mismatch");
        }
        "a16_future_schema" => {
            c.spec = json!({"schema":"relca-approval-spec/v9","future":{"private":"do-not-echo-private-sentinel"}});
            c.refuse(4, "U-LOG-0", "approval_spec_schema");
        }
        "a16_missing_task" => {
            c.episode_mut("c1")["reinvestigation_task_ids"] = json!(["unobserved"]);
            c.remove_task("c1", "r1");
            c.unknown_time_c1();
            c.unknown("W-LOG-0", "missing_reinvestigation_observation");
        }
        "a16_multiple_profiles" => {
            c.spec["profile"] = json!([profile(), profile()]);
            c.refuse(4, "U-LOG-1", "unsupported_approval_profiles");
        }
        "a16_bad_result" => {
            c.spec["profile"]["approval_approve_results"] = json!(["invented"]);
            c.refuse(3, "E-LOG-0", "approval_spec_bad_profile");
        }
        "a16_bad_operation" => {
            c.spec["profile"]["approval_operation"] = json!("A.invented");
            c.refuse(3, "E-LOG-0", "approval_spec_bad_profile");
        }
        "a17_spec_boundary" => {} // harness pads valid JSON with spaces to exactly 1048576 bytes
        "a17_spec_limit" | "a17_episode_limit" | "a17_event_limit" | "a17_line_limit"
        | "a17_order_limit" => {
            c.expected.numbers = None;
            c.expected.truncated = true;
            c.unknown(
                "W-LOG-4",
                match name {
                    "a17_spec_limit" => "approval_spec_limit",
                    "a17_episode_limit" => "approval_episode_limit",
                    "a17_event_limit" => "event_limit",
                    "a17_line_limit" => "line_limit",
                    _ => "order_limit",
                },
            );
            if name == "a17_episode_limit" {
                fill_episodes(&mut c, 10001);
            }
            if name == "a17_event_limit" {
                fill_events(&mut c, 10001);
            }
            if name == "a17_order_limit" {
                for i in 0..11 {
                    c.records.extend(task(
                        "c1",
                        &format!("extra{i}"),
                        "P.propose",
                        0,
                        1,
                        "p",
                        "yes",
                    ));
                }
            }
        }
        "a17_episode_boundary" => {
            fill_episodes(&mut c, 10000);
            c.expected.numbers = None;
            c.unknown("W-LOG-0", "missing_start");
        }
        "a17_event_boundary" => {
            fill_events(&mut c, 10000);
        }
        "a17_line_boundary" => {} // harness pads one event with JSON whitespace to exactly 65536 bytes
        "a18_independent_violation" => {
            c.model = c.model.replace(
                "bounds {",
                "contract Extra { separation P.propose.finish, A.decide.finish } bounds {",
            );
            for e in c.records.iter_mut().skip(1) {
                if e["case_id"] == "c1" && e["task_id"] == "approval" {
                    e["executor_id"] = json!("p");
                }
            }
            c.remove_task("c1", "r1");
            c.episode_mut("c1")["reinvestigation_task_ids"] = Value::Null;
            c.unknown_time_c1();
            c.expected.exit = 1;
            c.expected.code = Some("E-LOG-2");
            c.expected.reason = Some("separation");
        }
        "a18_q1_unsupported" => {
            c.model = c
                .model
                .replace("x = P.propose()", "x = P.propose() response_loss none");
            c.expected.numbers = None;
            c.expected.exit = 4;
            c.expected.code = Some("U-LOG-1");
            c.expected.reason = Some("unsupported_q1_model_feature");
        }
        "a18_defer" => {
            c.model = c.model.replace(
                "include [P,A,R]",
                "include [P,A,R]; defer R.check reason \"unmeasured\" loses \"burden\"",
            );
            let n = c.numbers();
            n.total = None;
            n.mean = None;
            n.known_time = 0;
            n.unknown_time = 3;
            c.unknown("W-LOG-0", "not_configured");
        }
        "a18_cross_episode_overlap" => {
            c.event_mut("c3", "r1", "start")["time"] = json!({"earliest":2,"latest":2});
            c.event_mut("c3", "r1", "finish")["time"] = json!({"earliest":4,"latest":4});
            let n = c.numbers();
            n.total = None;
            n.mean = None;
            c.expected.exit = 4;
            c.expected.code = Some("U-LOG-1");
            c.expected.reason = Some("unsupported_cross_episode_overlap");
        }
        _ => panic!("unknown fixed acceptance name {name}"),
    }
    c
}

fn fill_episodes(c: &mut Case, target: usize) {
    for i in 3..target {
        c.spec["episodes"]
            .as_array_mut()
            .unwrap()
            .push(episode("c1", &format!("a{i}"), json!([])));
    }
}
fn fill_events(c: &mut Case, target: usize) {
    // Do not create thousands of CaseScopes: that would hit the header byte limit
    // instead of the intended event limit. Unlinked checks stay explicitly unknown.
    let mut i = 0;
    while c.records.len() - 1 < target {
        let id = format!("extra{i}");
        let mut more = task("c1", &id, "R.check", 20, 22, "p", "checked");
        let remaining = target - (c.records.len() - 1);
        more.truncate(remaining);
        c.records.extend(more);
        i += 1;
    }
    if !c.expected.truncated {
        c.expected.numbers = None;
        c.unknown("W-LOG-0", "unassigned_reinvestigation_task");
    }
}
