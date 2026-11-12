//! ADR 0023: hand oracles fixed before implementation. No production DTO types
//! are used to manufacture expected metadata, paths or defaults.
use relca_core::report;
use serde_json::{Value, json};
use std::path::PathBuf;
#[path = "support/responsibility_defaults_oracle.rs"]
mod responsibility_oracle;

pub const NAMES: [&str; 28] = [
    "act-arrival-units",
    "act-arrival-units.broken-later-first",
    "act-composite",
    "act-composite.broken-team-work",
    "act-control",
    "act-control.broken-explicit-environment",
    "act-cut-context",
    "act-cut-context.broken-no-cut",
    "act-debtor",
    "act-debtor-actor",
    "act-debtor-actor.broken-omitted",
    "act-debtor-value",
    "act-debtor-value.broken-other-person",
    "act-debtor.broken-unknown",
    "act-defaults",
    "act-defaults.broken-explicit-boundary",
    "act-grain",
    "act-grain.broken-swapped",
    "act-horizon",
    "act-horizon.broken-short-horizon",
    "act-legacy",
    "act-legacy.broken-longer-task",
    "act-overlap",
    "act-overlap.broken-relation",
    "act-units",
    "act-units.broken-duration-day",
    "act-via",
    "act-via.broken-upstream-only",
];
pub const NOTICES: [&str; 2] = [
    "Cut 外の存在(未知の被影響者、一度だけの拒否、登録されていない当事者)が作用しないとは主張しない",
    "この結果を運用の範囲決定に使う場合は採用経路を別に追うこと",
];
pub fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}
pub fn source(name: &str) -> String {
    std::fs::read_to_string(root().join(name).join("model.relca")).unwrap()
}
pub fn expected(name: &str) -> Value {
    serde_json::from_str(&std::fs::read_to_string(root().join(name).join("expected.json")).unwrap())
        .unwrap()
}
pub fn check(src: &str) -> Value {
    serde_json::to_value(report::check(src)).unwrap()
}
pub fn subset(e: &Value, a: &Value) {
    match e {
        Value::Object(o) => {
            for (k, v) in o {
                if k == "absent_diagnostics" {
                    for code in v.as_array().unwrap() {
                        assert!(
                            !a["diagnostics"]
                                .as_array()
                                .unwrap()
                                .iter()
                                .any(|d| d["code"] == *code)
                        );
                    }
                } else if ["diagnostics", "properties", "terminals"].contains(&k.as_str()) {
                    let key = match k.as_str() {
                        "diagnostics" => "code",
                        "properties" => "id",
                        _ => "end",
                    };
                    let rows = a[k].as_array().unwrap();
                    if k != "diagnostics" {
                        assert_eq!(v.as_array().unwrap().len(), rows.len(), "{k} inventory");
                    }
                    for hand in v.as_array().unwrap() {
                        subset(
                            hand,
                            rows.iter()
                                .find(|r| r[key] == hand[key])
                                .unwrap_or_else(|| panic!("missing {hand} in {rows:?}")),
                        );
                    }
                } else {
                    subset(v, a.get(k).unwrap_or_else(|| panic!("missing {k}: {a}")));
                }
            }
        }
        Value::Array(es) => {
            let actual = a.as_array().unwrap_or_else(|| panic!("not array: {a}"));
            assert_eq!(es.len(), actual.len(), "exact inventory");
            for (x, y) in es.iter().zip(actual) {
                subset(x, y);
            }
        }
        _ => assert_eq!(e, a),
    }
}
pub fn assert_context(v: &Value) {
    assert_eq!(v["cut_context"]["notices"], json!(NOTICES));
    if v["cut"].is_object() {
        assert_eq!(v["cut_context"]["reentry"], v["cut"]["reentry"]);
    } else {
        assert_eq!(v["cut_context"]["reentry"], json!([]));
        assert_eq!(v["cut_context"]["horizon"], json!([]));
    }
}

/// Remove only ADR 0023 additions, retaining the old executable skeleton. This
/// lets the old binary check hand arithmetic/warnings/bounds BEFORE parser work.
pub fn old_syntax(name: &str) -> String {
    let mut src = source(name);
    for text in [
        "grain person; ",
        "grain ledger; ",
        "boundary \"one registered actor\"; ",
        "contains [Member]; ",
        "overlaps [Member]; ",
        "via model \"doc@v3\"; ",
        "control policy; ",
        "control environment; ",
        "; horizon receipt..resolution within 1h; contest within 5d structural_only",
        "; horizon receipt..resolution within 0min; contest within 5d structural_only",
        "; horizon job within 1d",
        " debtor q",
        " debtor p",
        " debtor Worker",
        " debtor missing",
    ] {
        src = src.replace(text, "");
    }
    for (from, to) in [
        ("1h", "60min"),
        ("2h", "120min"),
        ("1d", "1440min"),
        ("2d", "2880min"),
    ] {
        src = src.replace(from, to);
    }
    src
}

fn skeleton_diagnostics_match(src: &str, actual: &Value) -> bool {
    // ADR 0054 "範囲" / "意味": derive the precise carriers and Actor spans
    // from the selected IR, independently of production's warning vector.
    let ast = relca_core::syntax::parse(src).unwrap();
    let model = relca_core::lower::lower(&ast).ok();
    let expected = model
        .as_ref()
        .map(|m| json!(responsibility_oracle::expected(m)))
        .unwrap_or_else(|| json!([]));
    let diagnostics = actual["diagnostics"].as_array().unwrap();
    let got: Vec<_> = diagnostics
        .iter()
        .filter(|d| d["code"] == "W-RESP-0")
        .collect();
    if json!(got) != expected {
        return false;
    }
    let notices = model
        .as_ref()
        .map(responsibility_oracle::expected_disclosures)
        .unwrap_or_default();
    let got_notices: Vec<_> = actual["agency"]["unchecked"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .filter(|word| word.starts_with("responsibility_default_candidates:"))
        .collect();
    got_notices == notices
        && diagnostics.iter().all(|d| {
            ["W-CUT-0", "W-AGY-0", "E-ARR-0", "W-UNREACH-0"].contains(&d["code"].as_str().unwrap())
                || expected.as_array().unwrap().contains(d)
        })
}
#[test]
fn old_syntax_skeletons_check_warning_bounds_properties_and_hand_time_before_implementation() {
    for name in NAMES {
        let src = old_syntax(name);
        let a = check(&src);
        let hand_name = match name {
            "act-composite.broken-team-work" => "act-composite",
            "act-overlap.broken-relation" => "act-overlap",
            "act-debtor.broken-unknown" => "act-debtor",
            _ => name,
        };
        let mut hand = expected(hand_name);
        for key in ["actors", "cut_context", "via_model_paths", "obligations"] {
            hand.as_object_mut().unwrap().remove(key);
        }
        // Fixture warning spans belong to the original input, not the stripped
        // skeleton. Rebase only the three known unused declarations from text.
        for d in hand
            .get_mut("diagnostics")
            .and_then(Value::as_array_mut)
            .into_iter()
            .flatten()
        {
            if d["code"] == "W-UNREACH-0" {
                let declaration = match name {
                    "act-defaults" | "act-defaults.broken-explicit-boundary" => {
                        "capability c() -> T { duration fixed 0min; result nondet { ok } }"
                    }
                    "act-composite.broken-team-work" => {
                        "capability run() -> T { duration fixed 1min; result nondet { ok } }"
                    }
                    _ => panic!("unclassified skeleton warning {name}"),
                };
                let start = src.find(declaration).unwrap();
                d["span"] = json!({"start":start,"end":start + declaration.len()});
            }
        }
        subset(&hand, &a);
        if name == "act-defaults" {
            assert_eq!(
                fields(&a),
                ["A.boundary", "A.control", "A.permission", "schedule"]
            );
        }
        // ADR 0025: the defaults skeleton ends without calling A.c; the
        // composite fault adds Team.run but the process still calls Member.
        let unreachable: Vec<_> = a["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|d| d["code"].as_str().unwrap().starts_with("W-UNREACH-"))
            .map(|d| (d["code"].as_str().unwrap(), d["message"].as_str().unwrap()))
            .collect();
        let expected_unreachable = match name {
            "act-defaults" | "act-defaults.broken-explicit-boundary" => {
                vec![("W-UNREACH-0", "process Flow: A.c not_called")]
            }
            "act-composite.broken-team-work" => {
                vec![("W-UNREACH-0", "process Flow: Team.run not_called")]
            }
            _ => vec![],
        };
        assert_eq!(unreachable, expected_unreachable, "{name}");
        assert!(skeleton_diagnostics_match(&src, &a), "{name}: {a}");
    }
}

#[test]
fn skeleton_k4_gate_rejects_spurious_missing_and_wrong_carrier_warnings() {
    let source = old_syntax("act-via");
    let actual = check(&source);
    assert!(skeleton_diagnostics_match(&source, &actual));
    let warnings: Vec<_> = actual["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["code"] == "W-RESP-0")
        .cloned()
        .collect();
    // Hand witness for the independent IR rule: Judge stops, Ledger commits.
    assert_eq!(
        warnings
            .iter()
            .map(|d| d["message"].as_str().unwrap())
            .collect::<Vec<_>>(),
        [
            "Judge has no declared responsibility",
            "Ledger has no declared responsibility"
        ]
    );
    let empty_source = old_syntax("act-defaults");
    let mut spurious = check(&empty_source);
    assert!(skeleton_diagnostics_match(&empty_source, &spurious));
    spurious["diagnostics"]
        .as_array_mut()
        .unwrap()
        .push(warnings[0].clone());
    assert!(!skeleton_diagnostics_match(&empty_source, &spurious));
    let mut missing = actual.clone();
    let rows = missing["diagnostics"].as_array_mut().unwrap();
    let index = rows.iter().position(|d| d == &warnings[0]).unwrap();
    rows.remove(index);
    assert!(!skeleton_diagnostics_match(&source, &missing));
    let mut wrong = actual.clone();
    wrong["diagnostics"][index]["message"] = json!("Extract has no declared responsibility");
    assert!(!skeleton_diagnostics_match(&source, &wrong));
}
#[test]
fn fourteen_pairs_are_only_the_declared_single_edit_or_grain_exchange() {
    for (good, bad, from, to) in [
        (
            "act-defaults",
            "act-defaults.broken-explicit-boundary",
            "actor A { ",
            "actor A { boundary \"one registered actor\"; ",
        ),
        (
            "act-composite",
            "act-composite.broken-team-work",
            "contains [Member]; capacity 1 }",
            "contains [Member]; capacity 1; capability run() -> T { duration fixed 1min; result nondet { ok } } }",
        ),
        (
            "act-overlap",
            "act-overlap.broken-relation",
            "overlaps [Member]",
            "contains [Member]",
        ),
        (
            "act-via",
            "act-via.broken-upstream-only",
            "j = Judge.check(e)",
            "j = Judge.check(r)",
        ),
        (
            "act-horizon",
            "act-horizon.broken-short-horizon",
            "within 1h",
            "within 0min",
        ),
        (
            "act-units",
            "act-units.broken-duration-day",
            "fixed 1h",
            "fixed 1d",
        ),
        (
            "act-debtor",
            "act-debtor.broken-unknown",
            "debtor q",
            "debtor missing",
        ),
        (
            "act-debtor-actor",
            "act-debtor-actor.broken-omitted",
            " debtor Worker",
            "",
        ),
        (
            "act-legacy",
            "act-legacy.broken-longer-task",
            "fixed 5min",
            "fixed 6min",
        ),
        (
            "act-control",
            "act-control.broken-explicit-environment",
            "control policy",
            "control environment",
        ),
        (
            "act-cut-context",
            "act-cut-context.broken-no-cut",
            "  cut C { include [A]; reentry [review cut quarterly]; horizon job within 1d }\n",
            "",
        ),
        (
            "act-arrival-units",
            "act-arrival-units.broken-later-first",
            "[30min,1h]",
            "[2h,1h]",
        ),
        (
            "act-debtor-value",
            "act-debtor-value.broken-other-person",
            "debtor p",
            "debtor q",
        ),
    ] {
        let src = source(good);
        assert_eq!(src.matches(from).count(), 1, "{good}");
        assert_eq!(src.replacen(from, to, 1), source(bad), "{bad}");
    }
    let grain = source("act-grain")
        .replace("grain person", "grain TEMP")
        .replace("grain ledger", "grain person")
        .replace("grain TEMP", "grain ledger");
    assert_eq!(grain, source("act-grain.broken-swapped"));
}

#[test]
fn f1_to_f14_all_28_reports_match_hand_oracles_and_cut_notices() {
    let mut runtime = 0;
    let mut rejected = 0;
    for name in NAMES {
        let actual = check(&source(name));
        assert_eq!(
            actual["verdict"],
            expected(name)["verdict"],
            "hand verdict {name}: {actual}"
        );
        subset(&expected(name), &actual);
        assert_context(&actual);
        if expected(name)["verdict"] == "invalid" {
            rejected += 1;
            assert!(
                actual.get("obligations").is_none(),
                "no fabricated ledger {name}"
            );
        } else {
            runtime += 1;
            assert!(actual["actors"].is_array(), "resolved table {name}");
        }
    }
    assert_eq!((runtime, rejected), (24, 4));
}

fn fields(v: &Value) -> Vec<String> {
    v["assumptions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["field"].as_str().unwrap().to_owned())
        .collect()
}
#[test]
fn f2_all_twelve_default_fields_and_only_value_filling_assumptions() {
    let a = check(&source("act-defaults"));
    let b = check(&source("act-defaults.broken-explicit-boundary"));
    subset(&expected("act-defaults")["actors"], &a["actors"]);
    subset(
        &expected("act-defaults.broken-explicit-boundary")["actors"],
        &b["actors"],
    );
    assert_eq!(a["actors"][0].as_object().unwrap().len(), 13);
    assert_eq!(
        fields(&a),
        ["A.boundary", "A.control", "A.permission", "schedule"]
    );
    assert_eq!(fields(&b), ["A.control", "A.permission", "schedule"]);
    let mut old = a["assumptions"].as_array().unwrap().clone();
    old.retain(|r| r["field"] != "A.boundary");
    assert_eq!(json!(old), b["assumptions"]);
}
#[test]
fn f8_f14_inferred_capacity_is_default_and_has_exactly_one_assumption() {
    for name in [
        "act-debtor",
        "act-debtor-value",
        "act-debtor-value.broken-other-person",
    ] {
        let a = check(&source(name));
        subset(&expected(name)["actors"], &a["actors"]);
        let rows: Vec<_> = a["assumptions"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["field"] == "Worker.capacity")
            .collect();
        assert_eq!(
            rows,
            vec![
                &json!({"field":"Worker.capacity","value":"count(executors)=1","affects":"Actor capacity is the number of declared executors"})
            ]
        );
    }
}
fn erase_hash(v: &mut Value) {
    v["model"].as_object_mut().unwrap().remove("sha256");
}
fn metadata_pair(good: &str, bad: &str, fields: &[&str]) {
    let mut a = check(&source(good));
    let mut b = check(&source(bad));
    assert_eq!(a["verdict"], "satisfied", "{good}: {a}");
    assert_eq!(b["verdict"], "satisfied", "{bad}: {b}");
    for v in [&mut a, &mut b] {
        erase_hash(v);
        for field in fields {
            v.pointer_mut(field)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .remove("value");
        }
    }
    assert_eq!(
        serde_json::to_vec_pretty(&a).unwrap(),
        serde_json::to_vec_pretty(&b).unwrap()
    );
}
#[test]
fn f1_f11_f14_metadata_cannot_change_any_other_report_byte() {
    metadata_pair(
        "act-grain",
        "act-grain.broken-swapped",
        &["/actors/0/grain", "/actors/1/grain"],
    );
    metadata_pair(
        "act-control",
        "act-control.broken-explicit-environment",
        &["/actors/0/control"],
    );
    let a = check(&source("act-control"));
    assert!(!fields(&a).contains(&"A.control".into()));
    assert!(fields(&a).contains(&"B.control".into()));
    let mut a = check(&source("act-debtor-value"));
    let mut b = check(&source("act-debtor-value.broken-other-person"));
    for v in [&mut a, &mut b] {
        erase_hash(v);
        v["obligations"][0]
            .as_object_mut()
            .unwrap()
            .remove("debtor");
    }
    assert_eq!(
        serde_json::to_vec_pretty(&a).unwrap(),
        serde_json::to_vec_pretty(&b).unwrap()
    );
}
#[test]
fn unequal_length_grain_changes_only_labels_hash_and_actual_diagnostic_positions() {
    let src = source("act-grain");
    let changed = src
        .replace("grain person", "grain arbitrary_team_label")
        .replace("grain ledger", "grain model");
    let mut a = check(&src);
    let mut b = check(&changed);
    for v in [&mut a, &mut b] {
        assert_eq!(v["verdict"], "satisfied");
        erase_hash(v);
        for row in v["actors"].as_array_mut().unwrap() {
            row.as_object_mut().unwrap().remove("grain");
        }
        for d in v["diagnostics"].as_array_mut().unwrap() {
            d.as_object_mut().unwrap().remove("span");
        }
    }
    assert_eq!(
        serde_json::to_vec(&a).unwrap(),
        serde_json::to_vec(&b).unwrap()
    );
}
#[test]
fn f6_zero_horizon_is_not_an_execution_bound() {
    let mut a = check(&source("act-horizon"));
    let mut b = check(&source("act-horizon.broken-short-horizon"));
    for v in [&mut a, &mut b] {
        erase_hash(v);
        v["cut_context"]["horizon"][0]
            .as_object_mut()
            .unwrap()
            .remove("within");
    }
    assert_eq!(a, b);
}

fn simple(actor_items: &str) -> String {
    format!(
        "model M version \"0.2\" {{ type T = enum {{ ok }} actor A {{ {actor_items} capacity 1; capability run() -> T {{ duration fixed 1min; result nondet {{ ok }} }} }} actor B {{ }} process Flow {{ a=A.run(); end done }} bounds {{ cases 1; live_tasks 2; clock 3min }} verify reachable end.done verify safety no_resource_overcommit }}"
    )
}
fn diagnostic(src: &str, verdict: &str, code: &str) {
    let a = check(src);
    assert_eq!(a["verdict"], verdict, "{src}: {a}");
    assert!(
        a["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == code),
        "{src}: {a}"
    );
    assert!(a["actors"].is_null());
    assert_context(&a);
}
#[test]
fn syntax_name_duplicate_cycle_and_via_shape_matrix() {
    for (items, verdict, code) in [
        ("boundary \"x\"; boundary \"x\";", "unsupported", "U-SEM-0"),
        ("grain model; grain model;", "unsupported", "U-SEM-0"),
        ("contains [B,B];", "unsupported", "U-SEM-0"),
        ("contains [Missing];", "invalid", "E-NAME-0"),
        ("overlaps [Missing];", "invalid", "E-NAME-0"),
        ("contains [A];", "invalid", "E-CON-0"),
        ("overlaps [A];", "invalid", "E-CON-0"),
        ("control policy; control policy;", "unsupported", "U-SEM-0"),
        (
            "via model \"x@v\"; via model \"x@v\";",
            "unsupported",
            "U-SEM-0",
        ),
        ("state x: set<T>;", "unsupported", "U-SYN-0"),
        ("atomic { };", "unsupported", "U-SYN-0"),
        ("version \"v\";", "unsupported", "U-SYN-0"),
        ("decides [B];", "unsupported", "U-SYN-0"),
    ] {
        diagnostic(&simple(items), verdict, code);
    }
    for via in ["x", "@v", "x@", "x@@v", "x@v v", "x @v"] {
        diagnostic(
            &simple(&format!("via model \"{via}\";")),
            "invalid",
            "E-CON-0",
        );
    }
    for via in ["doc-extract@v3", "日本語@版"] {
        assert_eq!(
            check(&simple(&format!("via model \"{via}\";")))["verdict"],
            "satisfied"
        );
    }
    for label in ["model", "unconstrained_label", "party"] {
        let a = check(&simple(&format!(
            "grain {label}; contains []; overlaps [];"
        )));
        assert_eq!(a["verdict"], "satisfied");
        assert_eq!(a["actors"][0]["grain"]["value"], label);
        assert_eq!(
            a["actors"][0]["contains"],
            json!({"value":[],"status":"declared"})
        );
    }
    for body in [
        "actor A { contains [B] } actor B { contains [A] }",
        "actor A { contains [B] } actor B { contains [C] } actor C { contains [A] }",
    ] {
        let src = format!(
            "model M version \"0.2\" {{ {body} process Flow {{ end done }} bounds {{ cases 1; live_tasks 1; clock 1min }} verify safety no_resource_overcommit }}"
        );
        diagnostic(&src, "invalid", "E-CON-0");
    }
}
#[test]
fn k6_k7_transitive_and_positive_probability_support_are_not_ignored() {
    let s = source("act-composite.broken-team-work")
        .replace("contains [Member]", "contains [Middle]")
        .replace(
            "  actor Member",
            "  actor Middle { contains [Member] }\n  actor Member",
        );
    diagnostic(&s, "invalid", "E-CAP-0");
    let distributed = source("act-composite.broken-team-work").replacen(
        "duration fixed 1min",
        "duration { 1min: 0.5, 2min: 0.5 }",
        1,
    );
    diagnostic(&distributed, "invalid", "E-CAP-0");
    let zero = source("act-composite.broken-team-work").replacen(
        "duration fixed 1min",
        "duration fixed 0min",
        1,
    );
    assert_eq!(check(&zero)["verdict"], "satisfied");
}
#[test]
fn k19_overlap_reciprocal_triangles_and_repetitions_are_legal() {
    let src = simple("overlaps [B,B]; overlaps [B];").replace(
        "actor B { }",
        "actor B { overlaps [A,C] } actor C { overlaps [A,B] }",
    );
    let a = check(&src);
    assert_eq!(a["verdict"], "satisfied", "{a}");
    assert_eq!(
        a["actors"][0]["overlaps"],
        json!({"value":["B"],"status":"declared"})
    );
    assert_eq!(a["actors"][1]["overlaps"]["value"], json!(["A", "C"]));
}
#[test]
fn f7_metrics_window_cost_and_utilization_are_minute_normalized() {
    for (name, cost, util) in [
        ("act-units", "60", "1/48"),
        ("act-units.broken-duration-day", "1440", "1/2"),
    ] {
        let a = check(&source(name));
        assert_eq!(a["bounds"]["clock"], 2880);
        assert_eq!(a["metrics"]["window"], 2880);
        let model =
            relca_core::lower::lower(&relca_core::syntax::parse(&source(name)).unwrap()).unwrap();
        let deadlines: Vec<_> = model
            .contracts
            .iter()
            .filter_map(|c| c.response().map(|(_, _, d)| d))
            .collect();
        assert_eq!(deadlines, [2880]);
        subset(
            &json!({"min":cost,"max":cost,"reason":null}),
            &a["metrics"]["overall"]["observed_cost_total"],
        );
        subset(
            &json!({"min":util,"max":util,"reason":null}),
            &a["metrics"]["actors"][0]["utilization"],
        );
    }
}
#[test]
fn debtor_suffix_resolution_mixed_unconfigured_and_nonexecutor_fulfillment() {
    let src = source("act-debtor");
    for suffix in ["debtor q non_waivable", "non_waivable debtor q"] {
        let a = check(&src.replace("debtor q", suffix));
        assert_eq!(a["verdict"], "satisfied");
        assert_eq!(a["obligations"][0]["debtor"], "q");
        assert_eq!(a["obligations"][0]["non_waivable"], true);
        assert_eq!(a["obligations"][0]["outcomes"][0]["status"], "fulfilled");
    }
    diagnostic(
        &src.replace("debtor q", "debtor q debtor p"),
        "unsupported",
        "U-SEM-0",
    );
    diagnostic(
        &src.replace("actor Worker", "actor q { } actor Worker"),
        "invalid",
        "E-CON-0",
    );
    for name in ["T", "run", "missing"] {
        diagnostic(
            &src.replace("debtor q", &format!("debtor {name}")),
            "invalid",
            "E-NAME-0",
        );
    }
    assert!(
        check(&src.replace(" debtor q", ""))
            .get("obligations")
            .is_none(),
        "old unlabelled response must stay table-free"
    );
    let mixed=source("act-debtor-actor").replace("debtor Worker", "debtor Worker; response other: case.received -> one_of(Worker.run.finish) by 2min after case.received otherwise unresolved");
    let a = check(&mixed);
    let rows = a["obligations"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(
        (rows[0]["debtor"].clone(), rows[0]["debtor_status"].clone()),
        (json!("Worker"), json!("declared"))
    );
    assert_eq!(
        (rows[1]["debtor"].clone(), rows[1]["debtor_status"].clone()),
        (Value::Null, json!("not_configured"))
    );
    let unused = src
        .replace("response case.received", "response Worker.run.finish(ok)")
        .replace("after case.received", "after Worker.run.finish(ok)")
        .replace(
            "process Flow { w = Worker.run(); end done }",
            "process Flow { end done }",
        );
    assert_eq!(
        check(&unused)["obligations"],
        json!([]),
        "untriggered response cannot invent an instance"
    );
    let waiver =
        source("ctx-waive").replace("otherwise unresolved", "otherwise unresolved debtor q");
    let a = check(&waiver);
    assert_eq!(a["verdict"], "satisfied");
    assert_eq!(a["obligations"][0]["debtor"], "q");
    assert_eq!(a["obligations"][0]["outcomes"][0]["status"], "waived");
}

#[test]
fn debtor_projects_through_both_arrivals_backends_with_arrival_relative_times() {
    use relca_core::{engine, lower, reference, syntax};
    let src=source("act-arrival-units").replace("  verify reachable", "  contract C { response case.received -> one_of(A.run.finish) by 2min after case.received otherwise unresolved debtor A }\n  verify reachable");
    let model = lower::lower(&syntax::parse(&src).unwrap()).unwrap();
    let e = engine::explore(&model);
    // Arrivals at 30 and 60 make the reference walk deeper than its default
    // branch depth (128); widen only that limit, on a larger stack.
    let r = std::thread::Builder::new()
        .stack_size(256 << 20)
        .spawn(move || {
            reference::explore_with_limits(
                &model,
                reference::Limits {
                    branch_depth: 512,
                    ..reference::Limits::default()
                },
            )
        })
        .unwrap()
        .join()
        .unwrap();
    assert_eq!(r.stop, reference::StopReason::Complete);
    for out in [&e, &r.outcome] {
        let rows = serde_json::to_value(out.obligations.as_ref().unwrap()).unwrap();
        let rows = rows.as_array().unwrap();
        assert_eq!(rows.len(), 2);
        for (row, arrival) in rows.iter().zip([30, 60]) {
            assert_eq!(row["debtor"], "A");
            assert_eq!(row["debtor_status"], "declared");
            assert_eq!(row["outcomes"].as_array().unwrap().len(), 1);
            subset(
                &json!({"status":"fulfilled","triggered_at":arrival,"deadline":arrival+2,"resolved_at":arrival+1}),
                &row["outcomes"][0],
            );
        }
    }
}

#[test]
fn all_time_lexemes_normalize_equivalently_and_overflow_fails_closed() {
    // Hand expectation: equivalent source units lower to identical IR ticks.
    // Report SHA/span differ, so compare runtime and numeric bounds, not hashes.
    let src = source("act-units");
    let minutes = src
        .replace("1h", "60min")
        .replace("2d", "2880min")
        .replace("1d", "1440min");
    let hand = check(&minutes);
    for spelling in ["1h", "1 h", "1\nh", "60min"] {
        let a = check(&src.replace("1h", spelling));
        assert_eq!(a["verdict"], "satisfied");
        for key in ["bounds", "properties", "terminals", "metrics"] {
            assert_eq!(a[key], hand[key], "{spelling}: {key}");
        }
    }
    diagnostic(
        &simple("").replace("fixed 1min", "fixed 18446744073709551615h"),
        "unsupported",
        "U-SEM-0",
    );
    diagnostic(
        &simple("").replace("fixed 1min", "fixed 18446744073709551616h"),
        "unsupported",
        "U-SYN-0",
    );
    let arrivals = source("act-arrival-units");
    assert_eq!(
        check(&arrivals.replace("[30min,1h]", "[30min,60min]"))["verdict"],
        "satisfied"
    );
    diagnostic(
        &arrivals.replace("[30min,1h]", "[1h,30min]"),
        "invalid",
        "E-ARR-0",
    );
}

#[test]
fn time_units_cover_fixed_distribution_ack_clock_window_arrivals_response_contest_probability_horizon()
 {
    let mut fixed = simple("")
        .replace("fixed 1min", "fixed 1h")
        .replace("clock 3min", "clock 2d");
    fixed=fixed.replace("verify reachable", "contract C { response case.received -> one_of(A.run.finish) by 1h after case.received otherwise unresolved } verify reachable");
    let distributed = fixed.replace("duration fixed 1h", "duration {1h:0.5,120min:0.5}");
    let ack = source("coop-cancel")
        .replace("ack_within 1min", "ack_within 1h")
        .replace("clock 4min", "clock 2d");
    let contest = source("ctx-contest").replace("respond_by 2min", "respond_by 1h");
    let probability = source("duration-serial")
        .replace("3min", "1h")
        .replace("5min", "2h")
        .replace("clock 20min", "clock 1d")
        .replace("within 6min", "within 2h")
        .replace("within 8min", "within 3h")
        .replace("within 10min", "within 4h");
    for src in [
        fixed,
        distributed,
        ack,
        contest.clone(),
        probability,
        source("act-units"),
        source("act-arrival-units"),
        source("act-horizon"),
    ] {
        // Exercise d at every location exercised with h, not only at bounds or
        // horizon. Compare against a separately spelled minute model; a day is
        // exactly 1440 minutes even for acknowledgement and contest deadlines.
        for src in [
            src.clone(),
            src.replace("1h", "1d")
                .replace("2h", "2d")
                .replace("3h", "3d")
                .replace("4h", "4d"),
        ] {
            let normalized = src
                .replace("1h", "60min")
                .replace("2h", "120min")
                .replace("3h", "180min")
                .replace("4h", "240min")
                .replace("1d", "1440min")
                .replace("2d", "2880min")
                .replace("3d", "4320min")
                .replace("4d", "5760min")
                .replace("5d", "7200min");
            let a = check(&src);
            let b = check(&normalized);
            assert!(
                !["invalid", "unsupported"].contains(&a["verdict"].as_str().unwrap()),
                "{src}: {a}"
            );
            for key in [
                "verdict",
                "bounds",
                "properties",
                "terminals",
                "case_outcomes",
                "metrics",
                "obligations",
                "contest_runtime",
                "cut_context",
            ] {
                assert_eq!(a[key], b[key], "unit normalization at {key}: {src}");
            }
        }
    }
    let hour24 = simple("")
        .replace("fixed 1min", "fixed 24h")
        .replace("clock 3min", "clock 2d");
    let day = hour24.replace("fixed 24h", "fixed 1d");
    assert_eq!(check(&hour24)["terminals"], check(&day)["terminals"]);
    assert_eq!(
        check(&day)["terminals"][0]["elapsed"],
        json!({"min":1440,"max":1440})
    );
    let a = check(&contest);
    for alt in a["contest_runtime"][0]["alternatives"].as_array().unwrap() {
        if alt["filed"] == true {
            assert_eq!(alt["deadline"], 61, "respond_by is target1+60, not raw1");
        }
    }
    let duplicate = simple("").replace("duration fixed 1min", "duration {1h:0.5,60min:0.5}");
    diagnostic(&duplicate, "invalid", "E-PROB-0");
    // Largest representable converted clock and the immediately overflowing
    // quotient; no execution is attempted beyond the ordinary one-minute job.
    for (unit, factor) in [("h", 60u64), ("d", 1440u64)] {
        let q = u64::MAX / factor;
        let a = check(&simple("").replace("clock 3min", &format!("clock {q}{unit}")));
        assert_eq!(a["verdict"], "satisfied");
        assert_eq!(a["bounds"]["clock"], json!(q * factor));
        diagnostic(
            &simple("").replace("clock 3min", &format!("clock {}{unit}", q + 1)),
            "unsupported",
            "U-SEM-0",
        );
    }
}

#[test]
fn old_time_grammar_preflight_distribution_ack_contest_probability() {
    let fixed=simple("").replace("fixed 1min","fixed 60min").replace("clock 3min","clock 2880min").replace("verify reachable","contract C { response case.received -> one_of(A.run.finish) by 60min after case.received otherwise unresolved } verify reachable");
    let distributed = fixed.replace("duration fixed 60min", "duration {60min:0.5,120min:0.5}");
    let ack = source("coop-cancel")
        .replace("ack_within 1min", "ack_within 60min")
        .replace("clock 4min", "clock 2880min");
    let contest = source("ctx-contest").replace("respond_by 2min", "respond_by 60min");
    let probability = source("duration-serial")
        .replace("3min", "60min")
        .replace("5min", "120min")
        .replace("clock 20min", "clock 1440min")
        .replace("within 6min", "within 120min")
        .replace("within 8min", "within 180min")
        .replace("within 10min", "within 240min");
    for (src, verdict) in [
        (fixed, "satisfied"),
        (distributed, "violated"),
        (ack, "satisfied"),
        (contest, "satisfied"),
        (probability, "satisfied"),
    ] {
        let a = check(&src);
        assert_eq!(a["verdict"], verdict, "minute skeleton {src}: {a}");
        assert_eq!(a["bounds"]["truncated"], false);
    }
}

#[test]
fn horizon_append_zero_duplicate_and_comparison_mismatch() {
    let src = source("act-horizon");
    let multiple = src.replace("; contest within", "; horizon contest within");
    assert_eq!(check(&multiple)["cut_context"], check(&src)["cut_context"]);
    diagnostic(
        &src.replace(
            "; reentry",
            "; horizon receipt..resolution within 60min; reentry",
        ),
        "unsupported",
        "U-SEM-0",
    );
    let configurations=src.replace("  bounds", "  cut D { include [A]; horizon receipt..resolution within 0min; contest within 5d structural_only; reentry [review cut quarterly] }\n  configuration X { cut C; process Flow }\n  configuration Y { cut D; process Flow }\n  bounds");
    let d = serde_json::to_value(relca_core::diff::diff(&configurations, "X", "Y")).unwrap();
    let c = serde_json::to_value(relca_core::compare::compare(
        &configurations,
        &["X", "Y"],
        relca_core::compare::Analysis::Exhaustive,
    ))
    .unwrap();
    for v in [&d, &c] {
        assert_eq!(v["verdict"], "invalid", "horizon-only mismatch: {v}");
        assert!(
            v["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["code"] == "E-CMP-0")
        );
    }
    assert_eq!(
        d["mismatch"]["differences"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["field"] == "horizon")
            .count(),
        1,
        "not merely different cut names"
    );
    assert_eq!(
        c["mismatch"]["pairs"][0]["cut"]["differences"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["field"] == "horizon")
            .count(),
        1
    );
    let equivalent = configurations.replace("within 0min", "within 60min");
    let d = serde_json::to_value(relca_core::diff::diff(&equivalent, "X", "Y")).unwrap();
    assert!(
        !d["mismatch"]["differences"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["field"] == "horizon")
    );
}

#[test]
fn horizon_and_via_syntax_missing_parts_fail_closed_with_existing_codes() {
    let src = source("act-horizon");
    for malformed in [
        "receipt.. within 1h",
        "receipt..resolution 1h",
        "receipt..resolution within",
        "receipt . . resolution within 1h",
    ] {
        diagnostic(
            &src.replace("receipt..resolution within 1h", malformed),
            "invalid",
            "E-SYN-0",
        );
    }
    diagnostic(&simple("via model;"), "invalid", "E-SYN-0");
    let labels = src.replace(
        "receipt..resolution within 1h; contest within 5d structural_only",
        "model .. party within 0min; horizon arbitrary within 1d structural_only",
    );
    let a = check(&labels);
    assert_eq!(a["verdict"], "satisfied");
    assert_eq!(
        a["cut_context"]["horizon"],
        json!([{"from":"model","to":"party","within":0,"structural_only":false},{"from":null,"to":"arbitrary","within":1440,"structural_only":true}])
    );
    let semicolon = src
        .replace("include [A]; horizon", "horizon")
        .replace("; reentry", "; include [A]; reentry");
    assert_eq!(
        check(&semicolon)["verdict"],
        "satisfied",
        "semicolon before include is outer Cut separator"
    );
}

fn diamond(layers: usize) -> String {
    let mut src = String::from(
        "model Diamond version \"0.2\" { type T = enum { ok } actor Seed { via model \"seed@v1\"; capacity 1; capability make() -> T { duration fixed 1min; result nondet { ok } } } actor Relay { capacity 1; capability pair(a:T,b:T) -> T { duration fixed 1min; result nondet { ok } } } actor Ledger { capacity 1; capability post(a:T,b:T) -> T { commits; duration fixed 1min; result nondet { ok } } } process Flow { s=Seed.make(); ",
    );
    let (mut left, mut right) = ("s".to_owned(), "s".to_owned());
    for k in 0..layers {
        src.push_str(&format!(
            "a{k}=Relay.pair({left},{right}); b{k}=Relay.pair({left},{right}); "
        ));
        left = format!("a{k}");
        right = format!("b{k}");
    }
    src.push_str(&format!("posted=Ledger.post({left},{right}); end done }} bounds {{ cases 1; live_tasks 2; clock 40min }} verify reachable end.done verify safety no_resource_overcommit }}"));
    src
}
#[test]
fn k13_diamond_lists_both_call_sequences_not_only_one_shortest_path() {
    let a = check(&diamond(1));
    assert_eq!(a["verdict"], "satisfied", "{a}");
    assert_eq!(a["via_model_paths"]["path_count_lower_bound"], 2);
    let rows = a["via_model_paths"]["paths"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    let calls: std::collections::BTreeSet<_> = rows
        .iter()
        .map(|r| {
            r["calls"]
                .as_array()
                .unwrap()
                .iter()
                .map(|c| c["call"].as_str().unwrap())
                .collect::<Vec<_>>()
        })
        .collect();
    assert_eq!(
        calls,
        std::collections::BTreeSet::from([vec!["s", "a0", "posted"], vec!["s", "b0", "posted"]])
    );
    assert!(
        rows.iter()
            .all(|r| r["target"] == json!({"kind":"E","call":"posted"})),
        "single-valued relays are not U"
    );
}
#[test]
fn k20_4096_exact_and_4097th_path_mark_table_not_root_as_truncated() {
    for (layers, count, truncated) in [(12, 4096, false), (13, 4096, true)] {
        let a = check(&diamond(layers));
        assert_eq!(
            a["verdict"], "satisfied",
            "path enumeration must not replace root verdict: {a}"
        );
        assert_eq!(a["bounds"]["truncated"], false);
        assert_eq!(
            a["terminals"][0]["elapsed"],
            json!({"min":2*layers+2,"max":2*layers+2})
        );
        let p = &a["via_model_paths"];
        assert_eq!(p["paths"].as_array().unwrap().len(), count);
        assert_eq!(p["truncated"], truncated);
        if truncated {
            assert!(p["path_count_lower_bound"].as_u64().unwrap() >= 4097);
            assert!(
                p["unchecked"]
                    .as_array()
                    .unwrap()
                    .contains(&json!("path_enumeration_limit"))
            );
        } else {
            assert_eq!(p["path_count_lower_bound"], 4096);
        }
    }
}
#[test]
fn zero_edge_guard_only_repeated_capability_and_state_guard_paths() {
    let a = check(
        &simple("via model \"doc@v1\";")
            .replace("duration fixed 1min", "commits; duration fixed 1min"),
    );
    let rows = a["via_model_paths"]["paths"].as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["calls"], json!([{"call":"a","capability":"A.run"}]));
    assert_eq!(rows[0]["target"], json!({"kind":"E","call":"a"}));
    let guard = source("act-via")
        .replace("post(v:Verdict)", "post()")
        .replace("Ledger.post(j)", "Ledger.post()");
    let a = check(&guard);
    let rows = a["via_model_paths"]["paths"].as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0]["target"],
        json!({"kind":"U","call":"j","effect_call":"z","stops":"all_paths"})
    );
    let sites = source("act-via")
        .replace("r = Reader.make()", "r = Extract.make()")
        .replace("Ledger.post(j)", "Ledger.post(k)");
    let a = check(&sites);
    let rows = a["via_model_paths"]["paths"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    assert!(
        rows.iter()
            .any(|r| r["start_call"] == "e" && r["target"]["kind"] == "U")
    );
    assert!(
        rows.iter()
            .any(|r| r["start_call"] == "r" && r["target"]["kind"] == "E")
    );
    let state = source("state-agency.broken-state-guard")
        .replace("actor Source {", "actor Source { via model \"doc@v1\";");
    let a = check(&state);
    assert_eq!(a["verdict"], "satisfied");
    assert!(
        a["via_model_paths"]["paths"]
            .as_array()
            .unwrap()
            .iter()
            .all(|p| p["target"]["kind"] != "U")
    );
    assert!(
        a["via_model_paths"]["unchecked"]
            .as_array()
            .unwrap()
            .contains(&json!("state_guard_control_paths"))
    );
}
#[test]
fn contextual_words_stay_legal_old_identifiers_and_an_actor_named_ai_is_not_via() {
    for word in [
        "debtor",
        "horizon",
        "within",
        "structural_only",
        "policy",
        "environment",
        "h",
        "d",
    ] {
        let src = format!(
            "model M version \"0.2\" {{ type T=enum {{ok}} people [{word}] actor AI {{ executors [{word}]; capability {word}() -> T {{ duration fixed 1min; result nondet {{ok}} }} }} process Flow {{ {word}=AI.{word}(); end done }} bounds {{cases 1;live_tasks 1;clock 2min}} verify safety no_resource_overcommit }}"
        );
        let a = check(&src);
        assert_eq!(
            a["verdict"], "satisfied",
            "old legal identifier {word}: {a}"
        );
        assert!(a.get("via_model_paths").is_none());
    }
}
#[test]
fn all_added_graph_skeletons_are_old_legal_without_via_declarations() {
    for layers in [1, 12, 13] {
        let a = check(&diamond(layers).replace("via model \"seed@v1\"; ", ""));
        assert_eq!(a["verdict"], "satisfied", "{a}");
        assert_eq!(a["bounds"]["truncated"], false);
        assert_eq!(
            a["terminals"][0]["elapsed"],
            json!({"min":2*layers+2,"max":2*layers+2})
        );
    }
}

#[test]
fn runtime_clock_cut_keeps_resolved_metadata_static_failure_does_not() {
    let src = source("act-grain");
    let base = check(&src);
    let cut = check(&src.replace("clock 3min", "clock 1min"));
    assert_eq!(cut["bounds"]["truncated"], true);
    assert_eq!(base["actors"], cut["actors"]);
    assert_context(&cut);
    // The public report API does not expose state-budget injection. Its internal
    // 0/1 budget projection checks require separate internal coverage.
    let failed = check("not a model");
    assert_eq!(failed["actors"], Value::Null);
    assert_context(&failed);
}

#[test]
fn via_model_requires_exactly_one_at_with_both_sides_nonempty() {
    // ADR 0023 answer U5: exactly one @, non-empty name and version, no blank.
    let src = source("act-via");
    assert_eq!(src.matches("via model \"doc@v3\"").count(), 1);
    for bad in [
        "doc@v3@x", "doc@@v3", "@v3", "doc@", "doc", "doc @v3", "doc@v 3",
    ] {
        diagnostic(
            &src.replace("via model \"doc@v3\"", &format!("via model \"{bad}\"")),
            "invalid",
            "E-CON-0",
        );
    }
    assert_eq!(check(&src)["verdict"], "satisfied");
}
