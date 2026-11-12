//! ADR0044 integration hand oracles. No output capture is an expected-value source.
#![allow(dead_code)]
use serde_json::{Value, json};

pub fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .join(name)
            .join("model.relca"),
    )
    .unwrap()
}
pub fn replace(src: &str, from: &str, to: &str) -> String {
    assert_eq!(src.matches(from).count(), 1, "exact input edit: {from}");
    let edited = src.replacen(from, to, 1);
    assert_ne!(src, edited, "input transformation must be effective");
    edited
}
pub fn append(src: &str, item: &str) -> String {
    let k = src.rfind('}').unwrap();
    format!("{}\n{item}\n{}", &src[..k], &src[k..])
}
pub fn inline_rule(closed: bool) -> String {
    let s = replace(
        &fixture("rule-agency"),
        "decides approval_required: bool = true",
        "decides mode: enum { open, closed } = open",
    );
    let s = replace(
        &s,
        "ApprovalRule.approval_required == false || approval == pass",
        "ApprovalRule.mode == open && approval == pass",
    );
    if closed {
        replace(&s, "= open }", "= closed }")
    } else {
        s
    }
}
pub fn inline_rule_decides_first(closed: bool) -> String {
    let s = inline_rule(closed);
    // Put the invalid inline position before version, so ADR0044's historical
    // version-first parser precedence cannot mask the targeted diagnostic.
    let s = replace(&s, "version \"r1\"; decides", "decides");
    replace(
        &s,
        if closed { "= closed }" } else { "= open }" },
        if closed {
            "= closed; version \"r1\" }"
        } else {
            "= open; version \"r1\" }"
        },
    )
}
pub fn named_enum_rule(closed: bool) -> String {
    let s = replace(
        &inline_rule(closed),
        "mode: enum { open, closed }",
        "mode: Mode",
    );
    append(&s, "type Mode = enum { open, closed }")
}
pub fn record_rule(bad: bool) -> String {
    let s = append(&fixture("rule-agency"), "type R = record { flag: bool }");
    if bad {
        replace(
            &s,
            "decides approval_required: bool",
            "decides approval_required: R",
        )
    } else {
        s
    }
}
pub fn scenarios() -> String {
    append(
        &fixture("rule-basic"),
        "scenario Light { arrivals fixed [0min] }\nscenario Later { arrivals fixed [5min] }",
    )
}
pub fn rule_item(version: &str) -> Value {
    json!({"kind":"rule","actor":"ApprovalRule","version":version,"reads":["ApprovalRule.approval_required"]})
}
pub fn carrier() -> Value {
    json!({"actors":["Ledger"],"people":[],"unattributed_actors":["Ledger"],"permitted":[]})
}
pub fn effect(amended: bool) -> Value {
    json!({"effect":"Ledger.post","call":"ack","status":"derived","e":carrier(),"j":[],
        "u":if amended {json!([])} else {json!([rule_item("r1")])},
        "state_refusers":[],"process_judgements":"derived",
        "concentration":{"single_actor":!amended,"single_person":"unknown","single_generation_root":false,"generation_roots":[],
        "signal":"判定・拒否・実行の担い手と生成根の集中の兆候","mechanism":"同じ担い手又は生成根への依存が構成上重なる","check":"モデル外の統制・原本参照・ログで確認する"}})
}
pub fn bad_rule(tag: &str) -> Value {
    let mut v = rule_item("r1");
    match tag {
        "unknown" => v["extra"] = json!(true),
        "kind" => {
            v.as_object_mut().unwrap().remove("kind");
        }
        "mixed" => {
            v["call"] = json!("a");
            v["capability"] = json!("Approver.approve");
        }
        "reads" => v["reads"] = json!([]),
        _ => panic!("unknown rule fault"),
    }
    v
}
pub fn quality_is_withdrawn(q: &Value) -> bool {
    // ADR0045 keeps analysis exhaustive/simulation. Aborted is a cell reason,
    // never a new analysis vocabulary word. Do not invent a Rule-only schema.
    let aborted = |v: &Value| {
        v["reason"] == "aborted"
            && ["point", "lower", "upper", "min", "max", "mean"]
                .iter()
                .all(|k| v.get(k).is_none_or(Value::is_null))
    };
    fn all_cells(v: &Value) -> bool {
        match v {
            Value::Object(fields) => {
                (!fields.contains_key("reason") || fields["reason"] == "aborted")
                    && fields.iter().all(|(k, v)| match k.as_str() {
                        "point"
                        | "lower"
                        | "upper"
                        | "min"
                        | "max"
                        | "mean"
                        | "basis"
                        | "bounds_with_undetermined" => v.is_null(),
                        "samples" | "hits" | "unknown" | "trials" => v == &json!(0),
                        _ => all_cells(v),
                    })
            }
            Value::Array(xs) => xs.iter().all(all_cells),
            _ => true,
        }
    }
    all_cells(q)
        && q["checked"] == json!([])
        && q["properties"].as_array().is_some_and(|rows| {
            !rows.is_empty() && rows.iter().all(|r| aborted(&r["expected_count"]))
        })
        && q["cohort"]
            .as_array()
            .is_some_and(|rows| !rows.is_empty() && rows.iter().all(|r| aborted(&r["value"])))
        && q["outcomes_per_arrival"].as_array().is_some_and(|rows| {
            !rows.is_empty()
                && rows
                    .iter()
                    .all(|r| aborted(&r["expected_count"]) && aborted(&r["rate"]))
        })
}
pub fn differences() -> Value {
    let a = json!({"call":"a","capability":"Approver.approve","selects":"all_contexts"});
    let ack = json!({"call":"ack","capability":"Ledger.post"});
    let ar = json!({"call":"a","capability":"Approver.approve"});
    let mut rows = Vec::new();
    for (view, item, key, from, to) in [
        (
            "framing",
            "reachable",
            json!({"end":"held"}),
            json!(true),
            json!(false),
        ),
        (
            "framing",
            "selectors",
            json!({"end":"posted"}),
            json!([a.clone()]),
            json!([]),
        ),
        (
            "framing",
            "selectors",
            json!({"end":"held"}),
            json!([a]),
            json!([]),
        ),
        (
            "framing",
            "read_only",
            json!({"end":"posted"}),
            json!([ack.clone()]),
            json!([ar, ack.clone()]),
        ),
        (
            "framing",
            "read_only",
            json!({"end":"held"}),
            json!([ack]),
            json!([]),
        ),
        (
            "framing",
            "actors",
            json!({}),
            json!({"actors":["Approver"],"complete":true}),
            json!({"actors":[],"complete":true}),
        ),
        (
            "retention",
            "normal",
            json!({"capability":"Ledger.post"}),
            json!({"min":0,"max":1}),
            json!({"min":1,"max":1}),
        ),
        (
            "effects",
            "u",
            json!({"effect":"Ledger.post","call":"ack"}),
            json!([rule_item("r1")]),
            json!([]),
        ),
        (
            "effects",
            "concentration.single_actor",
            json!({"effect":"Ledger.post","call":"ack"}),
            json!(true),
            json!(false),
        ),
        (
            "refusal",
            "judgements",
            json!({"effect":"Ledger.post","call":"ack"}),
            json!([rule_item("r1")]),
            Value::Null,
        ),
    ] {
        let mut row = json!({"viewpoint":view,"item":item,"status":"changed","from":from,"to":to});
        row.as_object_mut()
            .unwrap()
            .extend(key.as_object().unwrap().clone());
        rows.push(row);
    }
    json!(rows)
}
pub const QUALITY: &str = r#"model RuleQuality version "0.2" {
 type T = enum { ok }
 people [p]
 observed case { hit: bool = false }
 actor ApprovalRule { version "r1"; decides enabled: bool = true }
 actor A { executors [p]; capability run() -> T { duration fixed 1min; result nondet { ok }; writes hit; finish { hit = true } } }
 authority G { basis "prior"; grants p to change ApprovalRule }
 process P { x = A.run(); end done }
 cut C { include [ApprovalRule,A] }
 configuration AsIs { cut C; process P }
 configuration ToBeB { cut C; process P; amend ApprovalRule from "r1" to "r2" by p under G { enabled = false } }
 property case.Hit = reached(ApprovalRule.enabled == true)
 metric cohort.Rate = expected(count(case.Hit)) / arrivals
 bounds { cases 1; live_tasks 1; clock 5min }
 simulation replications 4 seed 7 confidence 0.95
 verify safety no_resource_overcommit
}"#;

pub fn strip_quality(s: &str) -> String {
    let old = s
        .lines()
        .filter(|l| {
            !l.trim_start().starts_with("property case.")
                && !l.trim_start().starts_with("metric cohort.")
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert_ne!(s, old);
    old
}

pub fn quality_rule_assumption() -> Value {
    json!({"field":"quality.rule","value":"unsupported","affects":"quality for models with Rule state is not reported because the combination of ADR 0044 and ADR 0045 is unsupported"})
}

pub fn assert_quality_assumptions(v: &Value, sampled: bool, _rule: bool) {
    let rows = v["assumptions"].as_array().unwrap();
    let quality_rows: Vec<_> = rows
        .iter()
        .enumerate()
        .filter(|(_, r)| {
            matches!(
                r["field"].as_str(),
                Some(
                    "quality.definition"
                        | "quality.denominator"
                        | "quality.form"
                        | "quality.scheduler"
                        | "quality.estimation"
                        | "quality.rule"
                )
            )
        })
        .collect();
    let fields = vec![
        "quality.definition",
        "quality.denominator",
        "quality.form",
        if sampled {
            "quality.estimation"
        } else {
            "quality.scheduler"
        },
    ];
    assert_eq!(
        quality_rows
            .iter()
            .map(|(_, r)| r["field"].as_str().unwrap())
            .collect::<Vec<_>>(),
        fields
    );
    let start = quality_rows[0].0;
    for (k, (pos, _)) in quality_rows.iter().enumerate() {
        assert_eq!(*pos, start + k);
    }
    let end = start + fields.len();
    assert!(
        end == rows.len() || end + 1 == rows.len() && rows[end]["field"] == "probability.scheduler"
    );
    assert!(
        !rows[..start]
            .iter()
            .any(|r| r["field"] == "probability.scheduler")
    );
    assert!(!rows.iter().any(|a| a["field"] == "quality.rule"));
}

/// ADR0052: the initial fixed Rule value is the property, not an unknown.
pub fn assert_fixed_quality(q: &Value, enabled: bool, sampled: bool) {
    let n = if enabled { "1" } else { "0" };
    let value = &q["cohort"][0]["value"];
    assert_eq!(value["reason"], Value::Null);
    assert!(!q["checked"].as_array().unwrap().is_empty());
    if sampled {
        assert_eq!(value["point"], n);
        assert_eq!(
            q["properties"][0]["cases"][0]["hits"],
            if enabled { 4 } else { 0 }
        );
        assert_eq!(q["properties"][0]["cases"][0]["trials"], 4);
    } else {
        assert_eq!(
            *value,
            json!({"lower":n,"upper":n,"basis":"exact","reason":null})
        );
    }
}

pub const HELD_QUALITY: &str = r#"model M version "0.2" {
 type T = enum { ok, refused }
 actor R { version "r1"; decides enabled: bool = true }
 actor A { capacity 1; capability run() -> T { requires R.enabled == true else refused; duration fixed 5min; result nondet { ok } } }
 process P { x = A.run(); end done }
 cut C { include [R,A] }
 policy fifo tie_by case_id
 scenario Day { arrivals fixed [0min,1min,2min]; concurrent cases 1 on_overflow wait_external fifo; horizon 3min; bounds { cases 3; live_tasks 1; clock 20min } }
 property case.Hit = reached(R.enabled == true)
 metric cohort.Rate = expected(count(case.Hit)) / arrivals
 simulation replications 4 seed 7 confidence 0.95
 verify safety no_resource_overcommit
}"#;

pub fn held_quality(drain: bool, rule: bool) -> String {
    let s = if drain {
        replace(HELD_QUALITY, "horizon 3min;", "")
    } else {
        HELD_QUALITY.into()
    };
    if rule {
        return s;
    }
    let s = replace(
        &s,
        "actor R { version \"r1\"; decides enabled: bool = true }",
        "observed case { enabled: bool = true }",
    );
    assert_eq!(s.matches("R.enabled").count(), 2);
    let s = s.replace("R.enabled", "enabled");
    replace(&s, "include [R,A]", "include [A]")
}

pub fn atomic_rule(required: bool) -> String {
    let s = append(
        &fixture("agy2-atomic-u"),
        "actor R { version \"r1\"; decides required: bool = true }",
    );
    let s = replace(
        &s,
        "include [Doc, Clerk, Approver, Ledger]",
        "include [Doc, Clerk, Approver, Ledger, R]",
    );
    let s = replace(&s, "if v == pass", "if R.required == false || v == pass");
    if required {
        s
    } else {
        replace(&s, "required: bool = true", "required: bool = false")
    }
}

pub fn atomic_judge(required: bool) -> Value {
    json!({"call":"v","capability":"Approver.approve","actors":["Approver"],"people":["a1"],"unattributed_actors":[],"permitted":["a1"],"stops":if required {"all_paths"} else {"none"},"bypasses":if required {0} else {1}})
}

pub fn atomic_rule_item() -> Value {
    json!({"kind":"rule","actor":"R","version":"r1","reads":["R.required"]})
}

pub const VERSION_CONTROL: &str = r#"model VersionRule version "0.2" {
 type T = enum { ok, stale }
 actor ApprovalRule { capacity 1; version "r1"; decides enabled: bool = true
   state scratch: bool = true
   capability run() -> T { duration fixed 1min; result nondet { ok }; writes scratch; finish { scratch = true }; version_check [scratch] else stale }
 }
 process P { x = ApprovalRule.run(); end done }
 cut C { include [ApprovalRule] }
 bounds { cases 1; live_tasks 1; clock 5min }
 verify safety no_resource_overcommit
}"#;
