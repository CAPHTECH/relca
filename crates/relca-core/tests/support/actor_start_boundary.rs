//! Hand-only U1 inputs/oracles. No production evaluator or captured output is
//! used to manufacture expectations. See ../u12_boundary_calculations.md.
#![allow(dead_code)]
use serde_json::{Value, json};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Case {
    One,
    Zero,
    Unused,
    Dead,
    Sequential,
    ForkAll,
    SharedPerson,
    ResourceOnly,
    BothCallOrder,
    BothPrecise,
    RequiresZero,
    BranchShared,
}
pub const CASES: [Case; 12] = [
    Case::One,
    Case::Zero,
    Case::Unused,
    Case::Dead,
    Case::Sequential,
    Case::ForkAll,
    Case::SharedPerson,
    Case::ResourceOnly,
    Case::BothCallOrder,
    Case::BothPrecise,
    Case::RequiresZero,
    Case::BranchShared,
];
pub const CAPS: [&str; 5] = ["A.x", "B.before", "B.after", "B.pick", "B.y"];
impl Case {
    pub fn name(self) -> &'static str {
        match self {
            Self::One => "one",
            Self::Zero => "zero",
            Self::Unused => "unused",
            Self::Dead => "dead",
            Self::Sequential => "sequential",
            Self::ForkAll => "fork_all",
            Self::SharedPerson => "shared_person",
            Self::ResourceOnly => "resource_only",
            Self::BothCallOrder => "both_call_order",
            Self::BothPrecise => "both_precise",
            Self::RequiresZero => "requires_zero",
            Self::BranchShared => "branch_shared",
        }
    }
    pub fn source(self) -> String {
        let capacity = if matches!(self, Self::One | Self::SharedPerson | Self::ResourceOnly) {
            1
        } else {
            0
        };
        let people = if self == Self::SharedPerson {
            "people [p]"
        } else {
            ""
        };
        let a_pool = if self == Self::SharedPerson {
            "executors [p]".into()
        } else {
            format!("capacity {capacity}")
        };
        let b_pool = if self == Self::SharedPerson {
            "executors [p]"
        } else {
            "capacity 1"
        };
        let resource = if matches!(
            self,
            Self::ResourceOnly | Self::BothCallOrder | Self::BothPrecise
        ) {
            "resource R capacity 0"
        } else {
            ""
        };
        let uses = if resource.is_empty() {
            ""
        } else {
            "uses R: 1;"
        };
        let observed = if self == Self::RequiresZero {
            "observed case { ready: bool = false }"
        } else {
            ""
        };
        let requires = if self == Self::RequiresZero {
            "requires ready == true else fail;"
        } else {
            ""
        };
        let pick = if self == Self::BranchShared {
            "pass, fail"
        } else {
            "pass"
        };
        let policy = if matches!(self, Self::ResourceOnly | Self::BothCallOrder) {
            "arrivals fixed [0min]\n policy fifo tie_by case_id then call_order"
        } else {
            ""
        };
        let process = match self {
            Self::Unused => "pre = B.before(); end done",
            Self::Dead => "j = B.pick(); if j == fail { x = A.x(); end never } else {} end done",
            Self::Sequential => "pre = B.before(); x = A.x(); post = B.after(); end done",
            Self::ForkAll => {
                "fork f { x = A.x(); y = B.y() }; join all f; post = B.after(); end done"
            }
            Self::SharedPerson => "fork f { x = A.x(); y = B.y() }; join all f; end done",
            Self::BranchShared => {
                "j = B.pick(); if j == pass { x = A.x(); y = B.y(); end blocked } else { y2 = B.y(); alt = B.after(); end done }"
            }
            _ => "x = A.x(); end done",
        };
        format!(
            r#"model ActorStartBoundary version "0.2" {{
 type T = enum {{ pass, fail }}
 {people}
 {observed}
 {resource}
 actor Doc {{ source document }}
 actor A {{ {a_pool}; capability x() -> T {{ reads [Doc]; {uses} {requires} duration fixed 1min; result nondet {{ pass }} }} }}
 actor B {{ {b_pool};
   capability before() -> T {{ reads [Doc]; duration fixed 1min; result nondet {{ pass }} }}
   capability after() -> T {{ reads [Doc]; duration fixed 1min; result nondet {{ pass }} }}
   capability pick() -> T {{ reads [Doc]; duration fixed 1min; result nondet {{ {pick} }} }}
   capability y() -> T {{ reads [Doc]; duration fixed 2min; result nondet {{ pass }} }}
 }}
 process P {{ {process} }}
 cut K {{ question "Actor start boundary"; include [Doc, A, B] }}
 {policy}
 bounds {{ cases 1; live_tasks 3; clock 10min }}
 verify safety no_resource_overcommit
 verify reachable end.done
}}
"#
        )
    }
    pub fn actor_reason(self) -> bool {
        matches!(
            self,
            Self::Zero | Self::Sequential | Self::ForkAll | Self::RequiresZero | Self::BranchShared
        )
    }
    pub fn blocked(self) -> bool {
        !matches!(
            self,
            Self::One | Self::Unused | Self::Dead | Self::SharedPerson
        )
    }
    pub fn exit(self) -> i32 {
        i32::from(self.blocked())
    }
    pub fn observed_actors(self) -> Value {
        match self {
            Self::Unused => json!(["B"]),
            Self::Dead
            | Self::Sequential
            | Self::ForkAll
            | Self::SharedPerson
            | Self::BranchShared => json!(["A", "B"]),
            _ => json!(["A"]),
        }
    }
    pub fn framing_rows(self) -> Vec<Value> {
        let a = "actor_capacity_infeasible";
        match self {
            Self::ResourceOnly | Self::BothCallOrder => {
                vec![not_end("done", "resource_infeasible")]
            }
            Self::BothPrecise => vec![end("done", false, json!([]), json!([]))],
            Self::Dead => vec![
                end("never", false, json!([]), json!([])),
                end(
                    "done",
                    true,
                    json!([]),
                    json!([{"call":"j","capability":"B.pick"}]),
                ),
            ],
            Self::BranchShared => vec![
                not_end("blocked", a),
                end(
                    "done",
                    true,
                    json!([{"call":"j","capability":"B.pick","selects":"all_contexts"}]),
                    json!([]),
                ),
            ],
            _ if self.actor_reason() => vec![not_end("done", a)],
            _ => vec![end("done", true, json!([]), json!([]))],
        }
    }
    pub fn retention_rows(self) -> Vec<Value> {
        let counts = match self {
            Self::Unused => [(0, 0), (1, 1), (0, 0), (0, 0), (0, 0)],
            Self::Dead => [(0, 0), (0, 0), (0, 0), (1, 1), (0, 0)],
            Self::Sequential => [(1, 1), (1, 1), (1, 1), (0, 0), (0, 0)],
            Self::ForkAll => [(1, 1), (0, 0), (1, 1), (0, 0), (1, 1)],
            Self::SharedPerson => [(1, 1), (0, 0), (0, 0), (0, 0), (1, 1)],
            Self::BothPrecise | Self::RequiresZero => [(0, 0); 5],
            Self::BranchShared => [(0, 1), (0, 0), (0, 1), (1, 1), (1, 1)],
            _ => [(1, 1), (0, 0), (0, 0), (0, 0), (0, 0)],
        };
        CAPS.iter()
            .enumerate()
            .map(|(i, cap)| {
                let resource = i == 0 && matches!(self, Self::ResourceOnly | Self::BothCallOrder);
                let actor = self.actor_reason()
                    && (i == 0
                        || (i == 2 && matches!(self, Self::Sequential | Self::ForkAll))
                        || (i == 4 && self == Self::BranchShared));
                if resource || actor {
                    not_exercise(
                        cap,
                        if resource {
                            "resource_infeasible"
                        } else {
                            "actor_capacity_infeasible"
                        },
                    )
                } else {
                    exercise(cap, counts[i].0, counts[i].1)
                }
            })
            .collect()
    }
}
fn end(name: &str, reachable: bool, selectors: Value, read_only: Value) -> Value {
    json!({"status":"derived","end":name,"reachable":reachable,"selectors":selectors,"read_only":read_only})
}
fn not_end(name: &str, reason: &str) -> Value {
    json!({"status":"not_derived","end":name,"reason":reason})
}
fn exercise(cap: &str, min: usize, max: usize) -> Value {
    json!({"status":"derived","capability":cap,"normal":{"min":min,"max":max},"contest_route":false,"basis":"unknown","fallback":"unchecked","join_any_cancellation":false})
}
fn not_exercise(cap: &str, reason: &str) -> Value {
    json!({"status":"not_derived","capability":cap,"reason":reason,"contest_route":false,"basis":"unknown","fallback":"unchecked","join_any_cancellation":false})
}
pub fn strict_row(actual: &Value, wanted: &Value) -> Result<(), String> {
    if actual.get("reason") != wanted.get("reason") {
        return Err(format!(
            "reason mismatch: expected {} got {}",
            wanted["reason"], actual["reason"]
        ));
    }
    if actual != wanted {
        return Err(format!("row mismatch: expected {wanted} got {actual}"));
    }
    Ok(())
}
pub fn row<'a>(report: &'a Value, table: &str, key: &str, name: &str) -> &'a Value {
    let member = if table == "framing" {
        "ends"
    } else {
        "capabilities"
    };
    let rows = report["agency"]["viewpoints"][table][member]
        .as_array()
        .expect("required row table");
    let selected: Vec<_> = rows.iter().filter(|v| v[key] == name).collect();
    assert_eq!(selected.len(), 1, "unique {table} {name}");
    selected[0]
}
pub fn unchanged(case: Case, r: &Value, exit: i32) {
    assert_eq!(exit, case.exit(), "{} exit", case.name());
    assert_eq!(
        r["verdict"],
        if case.blocked() {
            "violated"
        } else {
            "satisfied"
        }
    );
    assert_eq!(
        r["bounds"],
        json!({"cases":1,"live_tasks":3,"clock":10,"truncated":false})
    );
    let reached = !case.blocked() || case == Case::BranchShared;
    let mut property =
        json!({"id":"reachable end.done","verdict":if reached {"satisfied"} else {"violated"}});
    if matches!(case, Case::ResourceOnly | Case::BothCallOrder) {
        property["cases"] = json!([{"case_id":1,"arrival":0,"verdict":"violated"}]);
    }
    assert_eq!(
        r["properties"],
        json!([
            {"id":"safety no_resource_overcommit","verdict":"satisfied"}, property
        ])
    );
    let terminals = if reached {
        let t = match case {
            Case::SharedPerson => 3,
            Case::BranchShared => 4,
            _ => 1,
        };
        json!([{"end":"done","elapsed":{"min":t,"max":t},"work":{"min":t,"max":t},"result_at":null}])
    } else {
        json!([])
    };
    assert_eq!(r["terminals"], terminals, "{} terminals", case.name());
    assert_eq!(
        r["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|d| d["code"] == "W-DEADLOCK-0")
            .count(),
        usize::from(case.blocked())
    );
    let people = if case == Case::SharedPerson {
        json!(["p"])
    } else {
        json!([])
    };
    let unattributed = if case == Case::SharedPerson {
        json!([])
    } else {
        case.observed_actors()
    };
    let person_rows = if case == Case::SharedPerson {
        json!([{"person":"p","normal":{"direct":["Doc"],"indirect":[]},"contest":{"direct":[]}}])
    } else {
        json!([])
    };
    assert_eq!(
        r["agency"]["viewpoints"]["observation"],
        json!({
            "status":"derived","sources":[{"actor":"Doc","normal":{"actors":case.observed_actors(),"people":people,"unattributed_actors":unattributed},"contest":{"actors":[],"people":[],"unattributed_actors":[],"routes":[]}}],
            "people":person_rows,"fallback":"unchecked"
        }),
        "{} lexical O",
        case.name()
    );
    assert_eq!(
        r["agency"]["viewpoints"]["proposal"],
        json!({"status":"not_derived","reason":"no_proposal_contract"})
    );
    assert_eq!(r["agency"]["refusal"], json!([]));
    assert_eq!(
        r["agency"]["viewpoints"]["effects"],
        json!({"status":"derived","rows":[],"unchecked":["authority","fallback_path"]})
    );
}
pub fn unaffected(case: Case, r: &Value) {
    let f = &r["agency"]["viewpoints"]["framing"];
    let l = &r["agency"]["viewpoints"]["retention"];
    assert_eq!(f["status"], "derived");
    assert_eq!(l["status"], "derived");
    assert_eq!(
        f["actors"],
        if case == Case::BranchShared {
            json!(["B"])
        } else {
            json!([])
        }
    );
    assert_eq!(
        f["unchecked"],
        json!(["type_domain_authority", "practical_framing"])
    );
    assert_eq!(l["notice"], "行使回数から不要とは判定しない");
    assert_eq!(
        l["unchecked"],
        json!(["fallback_dependency", "retention_basis", "actual_skill"])
    );
    let ends = case.framing_rows();
    assert_eq!(f["ends"].as_array().unwrap().len(), ends.len());
    assert_eq!(l["capabilities"].as_array().unwrap().len(), CAPS.len());
    for (table, key, wanted) in ends.iter().map(|w| ("framing", "end", w)).chain(
        case.retention_rows()
            .iter()
            .map(|w| ("retention", "capability", w)),
    ) {
        if wanted["reason"] == "actor_capacity_infeasible" {
            continue;
        }
        strict_row(row(r, table, key, wanted[key].as_str().unwrap()), wanted).unwrap();
    }
    if !case.actor_reason() {
        assert_eq!(
            f["complete"],
            !matches!(case, Case::ResourceOnly | Case::BothCallOrder)
        );
    }
}
pub fn actor_framing(case: Case, r: &Value) {
    assert!(case.actor_reason());
    for wanted in case
        .framing_rows()
        .iter()
        .filter(|w| w["reason"] == "actor_capacity_infeasible")
    {
        strict_row(
            row(r, "framing", "end", wanted["end"].as_str().unwrap()),
            wanted,
        )
        .unwrap();
    }
    assert_eq!(r["agency"]["viewpoints"]["framing"]["complete"], false);
}
pub fn actor_retention(case: Case, cap: &str, r: &Value) {
    let wanted = case
        .retention_rows()
        .into_iter()
        .find(|w| w["capability"] == cap)
        .unwrap();
    assert_eq!(wanted["reason"], "actor_capacity_infeasible");
    strict_row(row(r, "retention", "capability", cap), &wanted).unwrap();
}

pub fn any_source(capacity: usize) -> String {
    format!(
        r#"model ActorAnyBoundary version "0.2" {{
 type T = enum {{ pass, fail }}
 actor A {{ capacity {capacity}; capability x() -> T {{ duration fixed 1min; result nondet {{ fail }}; cancel cooperative ack_within 1min }} }}
 actor B {{ capacity 1;
   capability y() -> T {{ duration fixed 2min; result nondet {{ pass }}; cancel cooperative ack_within 1min }}
   capability after() -> T {{ duration fixed 1min; result nondet {{ pass }} }}
 }}
 process P {{ fork f {{ x = A.x(); y = B.y() }}; join any f where result == pass tie by [x,y] remaining cancel_then_drain if none -> done; post = B.after(); end done }}
 cut K {{ include [A, B] }}
 bounds {{ cases 1; live_tasks 3; clock 10min }}
 verify safety no_resource_overcommit
 verify reachable end.done
}}
"#
    )
}

pub fn assert_any(capacity: usize, r: &Value, exit: i32) {
    assert_eq!(exit, 0, "{r}");
    assert_eq!(r["verdict"], "satisfied");
    assert_eq!(r["bounds"]["truncated"], false);
    // y matches at t=2 in both models; after takes one more minute. x takes
    // one minute only with capacity 1, and otherwise is cancelled while queued.
    let work = if capacity == 0 { 3 } else { 4 };
    assert_eq!(
        r["terminals"],
        json!([{"end":"done","elapsed":{"min":3,"max":3},
        "work":{"min":work,"max":work},"result_at":{"min":2,"max":2}}])
    );
    let f = &r["agency"]["viewpoints"]["framing"];
    let done = row(r, "framing", "end", "done");
    let reads = if capacity == 0 {
        json!([{"call":"y","capability":"B.y"}])
    } else {
        json!([{"call":"x","capability":"A.x"},{"call":"y","capability":"B.y"}])
    };
    strict_row(done, &end("done", true, json!([]), reads)).unwrap();
    assert_eq!(f["complete"], true);
    assert_eq!(f["actors"], json!([]));
    for (cap, mut wanted) in [
        (
            "A.x",
            if capacity == 0 {
                not_exercise("A.x", ACI)
            } else {
                exercise("A.x", 1, 1)
            },
        ),
        ("B.y", exercise("B.y", 1, 1)),
        ("B.after", exercise("B.after", 1, 1)),
    ] {
        wanted["join_any_cancellation"] = json!(cap != "B.after");
        strict_row(row(r, "retention", "capability", cap), &wanted).unwrap();
    }
}

pub fn priority_source(actor_capacity: usize, resource_capacity: usize) -> String {
    format!(
        r#"model ActorReasonPriority version "0.2" {{
 type T = enum {{ pass }}
 observed case {{ ready: bool = true }}
 resource R capacity {resource_capacity}
 actor A {{ capacity {actor_capacity}; capability x() -> T {{ uses R: 1; duration fixed 1min; result nondet {{ pass }} }} }}
 actor B {{ capacity 1; capability after() -> T {{ duration fixed 1min; result nondet {{ pass }} }} }}
 process P {{ if ready == true {{ x = A.x() }} else {{}} post = B.after(); end done }}
 cut K {{ include [A, B] }}
 arrivals fixed [0min]
 policy fifo tie_by case_id then call_order
 bounds {{ cases 1; live_tasks 3; clock 10min }}
 verify safety no_resource_overcommit
 verify reachable end.done
}}
"#
    )
}

pub fn assert_priority(actor_capacity: usize, resource_capacity: usize, r: &Value, exit: i32) {
    assert_eq!(
        exit,
        i32::from(actor_capacity == 0 || resource_capacity == 0)
    );
    let reason = if resource_capacity == 0 {
        "resource_infeasible"
    } else if actor_capacity == 0 {
        "actor_capacity_infeasible"
    } else {
        "state_guards"
    };
    strict_row(row(r, "framing", "end", "done"), &not_end("done", reason)).unwrap();
    assert_eq!(r["agency"]["viewpoints"]["framing"]["complete"], false);
    strict_row(
        row(r, "retention", "capability", "A.x"),
        &not_exercise("A.x", reason),
    )
    .unwrap();
}

// V1--V4 expectations below are built from the hand tables, not from Reports.
// The public check/CLI is called only on the actual side in the two test targets.
pub const ACI: &str = "actor_capacity_infeasible";
const SG: &str = "state_guards";
const RI: &str = "resource_infeasible";

pub struct BoundaryProbe {
    pub name: String,
    pub source: String,
    pub exit: i32,
    pub ends: Vec<Value>,
    pub counts: Vec<Value>,
    pub framing_actors: Value,
    pub observation: Value,
    pub effects: Value,
    pub refusal: Value,
    lineage: Value,
    lineage_groups: Value,
    terminals: Value,
    ordered: bool,
    reached: bool,
    target: &'static str,
    diagnostics: Vec<(String, String)>,
}
fn empty_observation() -> Value {
    json!({"status":"derived","sources":[],"people":[],"fallback":"unchecked"})
}
fn empty_effects() -> Value {
    json!({"status":"derived","rows":[],"unchecked":["authority","fallback_path"]})
}
fn terminal(name: &str, elapsed: usize, work: usize, adopted: Option<usize>) -> Value {
    json!({"end":name,"elapsed":{"min":elapsed,"max":elapsed},
        "work":{"min":work,"max":work},"result_at":adopted.map(|t| json!({"min":t,"max":t}))})
}
fn warning(code: &str, message: impl Into<String>) -> (String, String) {
    (code.into(), message.into())
}
fn never(cap: &str) -> (String, String) {
    warning("W-UNREACH-0", format!("process P: {cap} never_started"))
}
fn unreachable(name: &str) -> (String, String) {
    warning(
        "W-UNREACH-2",
        format!("process P: end.{name} is never reached"),
    )
}
fn deadlock() -> (String, String) {
    warning(
        "W-DEADLOCK-0",
        "a reachable nonterminal state has no enabled transition",
    )
}
fn then_dead() -> (String, String) {
    warning("W-UNREACH-1", "process P: if then arm is never taken")
}
fn else_dead() -> (String, String) {
    warning("W-UNREACH-1", "process P: if else arm is never taken")
}
impl BoundaryProbe {
    pub fn assert_core(&self, report: &Value, exit: i32) {
        assert_eq!(exit, self.exit, "{} exit", self.name);
        assert_eq!(
            report["verdict"],
            if self.exit == 0 {
                "satisfied"
            } else {
                "violated"
            },
            "{}",
            self.name
        );
        assert_eq!(
            report["bounds"],
            json!({"cases":1,"live_tasks":4,"clock":20,"truncated":false})
        );
        let mut reach = json!({"id":format!("reachable end.{}", self.target), "verdict":if self.reached {"satisfied"} else {"violated"}});
        if self.ordered {
            reach["cases"] = json!([{"case_id":1,"arrival":0,"verdict":if self.reached {"satisfied"} else {"violated"}}]);
        }
        assert_eq!(
            report["properties"],
            json!([{"id":"safety no_resource_overcommit","verdict":"satisfied"},reach]),
            "{} properties",
            self.name
        );
        if self.ordered {
            assert_eq!(report["terminals"], json!([]));
            let outcomes: Vec<_> = self
                .terminals
                .as_array()
                .unwrap()
                .iter()
                .map(|t| {
                    let mut t = t.clone();
                    t["kind"] = json!("end");
                    t["completed_at"] = t["elapsed"].clone();
                    t
                })
                .collect();
            assert_eq!(
                report["case_outcomes"],
                json!([{"case_id":1,"arrival":0,"terminals":outcomes}]),
                "{} case terminals",
                self.name
            );
            assert_eq!(
                report["admission"],
                json!({"rejected_count":{"min":0,"max":0}})
            );
        } else {
            assert_eq!(
                report["terminals"], self.terminals,
                "{} terminals",
                self.name
            );
            assert!(report.get("case_outcomes").is_none());
        }
        let mut actual: Vec<_> = report["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .map(|d| {
                assert_eq!(d["severity"], "warning", "{} diagnostic {d}", self.name);
                (
                    d["code"].as_str().unwrap().to_owned(),
                    d["message"].as_str().unwrap().to_owned(),
                )
            })
            .collect();
        actual.sort();
        let mut wanted = self.diagnostics.clone();
        wanted.sort();
        assert_eq!(actual, wanted, "{} diagnostic multiset", self.name);
        assert_eq!(
            report["agency"]["viewpoints"]["observation"], self.observation,
            "{} O",
            self.name
        );
        assert_eq!(
            report["agency"]["viewpoints"]["proposal"],
            json!({"status":"not_derived","reason":"no_proposal_contract"})
        );
        assert_eq!(
            report["agency"]["viewpoints"]["effects"], self.effects,
            "{} J/U/E",
            self.name
        );
        assert_eq!(
            report["agency"]["refusal"], self.refusal,
            "{} legacy U",
            self.name
        );
        assert_eq!(
            report["agency"]["lineage"], self.lineage,
            "{} lineage",
            self.name
        );
        assert_eq!(
            report["agency"]["lineage_groups"], self.lineage_groups,
            "{} lineage groups",
            self.name
        );
    }
    pub fn framing(&self) -> Value {
        json!({"status":"derived","ends":self.ends,"actors":self.framing_actors,
            "complete":self.ends.iter().all(|r| r["status"] == "derived"),"unchecked":["type_domain_authority","practical_framing"]})
    }
    pub fn retention(&self) -> Value {
        json!({"status":"derived","capabilities":self.counts,"notice":"行使回数から不要とは判定しない",
            "unchecked":["fallback_dependency","retention_basis","actual_skill"]})
    }
    pub fn assert_framing(&self, report: &Value) {
        for wanted in &self.ends {
            strict_row(
                row(report, "framing", "end", wanted["end"].as_str().unwrap()),
                wanted,
            )
            .unwrap_or_else(|e| panic!("{} framing {}: {e}", self.name, wanted["end"]));
        }
        assert_eq!(
            report["agency"]["viewpoints"]["framing"],
            self.framing(),
            "{} F block",
            self.name
        );
    }
    pub fn assert_retention(&self, report: &Value) {
        for wanted in &self.counts {
            strict_row(
                row(
                    report,
                    "retention",
                    "capability",
                    wanted["capability"].as_str().unwrap(),
                ),
                wanted,
            )
            .unwrap_or_else(|e| panic!("{} retention {}: {e}", self.name, wanted["capability"]));
        }
        assert_eq!(
            report["agency"]["viewpoints"]["retention"],
            self.retention(),
            "{} L block",
            self.name
        );
    }
    pub fn assert_all(&self, report: &Value, exit: i32) {
        self.assert_core(report, exit);
        self.assert_framing(report);
        self.assert_retention(report);
    }
}

// Keep the independent input knobs explicit beside the hand table.
#[allow(clippy::too_many_arguments)]
fn base_source(
    body: &str,
    capacity: usize,
    xresult: &str,
    yresult: &str,
    observed: &str,
    requires: &str,
    ordered: bool,
    resource: bool,
) -> String {
    let declarations = if resource {
        "resource R capacity 1"
    } else {
        ""
    };
    let xuse = if resource { "uses R:2;" } else { "" };
    let yuse = if resource { "uses R:1;" } else { "" };
    let policy = if ordered {
        "arrivals fixed [0min]\n policy fifo tie_by case_id then call_order"
    } else {
        ""
    };
    format!(
        r#"model ActorViewpointBoundary version "0.2" {{
 type T = enum {{ pass, fail }}
 {observed}
 {declarations}
 actor A {{ capacity {capacity}; capability x() -> T {{ {xuse} {requires} duration fixed 1min; result nondet {{ {xresult} }}; cancel cooperative ack_within 1min }} }}
 actor B {{ capacity 1;
  capability y() -> T {{ {yuse} duration fixed 2min; result nondet {{ {yresult} }}; cancel cooperative ack_within 1min }}
  capability after() -> T {{ duration fixed 1min; result nondet {{ pass }} }}
 }}
 process P {{ {body} }}
 cut K {{ include [A,B] }}
 {policy}
 bounds {{ cases 1; live_tasks 4; clock 20min }}
 verify safety no_resource_overcommit
 verify reachable end.done
}}
"#
    )
}
#[allow(clippy::too_many_arguments)]
fn probe(
    name: String,
    source: String,
    exit: i32,
    reached: bool,
    ordered: bool,
    terminals: Vec<Value>,
    ends: Vec<Value>,
    counts: Vec<Value>,
    diagnostics: Vec<(String, String)>,
) -> BoundaryProbe {
    BoundaryProbe {
        name,
        source,
        exit,
        reached,
        ordered,
        terminals: json!(terminals),
        ends,
        counts,
        diagnostics,
        framing_actors: json!([]),
        observation: empty_observation(),
        effects: empty_effects(),
        refusal: json!([]),
        lineage: json!([]),
        lineage_groups: json!([]),
        target: "done",
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ForkKind {
    Match,
    Variable,
    Drain,
    NoMatch,
    ZeroOnly,
    All,
    State,
    Requires,
    ResourceOrder,
}
pub fn fork_probe(kind: ForkKind, capacity: usize, ordered: bool, reverse: bool) -> BoundaryProbe {
    let all = kind == ForkKind::All;
    let resource = kind == ForkKind::ResourceOrder;
    assert!(!resource || ordered);
    let xresult = if kind == ForkKind::ZeroOnly {
        "pass"
    } else {
        "fail"
    };
    let yresult = match kind {
        ForkKind::Variable => "pass, fail",
        ForkKind::NoMatch | ForkKind::ZeroOnly => "fail",
        _ => "pass",
    };
    let branches = if reverse {
        "y = B.y(); x = A.x()"
    } else {
        "x = A.x(); y = B.y()"
    };
    let remaining = if kind == ForkKind::Drain {
        "drain"
    } else {
        "cancel_then_drain"
    };
    let join = if all {
        "join all f".into()
    } else {
        format!(
            "join any f where result == pass tie by [x,y] remaining {remaining} if none -> no_match"
        )
    };
    let prefix = if kind == ForkKind::State {
        "if ready == true {} else {};"
    } else {
        ""
    };
    let tail = if all { "" } else { "; end no_match" };
    let body =
        format!("{prefix} fork f {{ {branches} }}; {join}; post = B.after(); end done{tail}");
    let observed = if matches!(kind, ForkKind::State | ForkKind::Requires) {
        "observed case { ready: bool = false }"
    } else {
        ""
    };
    let requires = if kind == ForkKind::Requires {
        "requires ready == true else fail;"
    } else {
        ""
    };
    let source = base_source(
        &body, capacity, xresult, yresult, observed, requires, ordered, resource,
    );
    let name = format!("v-fork-{kind:?}-cap{capacity}-order{ordered}-reverse{reverse}");
    let precise = capacity == 0 && matches!(kind, ForkKind::Match | ForkKind::Variable);
    let positive = capacity == 1 && matches!(kind, ForkKind::Match | ForkKind::All);
    let reached = precise || positive || matches!(kind, ForkKind::State | ForkKind::Requires);
    let blocked = !reached || kind == ForkKind::Variable;
    let mut diagnostics = Vec::new();
    if blocked {
        diagnostics.push(deadlock());
    }
    if capacity == 0 {
        diagnostics.push(never("A.x"));
    }
    if resource {
        diagnostics.push(never("B.y"));
    }
    if !reached {
        diagnostics.push(never("B.after"));
    }
    if kind == ForkKind::State {
        diagnostics.push(then_dead());
    }
    if !all {
        diagnostics.push(unreachable("no_match"));
    }
    if !reached {
        diagnostics.push(unreachable("done"));
    }
    let terminals = if reached {
        vec![terminal(
            "done",
            3,
            if capacity == 0 { 3 } else { 4 },
            if all { None } else { Some(2) },
        )]
    } else {
        vec![]
    };
    let mut counts = vec![
        if capacity == 0 {
            not_exercise("A.x", if resource { RI } else { ACI })
        } else {
            exercise("A.x", 1, 1)
        },
        if resource {
            not_exercise("B.y", RI)
        } else {
            exercise("B.y", 1, 1)
        },
        if resource {
            not_exercise("B.after", RI)
        } else if kind == ForkKind::NoMatch {
            exercise("B.after", 0, 0)
        } else if kind == ForkKind::Variable {
            exercise("B.after", 0, 1)
        } else if precise || positive {
            exercise("B.after", 1, 1)
        } else {
            not_exercise("B.after", ACI)
        },
    ];
    for (i, r) in counts.iter_mut().enumerate() {
        r["join_any_cancellation"] = json!(
            !all && kind != ForkKind::Drain && i < 2 && !(i == 0 && kind == ForkKind::Requires)
        );
    }
    let mut reads = vec![];
    if positive {
        reads.push(json!({"call":"x","capability":"A.x"}));
    }
    reads.push(json!({"call":"y","capability":"B.y"}));
    let done = if resource {
        not_end("done", RI)
    } else if kind == ForkKind::NoMatch {
        end("done", false, json!([]), json!([]))
    } else if kind == ForkKind::Variable {
        end(
            "done",
            true,
            json!([{"call":"y","capability":"B.y","selects":"all_contexts","blocked":true}]),
            json!([]),
        )
    } else if precise || positive {
        end(
            "done",
            true,
            json!([]),
            if all { json!([]) } else { json!(reads) },
        )
    } else {
        not_end("done", ACI)
    };
    let mut ends = vec![done];
    if !all {
        ends.push(if resource {
            not_end("no_match", RI)
        } else if kind == ForkKind::NoMatch {
            not_end("no_match", ACI)
        } else if kind == ForkKind::State {
            not_end("no_match", SG)
        } else {
            end("no_match", false, json!([]), json!([]))
        });
    }
    let mut p = probe(
        name,
        source,
        i32::from(blocked),
        reached,
        ordered,
        terminals,
        ends,
        counts,
        diagnostics,
    );
    if kind == ForkKind::Variable {
        p.framing_actors = json!(["B"]);
    }
    p
}
pub fn conservative_forks() -> Vec<BoundaryProbe> {
    [
        ForkKind::Drain,
        ForkKind::NoMatch,
        ForkKind::ZeroOnly,
        ForkKind::All,
        ForkKind::State,
        ForkKind::Requires,
        ForkKind::ResourceOrder,
    ]
    .into_iter()
    .map(|k| fork_probe(k, 0, k == ForkKind::ResourceOrder, false))
    .collect()
}

// Review follow-ups: independent hand expectations are in u12_boundary_calculations.md.
fn with_c(source: &str, capacity: usize, capability: &str) -> String {
    source
        .replacen(
            " process P",
            &format!(
                " actor C {{ capacity {capacity}; capability {capability}() -> T {{ duration fixed 1min; result nondet {{ pass }}; cancel cooperative ack_within 1min }} }}\n process P"
            ),
            1,
        )
        .replacen("include [A,B]", "include [A,B,C]", 1)
}
fn cancellation(mut count: Value) -> Value {
    count["join_any_cancellation"] = json!(true);
    count
}
pub fn sequential_after_refinement(capacity: usize) -> BoundaryProbe {
    let body = "fork f { x = A.x(); y = B.y() }; join any f where result == pass tie by [x,y] remaining cancel_then_drain if none -> no_match; post = B.after(); if post == pass { end immediate } else {}; stop = C.stop(); tail = B.tail(); end done; end no_match";
    let source = with_c(
        &base_source(body, 0, "fail", "pass", "", "", false, false)
            .replacen(
                "capability after() -> T { duration fixed 1min; result nondet { pass } }",
                "capability after() -> T { duration fixed 1min; result nondet { pass, fail } }\n  capability tail() -> T { duration fixed 1min; result nondet { pass } }",
                1,
            ),
        capacity,
        "stop",
    );
    let selectors = json!([{"call":"post","capability":"B.after","selects":"all_contexts"}]);
    let reads = json!([{"call":"y","capability":"B.y"}]);
    let mut diagnostics = vec![never("A.x"), unreachable("no_match")];
    let mut terminals = vec![terminal("immediate", 3, 3, Some(2))];
    if capacity == 0 {
        diagnostics.extend([
            deadlock(),
            never("C.stop"),
            never("B.tail"),
            unreachable("done"),
        ]);
    } else {
        terminals.push(terminal("done", 5, 5, Some(2)));
    }
    let mut p = probe(
        format!("v-follow-sequential-cap{capacity}"),
        source,
        i32::from(capacity == 0),
        capacity == 1,
        false,
        terminals,
        vec![
            end("immediate", true, selectors.clone(), reads.clone()),
            if capacity == 0 {
                not_end("done", ACI)
            } else {
                end("done", true, selectors, reads)
            },
            end("no_match", false, json!([]), json!([])),
        ],
        vec![
            cancellation(not_exercise("A.x", ACI)),
            cancellation(exercise("B.y", 1, 1)),
            exercise("B.after", 1, 1),
            if capacity == 0 {
                not_exercise("B.tail", ACI)
            } else {
                exercise("B.tail", 0, 1)
            },
            if capacity == 0 {
                not_exercise("C.stop", ACI)
            } else {
                exercise("C.stop", 0, 1)
            },
        ],
        diagnostics,
    );
    p.framing_actors = json!(["B"]);
    p
}
pub fn three_siblings(capacity: usize) -> BoundaryProbe {
    let mut p = fork_probe(ForkKind::Match, 0, false, false);
    p.name = format!("v-follow-three-siblings-cap{capacity}");
    p.source = with_c(&p.source, capacity, "z")
        .replacen("x = A.x(); y = B.y()", "x = A.x(); y = B.y(); z = C.z()", 1)
        .replacen("tie by [x,y]", "tie by [x,y,z]", 1);
    p.counts.push(cancellation(if capacity == 0 {
        not_exercise("C.z", ACI)
    } else {
        exercise("C.z", 1, 1)
    }));
    if capacity == 0 {
        p.diagnostics.push(never("C.z"));
    } else {
        p.terminals = json!([terminal("done", 3, 4, Some(1))]);
        p.ends[0] = end(
            "done",
            true,
            json!([]),
            json!([{"call":"y","capability":"B.y"},{"call":"z","capability":"C.z"}]),
        );
    }
    p
}
pub fn fork_in_if(drain: bool) -> BoundaryProbe {
    let remaining = if drain { "drain" } else { "cancel_then_drain" };
    let body = format!(
        "v = B.select(); if v == pass {{ fork f {{ x = A.x(); y = B.y() }}; join any f where result == pass tie by [x,y] remaining {remaining} if none -> no_match; post = B.after(); end done }} else {{ alt = B.other(); end other }}; end no_match"
    );
    let source = base_source(&body, 0, "fail", "pass", "", "", false, false).replacen(
        "actor B { capacity 1;",
        "actor B { capacity 1;\n  capability select() -> T { duration fixed 1min; result nondet { pass, fail } }\n  capability other() -> T { duration fixed 1min; result nondet { pass } }",
        1,
    );
    let selectors = json!([{"call":"v","capability":"B.select","selects":"all_contexts"}]);
    let mut terminals = if drain {
        vec![]
    } else {
        vec![terminal("done", 4, 4, Some(3))]
    };
    terminals.push(terminal("other", 2, 2, None));
    let mut diagnostics = vec![never("A.x"), unreachable("no_match")];
    if drain {
        diagnostics.extend([deadlock(), never("B.after"), unreachable("done")]);
    }
    let mut p = probe(
        format!("v-follow-if-drain{drain}"),
        source,
        i32::from(drain),
        !drain,
        false,
        terminals,
        vec![
            if drain {
                not_end("done", ACI)
            } else {
                end(
                    "done",
                    true,
                    selectors.clone(),
                    json!([{"call":"y","capability":"B.y"}]),
                )
            },
            end("other", true, selectors, json!([])),
            end("no_match", false, json!([]), json!([])),
        ],
        vec![
            if drain {
                not_exercise("A.x", ACI)
            } else {
                cancellation(not_exercise("A.x", ACI))
            },
            exercise("B.select", 1, 1),
            exercise("B.other", 0, 1),
            if drain {
                exercise("B.y", 0, 1)
            } else {
                cancellation(exercise("B.y", 0, 1))
            },
            if drain {
                not_exercise("B.after", ACI)
            } else {
                exercise("B.after", 0, 1)
            },
        ],
        diagnostics,
    );
    p.framing_actors = json!(["B"]);
    p
}
pub fn fallback_zero_duration(zero_duration: bool) -> BoundaryProbe {
    let mut p = fork_probe(ForkKind::Match, 0, false, false);
    p.name = format!("v-follow-duration-zero{zero_duration}");
    if zero_duration {
        p.source = p
            .source
            .replacen("duration fixed 1min", "duration fixed 0min", 1);
        p.ends[0] = not_end("done", ACI);
        p.counts[2] = not_exercise("B.after", ACI);
    }
    p
}
pub fn fallback_unused_requires(requires: bool) -> BoundaryProbe {
    let mut p = fork_probe(ForkKind::Match, 0, false, false);
    p.name = format!("v-follow-unused-requires{requires}");
    let guard = if requires {
        "requires ready == true else fail; "
    } else {
        ""
    };
    p.source = p.source
        .replacen("type T = enum { pass, fail }", "type T = enum { pass, fail }\n observed case { ready: bool = true }", 1)
        .replacen("actor B { capacity 1;", &format!("actor B {{ capacity 1;\n  capability unused() -> T {{ {guard}duration fixed 1min; result nondet {{ pass }} }}"), 1);
    p.counts.insert(1, exercise("B.unused", 0, 0));
    p.diagnostics
        .push(warning("W-UNREACH-0", "process P: B.unused not_called"));
    if requires {
        p.ends[0] = not_end("done", ACI);
        p.counts[3] = not_exercise("B.after", ACI);
    }
    p
}
pub fn followup_controls() {
    for (zero, one, old, new) in [
        (
            sequential_after_refinement(0),
            sequential_after_refinement(1),
            "actor C { capacity 1",
            "actor C { capacity 0",
        ),
        (
            three_siblings(0),
            three_siblings(1),
            "actor C { capacity 1",
            "actor C { capacity 0",
        ),
        (
            fork_in_if(true),
            fork_in_if(false),
            "remaining cancel_then_drain",
            "remaining drain",
        ),
        (
            fallback_zero_duration(true),
            fallback_zero_duration(false),
            "duration fixed 1min",
            "duration fixed 0min",
        ),
        (
            fallback_unused_requires(true),
            fallback_unused_requires(false),
            "capability unused() -> T { ",
            "capability unused() -> T { requires ready == true else fail; ",
        ),
    ] {
        assert_ne!(zero.source, one.source);
        assert_eq!(zero.source, one.source.replacen(old, new, 1));
    }
    let sequential = sequential_after_refinement(0);
    let three = three_siblings(0);
    let arm = fork_in_if(false);
    let fallback = fallback_zero_duration(true);
    let unused = fallback_unused_requires(true);
    for (wanted, broken) in [
        (sequential.ends[0].clone(), not_end("immediate", ACI)),
        (
            sequential.ends[1].clone(),
            end("done", true, json!([]), json!([])),
        ),
        (sequential.counts[3].clone(), exercise("B.tail", 0, 1)),
        (
            three.ends[0].clone(),
            end(
                "done",
                true,
                json!([]),
                json!([{"call":"y","capability":"B.y"},{"call":"z","capability":"C.z"}]),
            ),
        ),
        (arm.ends[1].clone(), not_end("other", ACI)),
        (arm.counts[2].clone(), not_exercise("B.other", ACI)),
        (
            fallback.ends[0].clone(),
            end("done", true, json!([]), json!([])),
        ),
        (unused.counts[3].clone(), exercise("B.after", 1, 1)),
    ] {
        strict_row(&wanted, &wanted).unwrap();
        assert!(strict_row(&broken, &wanted).is_err());
    }
}

#[derive(Clone, Copy, Debug)]
pub enum StateKind {
    Guard,
    BindingDead,
    BindingPossible,
    MixedDead,
    NestedDead,
    IndependentSuffix,
}
pub fn state_probe(kind: StateKind, ready: bool, capacity: usize) -> BoundaryProbe {
    let body = match kind {
        StateKind::Guard => "if ready == true { x = A.x() } else {}; post = B.after(); end done",
        StateKind::BindingDead | StateKind::BindingPossible => {
            "v = B.y(); if v == fail { x = A.x() } else {}; post = B.after(); end done"
        }
        StateKind::MixedDead => {
            "v = B.y(); if ready == true && v == fail { x = A.x() } else {}; post = B.after(); end done"
        }
        StateKind::NestedDead => {
            "v = B.y(); if ready == true { if v == fail { x = A.x() } else {} } else {}; post = B.after(); end done"
        }
        StateKind::IndependentSuffix => {
            "if ready == true { x = A.x(); end blocked } else {}; post = B.after(); end done"
        }
    };
    let observed = format!("observed case {{ ready: bool = {ready} }}");
    let source = base_source(
        body,
        capacity,
        "fail",
        if matches!(kind, StateKind::BindingPossible) {
            "pass, fail"
        } else {
            "pass"
        },
        &observed,
        "",
        false,
        false,
    );
    let blocked = (matches!(kind, StateKind::Guard) && ready && capacity == 0)
        || matches!(kind, StateKind::BindingPossible);
    let reached = !blocked || matches!(kind, StateKind::BindingPossible);
    let called_y = matches!(
        kind,
        StateKind::BindingDead
            | StateKind::BindingPossible
            | StateKind::MixedDead
            | StateKind::NestedDead
    );
    let mut diagnostics = if blocked { vec![deadlock()] } else { vec![] };
    if capacity == 0 || !ready {
        diagnostics.push(never("A.x"));
    }
    if !called_y {
        diagnostics.push(warning("W-UNREACH-0", "process P: B.y not_called"));
    }
    if !reached {
        diagnostics.push(never("B.after"));
    }
    if matches!(kind, StateKind::BindingPossible) {
        // Both branches are taken; fail blocks at x, pass reaches the suffix.
    } else if ready {
        diagnostics.push(else_dead());
    } else {
        diagnostics.push(then_dead());
    }
    if matches!(kind, StateKind::NestedDead) {
        diagnostics.extend([then_dead(), else_dead()]);
    }
    if matches!(kind, StateKind::IndependentSuffix) {
        diagnostics.push(unreachable("blocked"));
    }
    if !reached {
        diagnostics.push(unreachable("done"));
    }
    let (x, after, done) = match kind {
        StateKind::Guard if capacity == 0 => (
            not_exercise("A.x", ACI),
            not_exercise("B.after", ACI),
            not_end("done", ACI),
        ),
        StateKind::Guard => (
            not_exercise("A.x", SG),
            exercise("B.after", 1, 1),
            not_end("done", SG),
        ),
        StateKind::BindingDead => (
            exercise("A.x", 0, 0),
            exercise("B.after", 1, 1),
            end(
                "done",
                true,
                json!([]),
                json!([{"call":"v","capability":"B.y"}]),
            ),
        ),
        StateKind::MixedDead | StateKind::BindingPossible => (
            not_exercise("A.x", ACI),
            not_exercise("B.after", ACI),
            not_end("done", ACI),
        ),
        StateKind::NestedDead => (
            exercise("A.x", 0, 0),
            exercise("B.after", 1, 1),
            not_end("done", SG),
        ),
        StateKind::IndependentSuffix => (
            not_exercise("A.x", ACI),
            not_exercise("B.after", SG),
            not_end("done", SG),
        ),
    };
    let ends = if matches!(kind, StateKind::IndependentSuffix) {
        vec![not_end("blocked", ACI), done]
    } else {
        vec![done]
    };
    let t = if called_y {
        3
    } else if ready && capacity == 1 {
        2
    } else {
        1
    };
    probe(
        format!("v-state-{kind:?}-{ready}-cap{capacity}"),
        source,
        i32::from(blocked),
        reached,
        false,
        if !reached {
            vec![]
        } else {
            vec![terminal("done", t, t, None)]
        },
        ends,
        vec![
            x,
            exercise("B.y", usize::from(called_y), usize::from(called_y)),
            after,
        ],
        diagnostics,
    )
}
pub fn state_probes() -> Vec<BoundaryProbe> {
    let mut out: Vec<_> = [false, true]
        .into_iter()
        .flat_map(|ready| {
            [0, 1]
                .into_iter()
                .map(move |c| state_probe(StateKind::Guard, ready, c))
        })
        .collect();
    out.extend(
        [
            StateKind::BindingDead,
            StateKind::BindingPossible,
            StateKind::MixedDead,
            StateKind::NestedDead,
            StateKind::IndependentSuffix,
        ]
        .into_iter()
        .map(|k| state_probe(k, false, 0)),
    );
    out
}
pub fn ordered_sequential(capacity: usize) -> BoundaryProbe {
    let body = "pre = B.y(); x = A.x(); post = B.after(); end done";
    let source = base_source(body, capacity, "fail", "pass", "", "", true, false);
    let diagnostics = if capacity == 0 {
        vec![
            deadlock(),
            never("A.x"),
            never("B.after"),
            unreachable("done"),
        ]
    } else {
        vec![]
    };
    probe(
        format!("v-ordered-sequential-cap{capacity}"),
        source,
        i32::from(capacity == 0),
        capacity == 1,
        true,
        if capacity == 0 {
            vec![]
        } else {
            vec![terminal("done", 4, 4, None)]
        },
        vec![if capacity == 0 {
            not_end("done", ACI)
        } else {
            end("done", true, json!([]), json!([]))
        }],
        vec![
            if capacity == 0 {
                not_exercise("A.x", ACI)
            } else {
                exercise("A.x", 1, 1)
            },
            exercise("B.y", 1, 1),
            if capacity == 0 {
                not_exercise("B.after", ACI)
            } else {
                exercise("B.after", 1, 1)
            },
        ],
        diagnostics,
    )
}

pub fn ordered_same_actor() -> BoundaryProbe {
    let body = "fork f { x = A.x(); z = A.z() }; join all f; post = B.after(); end done";
    let source = base_source(body, 0, "fail", "pass", "", "", true, false).replacen(
        "actor A { capacity 0;",
        "actor A { capacity 0; capability z() -> T { duration fixed 1min; result nondet { pass } }",
        1,
    );
    probe(
        "v-ordered-same-actor".into(),
        source,
        1,
        false,
        true,
        vec![],
        vec![not_end("done", ACI)],
        vec![
            not_exercise("A.z", ACI),
            not_exercise("A.x", ACI),
            exercise("B.y", 0, 0),
            not_exercise("B.after", ACI),
        ],
        vec![
            deadlock(),
            never("A.z"),
            never("A.x"),
            warning("W-UNREACH-0", "process P: B.y not_called"),
            never("B.after"),
            unreachable("done"),
        ],
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JueKind {
    Base,
    ZeroClerk,
    ZeroApprover,
    ZeroLedger,
    NoGuard,
    DeadEffect,
    Atomic,
    AtomicZero,
    StateWriteOnly,
    StateWriteOnlyZero,
}
fn judge(stops: &str, bypasses: usize) -> Value {
    json!({"call":"v","capability":"Approver.approve","actors":["Approver"],"people":[],"unattributed_actors":["Approver"],"permitted":[],"stops":stops,"bypasses":bypasses})
}
pub fn jue_probe(kind: JueKind) -> BoundaryProbe {
    let atomic = matches!(
        kind,
        JueKind::Atomic
            | JueKind::AtomicZero
            | JueKind::StateWriteOnly
            | JueKind::StateWriteOnlyZero
    );
    let state_only = matches!(kind, JueKind::StateWriteOnly | JueKind::StateWriteOnlyZero);
    let zero_clerk = kind == JueKind::ZeroClerk;
    let zero_judge = kind == JueKind::ZeroApprover;
    let zero_ledger = matches!(
        kind,
        JueKind::ZeroLedger | JueKind::NoGuard | JueKind::AtomicZero | JueKind::StateWriteOnlyZero
    );
    let dead = kind == JueKind::DeadEffect;
    let no_guard = kind == JueKind::NoGuard;
    let commits = if state_only { "" } else { "commits;" };
    let ledger_body = if atomic {
        format!(
            "{commits} writes keys; duration fixed 1min; atomic {{ if key in keys {{ result rejected }} else {{ keys += key; result committed }} }}"
        )
    } else {
        "commits; duration fixed 1min; result nondet { committed }".into()
    };
    let ledger_state = if atomic {
        "state keys: set<CaseId>;"
    } else {
        ""
    };
    let key_param = if atomic { ", key: CaseId" } else { "" };
    let key_arg = if atomic { ", case.id" } else { "" };
    let ack = if atomic {
        "committed, rejected"
    } else {
        "committed"
    };
    let results = if dead { "pass" } else { "pass, fail" };
    let guard = if no_guard {
        ""
    } else if dead {
        "if v == pass { end returned } else {};"
    } else {
        "if v == pass {} else { end returned };"
    };
    let source = format!(
        r#"model NonemptyActorBoundary version "0.2" {{
 type Entry = enum {{ entry }}
 type V = enum {{ pass, fail }}
 type Ack = enum {{ {ack} }}
 actor Doc {{ source document }}
 actor Clerk {{ capacity {}; capability enter() -> Entry {{ reads [Doc]; duration fixed 1min; result nondet {{ entry }} }} }}
 actor Approver {{ capacity {}; capability approve(e: Entry) -> V {{ duration fixed 1min; result nondet {{ {results} }} }} }}
 actor Ledger {{ capacity {}; {ledger_state} capability post(v: V{key_param}) -> Ack {{ {ledger_body} }} }}
 process P {{ e = Clerk.enter(); v = Approver.approve(e); {guard} k = Ledger.post(v{key_arg}); end posted }}
 cut K {{ include [Doc,Clerk,Approver,Ledger] }}
 bounds {{ cases 1; live_tasks 4; clock 20min }}
 verify safety no_resource_overcommit
 verify reachable end.posted
}}
"#,
        usize::from(!zero_clerk),
        usize::from(!zero_judge),
        usize::from(!zero_ledger)
    );
    let blocked = zero_clerk || zero_judge || zero_ledger;
    let mut diagnostics = Vec::new();
    if blocked {
        diagnostics.push(deadlock());
    }
    for (cap, missing) in [
        ("Clerk.enter", zero_clerk),
        ("Approver.approve", zero_clerk || zero_judge),
        ("Ledger.post", blocked || dead),
    ] {
        if missing {
            diagnostics.push(never(cap));
        }
    }
    if dead {
        diagnostics.push(else_dead());
    }
    if !no_guard && (zero_clerk || zero_judge) {
        diagnostics.extend([then_dead(), else_dead()]);
        diagnostics.push(unreachable("returned"));
    }
    if blocked || dead {
        diagnostics.push(unreachable("posted"));
    }
    if !state_only {
        diagnostics.push(warning("W-RESP-0", "Ledger has no declared responsibility"));
    }
    if !state_only && !dead && !no_guard {
        diagnostics.push(warning(
            "W-RESP-0",
            "Approver has no declared responsibility",
        ));
    }
    if no_guard {
        diagnostics.push(warning(
            "W-AGY-0",
            "Ledger.post judgement v does not stop the effect (bypasses: 1)",
        ));
    }
    let selector = json!([{"call":"v","capability":"Approver.approve","selects":"all_contexts"}]);
    let mut ends = Vec::new();
    if !no_guard {
        ends.push(if zero_clerk || zero_judge {
            not_end("returned", ACI)
        } else if dead {
            end(
                "returned",
                true,
                json!([]),
                json!([{"call":"v","capability":"Approver.approve"}]),
            )
        } else {
            end("returned", true, selector.clone(), json!([]))
        });
    }
    ends.push(if blocked {
        not_end("posted", ACI)
    } else if dead {
        end("posted", false, json!([]), json!([]))
    } else {
        end("posted", true, selector, json!([]))
    });
    let terminals = if zero_clerk || zero_judge || no_guard {
        vec![]
    } else if zero_ledger || dead {
        vec![terminal("returned", 2, 2, None)]
    } else {
        vec![
            terminal("returned", 2, 2, None),
            terminal("posted", 3, 3, None),
        ]
    };
    let mut p = probe(
        format!("v-jue-{kind:?}"),
        source,
        i32::from(blocked || dead),
        !blocked && !dead,
        false,
        terminals,
        ends,
        vec![
            if zero_clerk {
                not_exercise("Clerk.enter", ACI)
            } else {
                exercise("Clerk.enter", 1, 1)
            },
            if zero_clerk || zero_judge {
                not_exercise("Approver.approve", ACI)
            } else {
                exercise("Approver.approve", 1, 1)
            },
            if blocked {
                not_exercise("Ledger.post", ACI)
            } else if dead {
                exercise("Ledger.post", 0, 0)
            } else {
                exercise("Ledger.post", 0, 1)
            },
        ],
        diagnostics,
    );
    p.target = "posted";
    if !zero_clerk && !zero_judge && !dead && !no_guard {
        p.framing_actors = json!(["Approver"]);
    }
    p.observation = json!({"status":"derived","sources":[{"actor":"Doc","normal":{"actors":["Clerk"],"people":[],"unattributed_actors":["Clerk"]},"contest":{"actors":[],"people":[],"unattributed_actors":[],"routes":[]}}],"people":[],"fallback":"unchecked"});
    let (stops, bypasses) = if no_guard {
        ("none", 1)
    } else {
        ("all_paths", 0)
    };
    let js = if dead {
        json!([])
    } else {
        json!([judge(stops, bypasses)])
    };
    let us = if dead || no_guard {
        json!([])
    } else {
        js.clone()
    };
    p.refusal = if atomic {
        json!([])
    } else {
        json!([{"effect":"Ledger.post","call":"k","reachable":!dead,"judgements":if dead {json!([])} else {json!([{"call":"v","capability":"Approver.approve","stops":stops,"bypasses":bypasses}])}}])
    };
    if !atomic {
        // v's input descends from Doc; the judge reads no independent source.
        // This lexical lineage remains even when the post's result guard dies.
        p.lineage = json!([{"call":"v","capability":"Approver.approve","subject_sources":["Doc"],"independent_sources":[]}]);
        p.lineage_groups =
            json!([{"subject_sources":["Doc"],"judgements":1,"actors":1,"independent_sources":[]}]);
    }
    p.effects = if state_only {
        json!({"status":"derived","rows":[],"unchecked":["authority","fallback_path","state_write_effects"]})
    } else {
        json!({"status":"derived","rows":[{
        "effect":"Ledger.post","call":"k","status":"derived",
        "e":{"actors":["Ledger"],"people":[],"unattributed_actors":["Ledger"],"permitted":[]},
        "j":js,"u":us,"state_refusers":if atomic {json!([{"actor":"Ledger","capability":"Ledger.post","reads":["Ledger.keys"],"on_normal_path":false}])} else {json!([])},
        "process_judgements":"derived","concentration":{
            "single_actor":false,"single_person":"unknown","single_generation_root":!dead,
            "generation_roots":if dead {json!([])} else {json!([{"call":"e","capability":"Clerk.enter"}])},
            "signal":"判定・拒否・実行の担い手と生成根の集中の兆候",
            "mechanism":"同じ担い手又は生成根への依存が構成上重なる",
            "check":"モデル外の統制・原本参照・ログで確認する"}
    }],"unchecked":["authority","fallback_path"]})
    };
    p
}
pub fn jue_probes() -> Vec<BoundaryProbe> {
    [
        JueKind::Base,
        JueKind::ZeroClerk,
        JueKind::ZeroApprover,
        JueKind::ZeroLedger,
        JueKind::NoGuard,
        JueKind::DeadEffect,
        JueKind::Atomic,
        JueKind::AtomicZero,
        JueKind::StateWriteOnly,
        JueKind::StateWriteOnlyZero,
    ]
    .into_iter()
    .map(jue_probe)
    .collect()
}

pub fn comparator_controls() {
    let precision = fork_probe(ForkKind::Variable, 0, false, false);
    let f = precision.framing();
    let l = precision.retention();
    let effects = jue_probe(JueKind::ZeroLedger).effects;
    for wanted in [&f, &l, &effects] {
        strict_row(wanted, wanted).unwrap();
    }
    let mut broken = f.clone();
    broken["ends"][0] = not_end("done", ACI);
    assert!(strict_row(&broken, &f).is_err());
    broken = f.clone();
    broken["actors"] = json!([]);
    assert!(strict_row(&broken, &f).is_err());
    broken = f.clone();
    broken["ends"][0]["selectors"][0]["call"] = json!("x");
    assert!(strict_row(&broken, &f).is_err());
    broken = f.clone();
    broken["ends"][1]["reachable"] = json!(true);
    assert!(strict_row(&broken, &f).is_err());
    broken = l.clone();
    broken["capabilities"][0]["normal"] = json!({"min":0,"max":0});
    assert!(strict_row(&broken, &l).is_err());
    broken = l.clone();
    broken["capabilities"][0]["join_any_cancellation"] = json!(false);
    assert!(strict_row(&broken, &l).is_err());
    broken = l.clone();
    broken["capabilities"][2]["normal"]["min"] = json!(1);
    assert!(strict_row(&broken, &l).is_err());
    broken = effects.clone();
    broken["rows"][0]["process_judgements"] = json!({"status":"not_derived","reason":ACI});
    assert!(strict_row(&broken, &effects).is_err());
    for field in ["e", "j", "u"] {
        broken = effects.clone();
        broken["rows"][0].as_object_mut().unwrap().remove(field);
        assert!(strict_row(&broken, &effects).is_err());
    }
    broken = effects.clone();
    broken["rows"][0]["u"] = json!([]);
    assert!(strict_row(&broken, &effects).is_err());
    broken = effects.clone();
    broken["rows"][0]["concentration"]["generation_roots"][0]["call"] = json!("v");
    assert!(strict_row(&broken, &effects).is_err());
}

pub fn edit_witnesses() {
    let dead = state_probe(StateKind::BindingDead, false, 0);
    let possible = state_probe(StateKind::BindingPossible, false, 0);
    assert_eq!(
        possible.source,
        dead.source.replacen(
            "duration fixed 2min; result nondet { pass }",
            "duration fixed 2min; result nondet { pass, fail }",
            1
        )
    );
    let base = fork_probe(ForkKind::Match, 0, false, false).source;
    assert_eq!(
        fork_probe(ForkKind::Drain, 0, false, false).source,
        base.replacen("cancel_then_drain", "drain", 1)
    );
    assert_eq!(
        fork_probe(ForkKind::NoMatch, 0, false, false).source,
        base.replacen(
            "duration fixed 2min; result nondet { pass }",
            "duration fixed 2min; result nondet { fail }",
            1
        )
    );
    assert_eq!(
        fork_probe(ForkKind::Match, 0, true, false).source,
        base.replacen(
            " \n bounds",
            " arrivals fixed [0min]\n policy fifo tie_by case_id then call_order\n bounds",
            1
        )
    );
    assert_eq!(
        fork_probe(ForkKind::Match, 0, true, true).source,
        fork_probe(ForkKind::Match, 0, true, false).source.replacen(
            "x = A.x(); y = B.y()",
            "y = B.y(); x = A.x()",
            1
        )
    );
    let jue = jue_probe(JueKind::Base).source;
    for (actor, kind) in [
        ("Clerk", JueKind::ZeroClerk),
        ("Approver", JueKind::ZeroApprover),
        ("Ledger", JueKind::ZeroLedger),
    ] {
        assert_eq!(
            jue_probe(kind).source,
            jue.replacen(
                &format!("actor {actor} {{ capacity 1"),
                &format!("actor {actor} {{ capacity 0"),
                1
            )
        );
    }
    assert_eq!(
        jue_probe(JueKind::NoGuard).source,
        jue_probe(JueKind::ZeroLedger).source.replacen(
            "if v == pass {} else { end returned };",
            "",
            1
        )
    );
    assert_eq!(
        jue_probe(JueKind::AtomicZero).source,
        jue_probe(JueKind::Atomic).source.replacen(
            "actor Ledger { capacity 1",
            "actor Ledger { capacity 0",
            1
        )
    );
    assert_eq!(
        jue_probe(JueKind::StateWriteOnly).source,
        jue_probe(JueKind::Atomic)
            .source
            .replacen("commits;", "", 1)
    );
}
