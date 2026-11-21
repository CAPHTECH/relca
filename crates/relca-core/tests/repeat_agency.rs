use relca_core::{ir, lower, mutation, report, syntax};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

const CHECK_AT_END: &str = include_str!("../../../skills/relca-design/examples/check-at-end.relca");
const CHECK_AT_END_LOOP: &str =
    include_str!("../../../skills/relca-design/examples/check-at-end-repeat.relca");
const M3: &str = include_str!("bounded_repetition/models/M3.relca");
const EVIDENCE: &str = include_str!("bounded_repetition/models/lineage.relca");
const FORK: &str = include_str!("bounded_repetition/models/fork-pair.relca");

fn lower_model(source: &str, config: Option<&str>) -> ir::Model {
    lower::lower_with_selection(&syntax::parse(source).unwrap(), config, None).unwrap()
}

fn agency(source: &str, config: Option<&str>) -> Value {
    let report = config.map_or_else(
        || report::check(source),
        |c| report::check_configuration(source, c),
    );
    let value = serde_json::to_value(report).unwrap();
    let a = &value["agency"];
    assert!(
        a.is_object() && a["status"] != "not_derived",
        "{:?}",
        value["diagnostics"]
    );
    a.clone()
}

// Explicit mapping, not a suffix heuristic: Hand's `{site}{round}` corresponds
// to Loop's physical `{site}` in that round and its one original site.
struct Sites {
    physical: BTreeMap<String, String>,
    original: BTreeMap<String, &'static str>,
}

impl Sites {
    fn new(loop_name: &str, rounds: usize, sites: &[&'static str]) -> Self {
        let mut s = Self {
            physical: BTreeMap::new(),
            original: BTreeMap::new(),
        };
        for round in 1..=rounds {
            for site in sites {
                let hand = format!("{site}{round}");
                s.physical
                    .insert(format!("@loop:{loop_name}:{round}:{site}"), hand.clone());
                s.original.insert(hand, site);
            }
        }
        s
    }

    fn rename(&self, value: &mut Value) {
        match value {
            Value::String(s) => {
                if let Some(name) = self.physical.get(s) {
                    s.clone_from(name);
                }
            }
            Value::Array(items) => items.iter_mut().for_each(|v| self.rename(v)),
            Value::Object(map) => map.values_mut().for_each(|v| self.rename(v)),
            _ => {}
        }
    }

    fn original(&self, name: &str) -> String {
        self.original.get(name).copied().unwrap_or(name).into()
    }

    fn group_hand(&self, hand: &mut Value) {
        let mut members: BTreeMap<Vec<String>, BTreeSet<String>> = BTreeMap::new();
        for row in hand["lineage"].as_array().unwrap() {
            members
                .entry(serde_json::from_value(row["subject_sources"].clone()).unwrap())
                .or_default()
                .insert(self.original(row["call"].as_str().unwrap()));
        }
        for row in hand["lineage_groups"].as_array_mut().unwrap() {
            let sources: Vec<String> =
                serde_json::from_value(row["subject_sources"].clone()).unwrap();
            row["judgements"] = json!(members[&sources].len());
        }
        if let Some(rows) = hand["viewpoints"]["observation"]["stages"].as_array_mut() {
            let mut grouped: Vec<Value> = Vec::new();
            for mut row in std::mem::take(rows) {
                row["call"] = json!(self.original(row["call"].as_str().unwrap()));
                if let Some(previous) = grouped.iter().find(|v| v["call"] == row["call"]) {
                    // In these witnesses, each repeat of a stage reads the same roots.
                    assert_eq!(*previous, row, "Hand source sets differ by round");
                } else {
                    grouped.push(row);
                }
            }
            *rows = grouped;
        }
    }
}

fn compare(label: &str, mut actual: Value, mut expected: Value, names: Sites) {
    assert!(
        !expected["origin"].as_array().unwrap().is_empty(),
        "{label}"
    );
    assert!(
        !expected["viewpoints"]["observation"]["stages"]
            .as_array()
            .unwrap()
            .is_empty(),
        "{label}"
    );
    names.rename(&mut actual);
    names.group_hand(&mut expected);
    for key in ["lineage", "lineage_groups", "origin"] {
        assert_eq!(actual[key], expected[key], "{label}/{key}");
    }
    let rules = if label == "gap" {
        ("agency-p2-loop-1", "agency-p1-requires-1")
    } else {
        ("agency-p2-loop-1", "agency-p0-1")
    };
    assert_eq!(actual["rules"], rules.0, "{label}/Loop rules");
    assert_eq!(expected["rules"], rules.1, "{label}/Hand rules");
    let mut unchecked = expected["unchecked"].as_array().unwrap().clone();
    unchecked.push(json!("state_guards"));
    unchecked.push(json!("loop_control"));
    assert_eq!(
        sorted(actual["unchecked"].as_array().unwrap()),
        sorted(&unchecked),
        "{label}/unchecked"
    );
    let lv = &actual["viewpoints"];
    let hv = &expected["viewpoints"];
    for key in ["rules", "scope", "observation", "proposal"] {
        assert_eq!(lv[key], hv[key], "{label}/{key}");
    }
    for key in ["framing", "retention"] {
        assert_eq!(lv[key]["status"], "not_derived", "{label}/{key}");
        assert_eq!(lv[key]["reason"], "loop_control", "{label}/{key}");
        assert_eq!(hv[key]["status"], "derived", "{label}/{key}");
    }
    assert_eq!(actual["refusal"], json!([]), "{label}");
    let l = lv["effects"]["rows"].as_array().unwrap();
    let h = hv["effects"]["rows"].as_array().unwrap();
    assert_eq!(l.len(), h.len(), "{label}/effects");
    if !h.is_empty() {
        assert!(
            !expected["refusal"].as_array().unwrap().is_empty(),
            "{label}/Hand refusal"
        );
        assert!(
            !expected["lineage"].as_array().unwrap().is_empty(),
            "{label}/Hand lineage"
        );
    }
    for (lr, hr) in l.iter().zip(h) {
        for key in ["effect", "call", "status", "e"] {
            assert_eq!(lr[key], hr[key], "{label}/effect/{key}");
        }
        assert_eq!(
            lr["process_judgements"]["reason"], "loop_control",
            "{label}"
        );
        assert!(lr.get("j").is_none() && lr.get("u").is_none(), "{label}");
        assert_eq!(
            lr["concentration"]["generation_roots"]["reason"], "loop_control",
            "{label}"
        );
    }
}

fn sorted(values: &[Value]) -> Vec<String> {
    let mut values: Vec<_> = values.iter().map(Value::to_string).collect();
    values.sort_unstable();
    values
}

#[test]
fn check_at_end_by_hand() {
    let hand = agency(CHECK_AT_END, None);
    let read_sites = ["a", "b", "x", "v"];
    for round in [1, 2] {
        for site in read_sites {
            let name = format!("{site}{round}");
            let row = hand["lineage"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["call"] == name)
                .unwrap();
            assert_eq!(row["subject_sources"], json!(["Spec"]));
            assert_eq!(row["independent_sources"], json!([]));
            if site == "v" {
                assert_eq!(row["capability"], "Reviewer.check");
            }
        }
    }
    assert_eq!(hand["lineage"].as_array().unwrap().len(), 8);
    assert_eq!(
        hand["lineage_groups"],
        json!([{"subject_sources":["Spec"],"judgements":8,"actors":3,"independent_sources":[]}])
    );
    assert_eq!(
        hand["viewpoints"]["observation"]["stages"]
            .as_array()
            .unwrap()
            .len(),
        12
    );
    assert_eq!(
        hand["origin"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["actor"] == "Reviewer")
            .unwrap()["direct"],
        json!(["Spec"])
    );
    assert_eq!(hand["refusal"].as_array().unwrap().len(), 2);
    for round in [1, 2] {
        let row = &hand["refusal"][round - 1];
        assert_eq!(row["call"], format!("r{round}"));
        assert_eq!(row["reachable"], true);
        assert!(row["judgements"].as_array().unwrap().contains(&json!({
            "call": format!("v{round}"), "capability":"Reviewer.check",
            "stops":"all_paths", "bypasses":0,
        })));
    }
    // Baseline Loop only reports lexical judgements that appear in a guard.
    // The Hand also derives the post's argument-ancestry judgements (a/b/x).
    // Their omission is part of the loop_control boundary, not a lost site.
    let mut supported_hand = hand;
    supported_hand["lineage"]
        .as_array_mut()
        .unwrap()
        .retain(|r| r["capability"] == "Reviewer.check");
    supported_hand["lineage_groups"][0]["actors"] = json!(1);
    compare(
        "CheckAtEnd",
        agency(CHECK_AT_END_LOOP, None),
        supported_hand,
        Sites::new("rounds", 2, &["p", "a", "b", "x", "v", "r"]),
    );
}

const PAIR: &str = "if r == pass { leave } else { f = Clerk.fix(); again }";
const GUARD: &str = "if r == fail { f = Clerk.fix(); again } else {}; leave";
const LEAVE_FIRST: &str = "if r == pass { leave } else {}; f = Clerk.fix(); again";

fn rework(source: &str, n: usize, branch: &str, feature: &str) -> (String, String, Sites) {
    let source = source.replace("clock 20min", "clock 60min");
    let source = if feature == "wait until" {
        source.replace(
            "reviewed: optional<V>",
            "reviewed: optional<V>; ready: bool = true",
        )
    } else {
        source
    };
    let start = source.find("repeat rework").unwrap();
    let limit_end = "} on_limit { end escalated }";
    let end = source.find(limit_end).unwrap() + limit_end.len();
    let wrap = |body| format!("{}{body}{}", &source[..start], &source[end..]);
    let wait = if feature == "wait until" {
        "wait until ready == true; "
    } else {
        ""
    };
    let gap = if feature == "gap" {
        " gap fixed 2min"
    } else {
        ""
    };
    let extra = if feature == "on_limit" {
        "last = Clerk.fix(); "
    } else {
        ""
    };
    let limit = format!("{extra}end escalated");
    let loop_source = wrap(format!(
        "repeat rework at_most {n}{gap} {{ {wait} r = Rev.review(entry); {branch} }} on_limit {{ {limit} }}"
    ));
    let mut hand = limit;
    for i in (1..=n).rev() {
        let w = if feature == "gap" && i < n {
            "wait 2min; "
        } else {
            ""
        };
        // Passing falls through to the one shared post site. Last failure
        // ends the case inside the nesting and cannot reach that site.
        hand = format!(
            "{wait} r{i} = Rev.review(entry); if r{i} == fail {{ f{i} = Clerk.fix(); {w}{hand} }} else {{}}"
        );
    }
    let mut sites = Sites::new("rework", n, &["r", "f"]);
    if feature == "on_limit" {
        sites
            .physical
            .insert(format!("@loop:rework:{n}:last"), "last".into());
    }
    (loop_source, wrap(hand), sites)
}

#[test]
fn rework_one_to_five_rounds_and_phase_variations() {
    let check = |label: &str, (lp, hd, map): (String, String, Sites)| {
        compare(label, agency(&lp, None), agency(&hd, None), map);
    };
    for (label, model) in [("M3", M3), ("RepeatedEvidence", EVIDENCE)] {
        for n in 1..=5 {
            for (shape, branch) in [
                ("pair", PAIR),
                ("guard", GUARD),
                ("leave-first", LEAVE_FIRST),
            ] {
                check(
                    &format!("{label}/{n}/{shape}"),
                    rework(model, n, branch, ""),
                );
            }
        }
    }
    for feature in ["wait until", "gap", "on_limit"] {
        check(feature, rework(M3, 3, GUARD, feature));
    }
}

#[test]
fn fork_pair() {
    compare(
        "fork-pair",
        agency(FORK, Some("Loop")),
        agency(FORK, Some("Hand")),
        Sites::new("wave", 2, &["a", "b"]),
    );
}

#[test]
fn round_conditions_are_a_known_difference() {
    let loop_src = include_str!("bounded_repetition/models/round-gate-loop.relca");
    let hand_src = include_str!("bounded_repetition/witnesses/round-gate-hand.relca");
    let loop_report = serde_json::to_value(report::check(loop_src)).unwrap();
    let hand_report = serde_json::to_value(report::check(hand_src)).unwrap();
    for r in [&loop_report, &hand_report] {
        assert_eq!(r["verdict"], "satisfied");
        let ends: BTreeMap<_, _> = r["terminals"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| {
                (
                    t["end"].as_str().unwrap(),
                    (t["elapsed"].clone(), t["work"].clone()),
                )
            })
            .collect();
        let fixed = |n| json!({"min":n,"max":n});
        assert_eq!(
            ends,
            BTreeMap::from([
                ("denied", (fixed(3), fixed(2))),
                ("done", (fixed(4), fixed(3)))
            ])
        );
    }
    let a = &loop_report["agency"];
    assert_eq!(a["lineage"], json!(["@loop:rounds:1:r", "@loop:rounds:2:r"].map(|call| json!({"call":call,"capability":"Rev.review","subject_sources":["Invoice"],"independent_sources":[]}))));
    assert!(
        a["unchecked"]
            .as_array()
            .unwrap()
            .contains(&json!("state_guards"))
    );
    assert_eq!(a["refusal"], json!([]));
    assert_eq!(
        hand_report["agency"]["lineage"],
        json!([{"call":"r2","capability":"Rev.review","subject_sources":["Invoice"],"independent_sources":[]}])
    );
    assert_eq!(
        hand_report["agency"]["refusal"],
        json!([{"call":"p2","effect":"Ledger.post","reachable":true,"judgements":[{"call":"r2","capability":"Rev.review","stops":"all_paths","bypasses":0}]}])
    );
}

#[test]
fn mutation_keeps_existing_loop_control_boundary() {
    let standard = serde_json::to_value(mutation::mutate(M3)).unwrap();
    let guarded = serde_json::to_value(mutation::mutate(&M3.replace(PAIR, GUARD))).unwrap();
    // Both spellings allow removal of the early rejection (detected); only
    // the paired form also offers the opposite change (still withheld).
    for (name, result, reason) in [
        ("evidence_missing", "undetected", json!("no_worsening")),
        ("summary_reused", "undetected", json!("no_assigned_check")),
        ("refusal_path_cut", "detected", Value::Null),
    ] {
        let row = |value: &Value| {
            value["mutations"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["kind"] == name)
                .unwrap()
                .clone()
        };
        let expected = row(&standard);
        let actual = row(&guarded);
        assert_eq!(actual["status"], "generated", "{name}");
        let mutants = actual["mutants"].as_array().unwrap();
        assert_eq!(mutants.len(), 1);
        assert_eq!(mutants[0]["result"], result, "{name}");
        assert_eq!(mutants[0]["reason"], reason, "{name}");
        let standard_rows = expected["mutants"].as_array().unwrap();
        assert!(
            standard_rows
                .iter()
                .any(|m| m["result"] == result && m["reason"] == reason),
            "{name}"
        );
        if name == "refusal_path_cut" {
            assert!(
                standard_rows
                    .iter()
                    .any(|m| m["result"] == "unsupported" && m["reason"] == "loop_control")
            );
        }
    }
}

#[test]
fn business_branches_keep_the_path_limit_in_loop_and_hand() {
    let decision = "if r == pass { wait 1min } else { wait 1min }; ";
    let (source, _, _) = rework(M3, 1, PAIR, "");
    let from = source.find("repeat rework").unwrap();
    let to = source.find("p = Ledger.post").unwrap();
    // The initial path + splits = 2^(rounds*k+1); the final decision
    // contributes the +1. Repeat always takes exactly the declared rounds.
    for (rounds, last) in [(1, 11), (2, 5), (3, 3)] {
        for k in [last, last + 1] {
            let block = decision.repeat(k);
            for process in [
                block.repeat(rounds),
                format!("repeat rework at_most {rounds} {{ {block} again }} on_limit {{ leave }}"),
            ] {
                let source = format!(
                    "{}r = Rev.review(entry); {process}; {decision}\n{}",
                    &source[..from],
                    &source[to..]
                );
                let m = lower_model(&source, None);
                let a = serde_json::to_value(&m.agency).unwrap();
                if k == last {
                    assert!(
                        m.agency.as_derived().is_some(),
                        "rounds={rounds} k={k}: {a}"
                    );
                } else {
                    assert_eq!(a["status"], "not_derived", "rounds={rounds} k={k}");
                    assert_eq!(a["limit"]["counter"], "paths");
                    assert!(m.diagnostics.iter().any(|d| d.code == "W-AGY-4"));
                }
            }
        }
    }
}

#[test]
fn no_new_j_row_for_a_call_after_an_always_ending_loop() {
    let source = M3
        .replace(PAIR, "end denied")
        .replace("verify reachable end.done", "verify reachable end.denied");
    let value = serde_json::to_value(report::check(&source)).unwrap();
    assert_eq!(value["verdict"], "vacuous");
    let row = &value["agency"]["viewpoints"]["effects"]["rows"][0];
    assert_eq!(row["call"], "p");
    assert_eq!(row["process_judgements"]["reason"], "loop_control");
    assert!(row.get("j").is_none());
}
