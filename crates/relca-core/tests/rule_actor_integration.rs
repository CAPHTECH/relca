//! ADR0044 x ADR0038–0045. Acceptance first; production repairs belong to the next role.
#[path = "support/rule_integration_inputs.rs"]
mod i;
use relca_core::ir::*;
use relca_core::{engine, lower, reference, report, syntax};
use serde_json::{Value, json};
#[path = "support/responsibility_defaults_oracle.rs"]
mod responsibility_oracle;

fn model(s: &str, config: Option<&str>, scenario: Option<&str>) -> Model {
    let a = syntax::parse(s).unwrap_or_else(|d| panic!("parse substrate: {d:?}"));
    lower::lower_with_selection(&a, config, scenario).unwrap_or_else(|d| panic!("lower: {d:?}"))
}
fn check(s: &str, config: Option<&str>, scenario: Option<&str>) -> Value {
    json!(report::check_with_selection(s, config, scenario))
}
fn outcome(m: &Model, r: bool) -> relca_core::outcome::Outcome {
    if r {
        let o = reference::explore(m);
        assert_eq!(
            o.stop,
            reference::StopReason::Complete,
            "reference must not silently exclude input"
        );
        o.outcome
    } else {
        engine::explore(m)
    }
}
fn term(v: &Value, end: &str, t: u64) {
    let xs = v["terminals"].as_array().unwrap();
    let x = xs
        .iter()
        .find(|x| x["end"] == end)
        .unwrap_or_else(|| panic!("missing {end}: {v}"));
    assert_eq!(x["elapsed"], json!({"min":t,"max":t}));
    assert_eq!(x["work"], json!({"min":t,"max":t}));
}
macro_rules! pair {
    ($e:ident,$r:ident,$body:expr) => {
        #[test]
        fn $e() {
            ($body)(false);
        }
        #[test]
        fn $r() {
            ($body)(true);
        }
    };
}

pair!(t21_named_enum_engine, t21_named_enum_reference, |r| {
    for closed in [false, true] {
        let s = i::named_enum_rule(closed);
        let m = model(&s, None, None);
        let o = outcome(&m, r);
        assert_eq!(o.terminals.len(), if closed { 1 } else { 2 });
        let held = o
            .terminals
            .iter()
            .find(|t| m.ends[t.end.0] == "held")
            .unwrap();
        assert_eq!(
            (
                held.elapsed.min,
                held.elapsed.max,
                held.work.min,
                held.work.max
            ),
            (2, 2, 2, 2)
        );
        if !closed {
            let posted = o
                .terminals
                .iter()
                .find(|t| m.ends[t.end.0] == "posted")
                .unwrap();
            assert_eq!(
                (
                    posted.elapsed.min,
                    posted.elapsed.max,
                    posted.work.min,
                    posted.work.max
                ),
                (3, 3, 3, 3)
            );
        }
        let v = check(&s, None, None);
        assert_eq!(v["verdict"], if closed { "vacuous" } else { "satisfied" });
    }
});

fn t21_inline_position(decides_first: bool) {
    for closed in [false, true] {
        let s = if decides_first {
            i::inline_rule_decides_first(closed)
        } else {
            i::inline_rule(closed)
        };
        let errors = syntax::parse(&s).unwrap_err();
        assert_eq!(errors.len(), 1);
        assert_eq!(
            errors[0].message,
            "unsupported syntax: inline enum position is unsupported"
        );
        let v = check(&s, None, None);
        assert_eq!(v["verdict"], "unsupported");
        let ds = v["diagnostics"].as_array().unwrap();
        assert_eq!(ds.len(), 1);
        assert_eq!(ds[0]["code"], "U-SYN-0");
        assert_eq!(ds[0]["severity"], "error");
        assert_eq!(
            ds[0]["message"],
            "unsupported syntax: inline enum position is unsupported"
        );
        assert!(ds[0]["span"].is_object());
        let start = s.find("enum { open, closed }").unwrap();
        assert_eq!(ds[0]["span"], json!({"start":start,"end":start+4}));
        assert_eq!(v["cut"], "unresolved");
    }
}
#[test]
fn t21_inline_enum_position_remains_unsupported_not_a_new_rule_type_syntax() {
    t21_inline_position(false);
}
#[test]
fn t21_inline_enum_decides_first_keeps_inner_type_diagnostic() {
    t21_inline_position(true);
}

#[test]
fn s01_named_record_refuses_at_lower_with_one_located_error() {
    assert_eq!(
        check(&i::record_rule(false), None, None)["verdict"],
        "satisfied"
    );
    let s = i::record_rule(true);
    let a = syntax::parse(&s).expect("record grammar belongs to ADR0038");
    let ds = lower::lower(&a).unwrap_err();
    assert_eq!(ds.len(), 1, "no E-TYPE cascade: {ds:?}");
    assert_eq!(ds[0].code, "U-SEM-0");
    assert_eq!(ds[0].message, "rule value type is unsupported");
    assert!(ds[0].span.is_some());
    assert_eq!(check(&s, None, None)["verdict"], "unsupported");
}

#[test]
fn s22_scenario_body_amend_keeps_parser_error_without_cut_warning() {
    let good = i::scenarios();
    assert_eq!(
        check(&good, Some("AsIs"), Some("Light"))["verdict"],
        "satisfied"
    );
    let bad = i::replace(
        &good,
        "scenario Light { arrivals fixed [0min] }",
        "scenario Light { arrivals fixed [0min]; amend ApprovalRule from \"r1\" to \"r2\" by cfo under RuleChange { approval_required = false } }",
    );
    let v = check(&bad, Some("AsIs"), Some("Light"));
    assert_eq!(v["verdict"], "unsupported");
    let ds = v["diagnostics"].as_array().unwrap();
    assert_eq!(ds.len(), 1);
    let start = bad.find("scenario Light").unwrap()
        + bad[bad.find("scenario Light").unwrap()..]
            .find("amend")
            .unwrap();
    assert_eq!(
        ds[0],
        json!({"code":"U-SYN-0","severity":"error","message":"unsupported syntax: scenario item","span":{"start":start,"end":start+5}})
    );
    assert_eq!(v["cut"], "unresolved");
}

#[test]
fn scenario_fresh_lower_checks_unselected_amendments_once() {
    let s = i::replace(
        &i::scenarios(),
        "by cfo under RuleChange {",
        "by a1 under RuleChange {",
    );
    for day in ["Light", "Later"] {
        let v = check(&s, Some("AsIs"), Some(day));
        assert_eq!(v["verdict"], "invalid");
        let errors: Vec<_> = v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|d| d["severity"] == "error")
            .collect();
        assert_eq!(
            errors.len(),
            1,
            "scenario revalidation must deduplicate the common error"
        );
        assert_eq!(errors[0]["code"], "E-CON-0");
        assert_eq!(
            errors[0]["message"],
            "rule change by a person excluded by not_by"
        );
        let start = s.find("by a1 under RuleChange").unwrap() + 3;
        assert_eq!(errors[0]["span"], json!({"start":start,"end":start+2}));
    }
}

pair!(t24_product_engine, t24_product_reference, |r| {
    let s = i::scenarios();
    let selections = [
        ("AsIs", "Light", 0),
        ("ToBeB", "Later", 5),
        ("ToBeB", "Light", 0),
        ("AsIs", "Later", 5),
    ];
    // Fresh lowering through both orders, not reusing or patching an effective Model.
    for reverse in [false, true] {
        for k in 0..selections.len() {
            let (c, day, arrival) = selections[if reverse { 3 - k } else { k }];
            let m = model(&s, Some(c), Some(day));
            let rule = m
                .actors
                .iter()
                .find(|a| a.name == "ApprovalRule")
                .unwrap()
                .metadata
                .rule
                .as_ref()
                .unwrap();
            assert_eq!(
                rule.effective_version,
                if c == "AsIs" { "r1" } else { "r2" }
            );
            let o = outcome(&m, r);
            let cases = o.case_outcomes.as_ref().unwrap();
            assert_eq!(cases.len(), 1);
            let ts = &cases[0].terminals;
            assert_eq!(ts.len(), if c == "AsIs" { 2 } else { 1 });
            for t in ts {
                let relca_core::outcome::CaseTerminalKind::End(end) = t.kind else {
                    panic!("normal end required")
                };
                let elapsed = if m.ends[end.0] == "posted" { 3 } else { 2 };
                assert_eq!(
                    t.elapsed,
                    Some(relca_core::outcome::Range {
                        min: elapsed,
                        max: elapsed
                    })
                );
                assert_eq!(
                    t.work,
                    Some(relca_core::outcome::Range {
                        min: elapsed,
                        max: elapsed
                    })
                );
                assert_eq!(
                    t.completed_at,
                    Some(relca_core::outcome::Range {
                        min: arrival + elapsed,
                        max: arrival + elapsed
                    })
                );
            }
            assert_eq!(
                check(&s, Some(c), Some(day))["verdict"],
                if c == "AsIs" { "satisfied" } else { "violated" }
            );
        }
    }
});

#[test]
fn s26_rule_version_is_not_mutable_state_version_lower_diagnostic() {
    assert_eq!(
        check(i::VERSION_CONTROL, None, None)["verdict"],
        "satisfied"
    );
    let bad = i::replace(
        i::VERSION_CONTROL,
        "version_check [scratch]",
        "version_check [enabled]",
    );
    let a = syntax::parse(&bad).unwrap();
    let ds = lower::lower(&a).unwrap_err();
    assert!(
        ds.iter().all(|d| d.code == "U-SEM-0" && d.span.is_some()),
        "{ds:?}"
    );
    assert!(
        ds.iter()
            .any(|d| d.message == "version_check outside shared Actor state is unsupported"),
        "{ds:?}"
    );
}

fn forged(tag: &str) -> Model {
    if tag == "version_check" {
        // Start from an independently executable version_check with a distinct
        // else value; do not let an overlapping reject result mask the Rule fault.
        let mut m = model(i::VERSION_CONTROL, None, None);
        let id = m
            .actors
            .iter()
            .find_map(|a| a.metadata.rule.as_ref().map(|r| r.values[0].variable))
            .unwrap();
        m.capabilities[0].version_check.as_mut().unwrap().variables = vec![id];
        return m;
    }
    let src = if tag == "record" {
        i::append(
            &i::fixture("rule-basic"),
            "type RecordValue = record { flag: bool }",
        )
    } else {
        i::fixture("rule-basic")
    };
    let mut m = model(&src, Some("AsIs"), None);
    let (owner, id) = m
        .actors
        .iter()
        .enumerate()
        .find_map(|(n, a)| a.metadata.rule.as_ref().map(|r| (n, r.values[0].variable)))
        .unwrap();
    let auth = m
        .authorities
        .iter()
        .position(|a| a.name == "RuleChange")
        .unwrap();
    match tag {
        "record" => {
            m.state_variables[id.0].base = TypeId(
                m.types
                    .iter()
                    .position(|t| t.name == "RecordValue")
                    .unwrap(),
            );
        }
        "last_write_wins" => m.state_variables[id.0].last_write_wins = true,
        "resolution" => m.concurrency_resolutions.push(ResolvedPair {
            variable: id,
            left: CapabilityId(0),
            right: CapabilityId(1),
            resolution: ConcurrencyResolution::LastWriteWins,
        }),
        "lost_update" => m.verify.push(Verify {
            property: Property::NoLostUpdate(id),
            span: m.state_variables[id.0].span,
        }),
        "owner" => m.state_variables[id.0].kind = StateKind::Rule(ActorId(usize::MAX)),
        "variable" => {
            m.actors[owner].metadata.rule.as_mut().unwrap().values[0].variable =
                StateVarId(usize::MAX)
        }
        "type" => m.state_variables[id.0].base = TypeId(usize::MAX),
        "metadata" => m.actors[owner].metadata.rule = None,
        "duplicate" => {
            let v = m.actors[owner].metadata.rule.as_ref().unwrap().values[0].clone();
            m.actors[owner]
                .metadata
                .rule
                .as_mut()
                .unwrap()
                .values
                .push(v);
        }
        "grant_rule" => m.authorities[auth].rule_grants[0].rule = ActorId(usize::MAX),
        "grant_person" => m.authorities[auth].rule_grants[0].persons = vec![PersonId(usize::MAX)],
        "under_nonrule" => {
            m.authorities[auth].under = Some(RuleBasis {
                rule: ActorId(1),
                version: "r1".into(),
                span: m.authorities[auth].span,
            })
        }
        _ => panic!("unknown probe {tag}"),
    }
    m
}
fn rejects(m: &Model, r: bool) {
    if r {
        assert_eq!(
            reference::explore(m).stop,
            reference::StopReason::SemanticAbort
        );
    } else {
        let o = engine::explore(m);
        assert!(o.diagnostics.iter().any(|d| d.code == "U-SEM-0"), "{o:?}");
        assert!(o.terminals.is_empty());
    }
}
macro_rules! ir_pair {
    ($e:ident,$r:ident,$tag:literal) => {
        pair!($e, $r, |r| rejects(&forged($tag), r));
    };
}
ir_pair!(ir_record_engine, ir_record_reference, "record");
ir_pair!(ir_version_engine, ir_version_reference, "version_check");
pair!(
    ir_version_control_engine,
    ir_version_control_reference,
    |r| {
        let m = model(i::VERSION_CONTROL, None, None);
        let o = outcome(&m, r);
        assert_eq!(o.terminals.len(), 1);
        let t = &o.terminals[0];
        assert_eq!(m.ends[t.end.0], "done");
        assert_eq!(
            (t.elapsed.min, t.elapsed.max, t.work.min, t.work.max),
            (1, 1, 1, 1)
        );
    }
);
ir_pair!(ir_lww_engine, ir_lww_reference, "last_write_wins");
ir_pair!(ir_resolution_engine, ir_resolution_reference, "resolution");
ir_pair!(
    ir_lost_update_engine,
    ir_lost_update_reference,
    "lost_update"
);
ir_pair!(ir_owner_engine, ir_owner_reference, "owner");
ir_pair!(ir_variable_engine, ir_variable_reference, "variable");
ir_pair!(ir_type_engine, ir_type_reference, "type");
ir_pair!(ir_metadata_engine, ir_metadata_reference, "metadata");
ir_pair!(ir_duplicate_engine, ir_duplicate_reference, "duplicate");
ir_pair!(ir_grant_rule_engine, ir_grant_rule_reference, "grant_rule");
ir_pair!(
    ir_grant_person_engine,
    ir_grant_person_reference,
    "grant_person"
);
ir_pair!(ir_under_engine, ir_under_reference, "under_nonrule");
pair!(ir_control_engine, ir_control_reference, |r| {
    let m = model(&i::fixture("rule-basic"), Some("AsIs"), None);
    let o = outcome(&m, r);
    assert_eq!(o.terminals.len(), 2);
    for t in &o.terminals {
        let n = if m.ends[t.end.0] == "posted" { 3 } else { 2 };
        assert_eq!(
            (t.elapsed.min, t.elapsed.max, t.work.min, t.work.max),
            (n, n, n, n)
        );
    }
});

#[test]
fn c5_effects_rule_u_is_not_a_person_process_j_or_generation_root() {
    let s = i::fixture("rule-basic");
    for c in ["AsIs", "ToBeB"] {
        let v = check(&s, Some(c), None);
        let effects = &v["agency"]["viewpoints"]["effects"];
        assert_eq!(effects["status"], "derived");
        let rows = effects["rows"].as_array().unwrap();
        assert_eq!(rows.len(), 1);
        let x = &rows[0];
        assert_eq!(x["effect"], "Ledger.post");
        assert_eq!(x["call"], "ack");
        assert_eq!(x["e"], i::carrier());
        assert_eq!(x["process_judgements"], "derived");
        assert_eq!(x["j"], json!([]));
        assert_eq!(
            x["u"],
            if c == "AsIs" {
                json!([i::rule_item("r1")])
            } else {
                json!([])
            }
        );
        for (k, expected) in [
            ("single_actor", json!(c == "AsIs")),
            ("single_person", json!("unknown")),
            ("single_generation_root", json!(false)),
            ("generation_roots", json!([])),
        ] {
            assert_eq!(x["concentration"][k], expected, "{c}/{k}");
        }
    }
}
#[test]
fn c5_rule_responsibility_warning_order_is_derived_not_declared() {
    let s = i::fixture("rule-basic");
    for (c, names) in [
        ("AsIs", vec!["ApprovalRule", "Ledger"]),
        ("ToBeB", vec!["Ledger"]),
    ] {
        let v = check(&s, Some(c), None);
        let got: Vec<_> = v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|d| d["code"] == "W-RESP-0")
            .map(|d| d["message"].clone())
            .collect();
        assert_eq!(
            got,
            names
                .iter()
                .map(|n| json!(format!("{n} has no declared responsibility")))
                .collect::<Vec<_>>(),
            "{c}"
        );
    }
}

#[test]
fn c5_framing_keeps_lexical_support_under_fixed_rule_values() {
    let s = i::fixture("rule-basic");
    for c in ["AsIs", "ToBeB"] {
        let v = check(&s, Some(c), None);
        let f = &v["agency"]["viewpoints"]["framing"];
        assert_eq!(f["status"], "derived");
        assert_eq!(f["complete"], true);
        assert_eq!(
            f["actors"],
            if c == "AsIs" {
                json!(["Approver"])
            } else {
                json!([])
            }
        );
        for end in ["posted", "held"] {
            let e = f["ends"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["end"] == end)
                .unwrap();
            assert_eq!(e["status"], "derived");
            assert_eq!(e["reachable"], c == "AsIs" || end == "posted");
            assert_eq!(
                e["selectors"],
                if c == "AsIs" {
                    json!([{"call":"a","capability":"Approver.approve","selects":"all_contexts"}])
                } else {
                    json!([])
                }
            );
            let readers = if c == "AsIs" {
                json!([{"call":"ack","capability":"Ledger.post"}])
            } else if end == "posted" {
                json!([{"call":"a","capability":"Approver.approve"},{"call":"ack","capability":"Ledger.post"}])
            } else {
                json!([])
            };
            assert_eq!(
                e["read_only"], readers,
                "{c}/{end}: lexical, not boolean-simplified support"
            );
        }
    }
}
#[test]
fn c5_retention_counts_started_invocations_not_requires_refusals() {
    let s = i::fixture("rule-basic");
    for c in ["AsIs", "ToBeB"] {
        let v = check(&s, Some(c), None);
        let l = &v["agency"]["viewpoints"]["retention"];
        assert_eq!(l["status"], "derived");
        let xs = l["capabilities"].as_array().unwrap();
        for (cap, min, max) in [
            ("Approver.approve", 1, 1),
            ("Ledger.post", if c == "AsIs" { 0 } else { 1 }, 1),
        ] {
            let row = xs.iter().find(|x| x["capability"] == cap).unwrap();
            assert_eq!(row["status"], "derived");
            assert_eq!(row["normal"], json!({"min":min,"max":max}));
        }
    }
}
#[test]
fn c5_f_l_support_and_exact_ten_semantic_changes() {
    let d = json!(relca_core::diff::diff(
        &i::fixture("rule-diff"),
        "AsIs",
        "ToBeB"
    ));
    assert_eq!(d["agency"]["differences"]["status"], "derived");
    assert_eq!(d["agency"]["differences"]["rules"], "agency-diff-p2-1");
    assert_eq!(d["agency"]["differences"]["rows"], i::differences());
}

pair!(
    quality_rule_exhaustive_engine,
    quality_rule_exhaustive_reference,
    |r| {
        for c in ["AsIs", "ToBeB"] {
            let m = model(i::QUALITY, Some(c), None);
            let q = if r {
                reference::explore(&m).outcome.quality
            } else {
                engine::explore(&m).quality
            };
            i::assert_fixed_quality(&json!(q.unwrap()), c == "AsIs", false);
        }
    }
);
pair!(
    quality_rule_sample_engine,
    quality_rule_sample_reference,
    |r| {
        for c in ["AsIs", "ToBeB"] {
            let m = model(i::QUALITY, Some(c), None);
            let q = if r {
                reference::simulate(&m).outcome.quality
            } else {
                engine::simulate(&m).quality
            };
            i::assert_fixed_quality(&json!(q.unwrap()), c == "AsIs", true);
        }
    }
);

#[test]
fn c5_supplemental_process_j_has_real_read_lineage_not_fabricated_from_original() {
    let s = i::fixture("rule-basic");
    let s = i::append(
        &s,
        "type Entry = enum { entry }\nactor Reader { capacity 1; capability make() -> Entry { duration fixed 1min; result nondet { entry } } }",
    );
    let s = i::replace(
        &s,
        "include [ApprovalRule, Approver, Ledger]",
        "include [ApprovalRule, Approver, Ledger, Reader]",
    );
    let s = i::replace(
        &s,
        "capability approve() -> V {",
        "capability approve(e: Entry) -> V { reads [e];",
    );
    let s = i::replace(
        &s,
        "a = Approver.approve();",
        "e = Reader.make(); a = Approver.approve(e);",
    );
    for c in ["AsIs", "ToBeB"] {
        let v = check(&s, Some(c), None);
        term(&v, "posted", 4);
        if c == "AsIs" {
            term(&v, "held", 3);
        }
        let refusal = &v["agency"]["refusal"];
        if c == "AsIs" {
            assert_eq!(
                refusal[0]["judgements"],
                json!([{"call":"a","capability":"Approver.approve","stops":"all_paths","bypasses":0},i::rule_item("r1")])
            );
        } else {
            assert_eq!(
                refusal[0]["judgements"],
                json!([{"call":"a","capability":"Approver.approve","stops":"none","bypasses":1}])
            );
        }
    }
}

#[test]
fn c5_effects_wire_accepts_closed_rule_u_without_adding_process_j_fields() {
    let x = json!({"status":"derived","rows":[i::effect(false)],"unchecked":["authority","fallback_path"]});
    let decoded =
        serde_json::from_value::<relca_core::agency::Agency>(agency_with_effects(x.clone()))
            .expect("Rule union is valid effects.u");
    assert_eq!(
        json!(decoded)["viewpoints"]["effects"],
        x,
        "untagged process arm and Rule arm round trip without fabricated carrier"
    );
}
#[test]
fn c5_difference_cells_accept_closed_rule_u() {
    let x = json!({"rules":"agency-diff-p2-1","status":"derived","rows":i::differences(),"unchecked":["fallback_path","workload_transfer","via_model_paths"]});
    let decoded = serde_json::from_value::<relca_core::agency::Differences>(x.clone())
        .expect("Rule cells belong to the closed difference table");
    assert_eq!(json!(decoded), x);
}
#[test]
fn c5_effects_and_difference_union_reject_bad_or_duplicate_rule_items() {
    for tag in ["unknown", "kind", "mixed", "reads", "duplicate"] {
        let bad = if tag == "duplicate" {
            json!([i::rule_item("r1"), i::rule_item("r1")])
        } else {
            json!([i::bad_rule(tag)])
        };
        let mut effect = i::effect(false);
        effect["u"] = bad.clone();
        let x =
            json!({"status":"derived","rows":[effect],"unchecked":["authority","fallback_path"]});
        assert!(
            serde_json::from_value::<relca_core::agency::Agency>(agency_with_effects(x)).is_err(),
            "effects {tag}"
        );
        let row = json!({"viewpoint":"effects","item":"u","effect":"Ledger.post","call":"ack","status":"changed","from":bad,"to":[]});
        let x = json!({"rules":"agency-diff-p2-1","status":"derived","rows":[row],"unchecked":["fallback_path","workload_transfer","via_model_paths"]});
        assert!(
            serde_json::from_value::<relca_core::agency::Differences>(x).is_err(),
            "differences {tag}"
        );
    }
}

fn agency_with_effects(x: Value) -> Value {
    let mut a = json!(model(&i::fixture("rule-basic"), Some("AsIs"), None).agency);
    a["viewpoints"]["effects"] = x;
    a
}

#[test]
fn rule_change_grants_do_not_grant_contest_reversal_to_a_person() {
    let s = i::append(
        &i::fixture("rule-basic"),
        r#"
 actor Clerk { capacity 1; capability recheck() -> V { duration fixed 1min; result nondet { pass, fail } } }
 actor Vendor { control party; standing over [Ledger.post.commit] basis "claim" }
 contract Review { contest Vendor on Ledger.post.commit route Clerk.recheck reversible_by [cfo] respond_by 5min }
"#,
    );
    let s = i::replace(
        &s,
        "include [ApprovalRule, Approver, Ledger]",
        "include [ApprovalRule, Approver, Ledger, Clerk, Vendor]",
    );
    let s = i::replace(
        &s,
        "a = Approver.approve(); ack = Ledger.post();",
        "a = Approver.approve(); if a == fail { end denied } else { }; ack = Ledger.post();",
    );
    let v = check(&s, Some("AsIs"), None);
    let paths = v["contest_paths"].as_array().unwrap();
    assert_eq!(paths.len(), 1);
    let reversal = &paths[0]["checks"]["reversal_authority"];
    assert_eq!(reversal["satisfied"], false);
    assert_eq!(reversal["people"][0]["person"], "cfo");
    assert!(
        reversal["people"][0]["decisions"]
            .as_array()
            .unwrap()
            .iter()
            .all(|d| d["authorities"] == json!([])),
        "RuleChange is not a Capability grant"
    );
    let control = i::replace(&s, "reversible_by [cfo]", "reversible_by [a1]");
    assert_eq!(
        check(&control, Some("AsIs"), None)["contest_paths"][0]["checks"]["reversal_authority"]["satisfied"],
        true
    );
}

#[test]
fn rule_u_does_not_prove_a_persons_all_paths_override() {
    let s = i::replace(
        &i::fixture("rule-agency"),
        "actor ApprovalRule { control policy;",
        "actor ApprovalRule { executors [cfo]; control policy;",
    );
    let s = i::append(
        &s,
        r#"
 authority RuleChange { basis "change only"; grants cfo to change ApprovalRule }
 process Before { end posted }
 configuration From { cut C; process Before }
 configuration To { cut C; process Flow; intent { add Ledger reason "posting"; add Ledger.post reason "posting"; override relation Ledger.post by cfo } }
"#,
    );
    let d = json!(relca_core::diff::diff(&s, "From", "To"));
    let row = d["differences"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["kind"] == "relation" && r["target"] == "Ledger.post")
        .unwrap();
    let a = row["annotations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["kind"] == "override")
        .unwrap();
    assert_eq!(
        a["verdict"], "violated",
        "a Rule item is not a process judgement held by cfo"
    );
    assert_eq!(a["effects"][0]["judgements"], json!([]));
    assert_ne!(a["effects"][0]["stops"], "all_paths");
}

pair!(
    limits_horizon_rule_engine,
    limits_horizon_rule_reference,
    |r| {
        let base = r#"model M version "0.2" {
 type T = enum { ok, refused }
 actor R { version "r1"; decides enabled: bool = true }
 actor A { capacity 1; capability run() -> T { requires R.enabled == true else refused; duration fixed 5min; result nondet { ok } } }
 process P { x = A.run(); end done }
 cut C { include [R,A] }
 policy fifo tie_by case_id
 scenario Day { arrivals fixed [0min,1min,2min]; concurrent cases 1 on_overflow wait_external fifo; horizon 3min; bounds { cases 3; live_tasks 1; clock 20min } }
 verify safety no_resource_overcommit
}"#;
        for mode in ["held", "reject", "overflow", "drain"] {
            let s = match mode {
                "reject" => i::replace(base, "wait_external fifo", "reject"),
                "overflow" => i::append(
                    &i::replace(
                        base,
                        "concurrent cases 1 on_overflow wait_external fifo;",
                        "",
                    ),
                    "concurrent cases 1 on_overflow end overflowed",
                ),
                "drain" => i::replace(base, "horizon 3min;", ""),
                _ => base.into(),
            };
            let m = model(&s, None, Some("Day"));
            let o = outcome(&m, r);
            let cases = o.case_outcomes.as_ref().unwrap();
            assert_eq!(cases.len(), 3);
            for (k, c) in cases.iter().enumerate() {
                assert_eq!(c.terminals.len(), 1);
                let t = &c.terminals[0];
                use relca_core::outcome::CaseTerminalKind::*;
                match mode {
                    "drain" => {
                        assert!(matches!(t.kind, End(_)));
                        let at = 5 * (k as u64 + 1);
                        assert_eq!(
                            t.completed_at,
                            Some(relca_core::outcome::Range { min: at, max: at })
                        );
                    }
                    "held" => assert_eq!(t.kind, CutOff),
                    "reject" if k > 0 => assert_eq!(t.kind, Rejected),
                    "overflow" if k > 0 => assert!(matches!(t.kind, Overflow(_))),
                    _ => assert_eq!(t.kind, CutOff),
                }
            }
            assert!(
                !o.truncated,
                "business horizon is not computational truncation"
            );
            let control = i::replace(
                &s,
                "actor R { version \"r1\"; decides enabled: bool = true }",
                "observed case { enabled: bool = true }",
            );
            let control = i::replace(
                &control,
                "requires R.enabled == true else refused;",
                "requires enabled == true else refused;",
            );
            let control = i::replace(&control, "include [R,A]", "include [A]");
            assert_eq!(
                o,
                outcome(&model(&control, None, Some("Day")), r),
                "constant Rule must not add shared-state scheduling: {mode}"
            );
        }
    }
);

fn to_be_b_quality(r: bool, sample: bool) {
    let m = model(i::QUALITY, Some("ToBeB"), None);
    let q = if sample {
        if r {
            reference::simulate(&m).outcome.quality
        } else {
            engine::simulate(&m).quality
        }
    } else if r {
        reference::explore(&m).outcome.quality
    } else {
        engine::explore(&m).quality
    };
    i::assert_fixed_quality(&json!(q.unwrap()), false, sample);
}

#[test]
fn quality_withdrawal_oracle_rejects_zero_and_partial_or_normal_estimates() {
    let cell = json!({"lower":null,"upper":null,"basis":null,"reason":"aborted"});
    let q = json!({"analysis":"exhaustive","checked":[],"properties":[{"expected_count":cell}],"cohort":[{"value":cell}],"outcomes_per_arrival":[{"expected_count":cell,"rate":cell}]});
    assert!(i::quality_is_withdrawn(&q));
    let mut zero = q.clone();
    zero["cohort"][0]["value"]["lower"] = json!("0");
    assert!(!i::quality_is_withdrawn(&zero));
    let mut checked = q.clone();
    checked["checked"] = json!(["expected_counts"]);
    assert!(!i::quality_is_withdrawn(&checked));
    let mut normal = q.clone();
    normal["properties"][0]["expected_count"]["reason"] = Value::Null;
    assert!(!i::quality_is_withdrawn(&normal));
}
pair!(quality_to_be_b_engine, quality_to_be_b_reference, |r| {
    to_be_b_quality(r, false)
});
pair!(
    quality_to_be_b_sample_engine,
    quality_to_be_b_sample_reference,
    |r| to_be_b_quality(r, true)
);

fn sample_outcome(m: &Model, r: bool) -> relca_core::outcome::SimulationOutcome {
    let o = if r {
        let result = reference::simulate(m);
        assert_eq!(result.stop, reference::StopReason::Complete);
        result.outcome
    } else {
        engine::simulate(m)
    };
    assert!(!o.diagnostics.iter().any(|d| d.code == "U-SEM-0"));
    assert_eq!(o.attempted_replications, 4);
    assert_eq!(o.completed_replications, 4);
    assert_eq!(o.replications.len(), 4);
    o
}

fn quality_execution(r: bool, sampled: bool) {
    for c in ["AsIs", "ToBeB"] {
        let m = model(i::QUALITY, Some(c), None);
        let old = model(&i::strip_quality(i::QUALITY), Some(c), None);
        if sampled {
            let mut a = sample_outcome(&m, r);
            let b = sample_outcome(&old, r);
            i::assert_fixed_quality(&json!(a.quality.take().unwrap()), c == "AsIs", true);
            assert_eq!(a, b, "quality must not change execution: {c}");
            for rep in &a.replications {
                assert_eq!(rep.cut, None);
                assert_eq!(rep.cases.len(), 1);
                assert_eq!(
                    rep.cases[0].terminal.as_ref().unwrap().end.as_deref(),
                    Some("done")
                );
            }
        } else {
            let mut a = outcome(&m, r);
            let b = outcome(&old, r);
            assert!(!a.diagnostics.iter().any(|d| d.code == "U-SEM-0"));
            assert!(!a.truncated);
            assert_eq!(a.terminals.len(), 1);
            let t = &a.terminals[0];
            assert_eq!(m.ends[t.end.0], "done");
            assert_eq!(
                (t.elapsed.min, t.elapsed.max, t.work.min, t.work.max),
                (1, 1, 1, 1)
            );
            i::assert_fixed_quality(&json!(a.quality.take().unwrap()), c == "AsIs", false);
            assert!(a == b, "quality must not change execution: {c}");
        }
    }
}
pair!(quality_execution_engine, quality_execution_reference, |r| {
    quality_execution(r, false)
});
pair!(
    quality_execution_sample_engine,
    quality_execution_sample_reference,
    |r| quality_execution(r, true)
);

#[test]
fn quality_rule_disclosure_follows_four_rows_and_non_quality_has_no_row() {
    let mut reports = Vec::new();
    for c in ["AsIs", "ToBeB"] {
        for sampled in [false, true] {
            let v = if sampled {
                json!(report::simulate_with_selection(i::QUALITY, Some(c), None))
            } else {
                check(i::QUALITY, Some(c), None)
            };
            reports.push((v, sampled));
            let old = i::strip_quality(i::QUALITY);
            let v = if sampled {
                json!(report::simulate_with_selection(&old, Some(c), None))
            } else {
                check(&old, Some(c), None)
            };
            assert!(
                !v["assumptions"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|a| a["field"] == "quality.rule")
            );
        }
    }
    for (v, sampled) in reports {
        i::assert_quality_assumptions(&v, sampled, true);
    }
}

fn held_quality(r: bool, sampled: bool, disclosure: bool) {
    for drain in [false, true] {
        for rule in [true, false] {
            let s = i::held_quality(drain, rule);
            let m = model(&s, None, Some("Day"));
            let old = model(&i::strip_quality(&s), None, Some("Day"));
            let q = if sampled {
                let mut o = sample_outcome(&m, r);
                let baseline = sample_outcome(&old, r);
                let q = json!(o.quality.take().unwrap());
                assert_eq!(o, baseline, "Day execution must not depend on quality");
                for rep in &o.replications {
                    assert_eq!(rep.cases.len(), 3);
                    for c in &rep.cases {
                        if drain {
                            assert_eq!(c.terminal.as_ref().unwrap().end.as_deref(), Some("done"));
                        } else {
                            assert_eq!(c.cut_off, Some(true));
                            assert!(c.terminal.is_none());
                        }
                    }
                }
                q
            } else {
                let mut o = outcome(&m, r);
                assert!(!o.truncated);
                assert!(!o.diagnostics.iter().any(|d| d.code == "U-SEM-0"));
                let cases = o.case_outcomes.as_ref().unwrap();
                assert_eq!(cases.len(), 3);
                for (k, c) in cases.iter().enumerate() {
                    assert_eq!(c.terminals.len(), 1);
                    let t = &c.terminals[0];
                    if drain {
                        let at = 5 * (k as u64 + 1);
                        let elapsed = at - k as u64;
                        assert!(matches!(
                            t.kind,
                            relca_core::outcome::CaseTerminalKind::End(_)
                        ));
                        assert_eq!(
                            t.completed_at,
                            Some(relca_core::outcome::Range { min: at, max: at })
                        );
                        assert_eq!(
                            t.elapsed,
                            Some(relca_core::outcome::Range {
                                min: elapsed,
                                max: elapsed
                            })
                        );
                        assert_eq!(t.work, Some(relca_core::outcome::Range { min: 5, max: 5 }));
                    } else {
                        assert_eq!(t.kind, relca_core::outcome::CaseTerminalKind::CutOff);
                        assert!(t.completed_at.is_none());
                    }
                }
                let q = json!(o.quality.take().unwrap());
                assert!(o == outcome(&old, r));
                q
            };
            {
                assert!(!q["checked"].as_array().unwrap().is_empty());
                let count = &q["properties"][0]["expected_count"];
                let rate = &q["cohort"][0]["value"];
                assert_eq!(count["reason"], Value::Null);
                assert_eq!(rate["reason"], Value::Null);
                if sampled {
                    assert_eq!(count["mean"], "3");
                    assert_eq!(rate["point"], "1");
                    assert_eq!(count["samples"], 4);
                } else {
                    assert_eq!(
                        count,
                        &json!({"lower":"3","upper":"3","basis":"exact","reason":null})
                    );
                    assert_eq!(
                        rate,
                        &json!({"lower":"1","upper":"1","basis":"exact","reason":null})
                    );
                }
            }
            if disclosure {
                let v = if sampled {
                    json!(report::simulate_with_selection(&s, None, Some("Day")))
                } else {
                    check(&s, None, Some("Day"))
                };
                i::assert_quality_assumptions(&v, sampled, rule);
            }
        }
    }
}
pair!(quality_day_engine, quality_day_reference, |r| held_quality(
    r, false, false
));
pair!(
    quality_day_sample_engine,
    quality_day_sample_reference,
    |r| held_quality(r, true, false)
);
#[test]
fn quality_day_disclosure_and_assumption_order() {
    held_quality(false, false, true);
}
#[test]
fn quality_day_sample_disclosure_and_assumption_order() {
    held_quality(false, true, true);
}

fn unreachable_rule_responsibility(c: &str) {
    let s = i::fixture("rule-partial.broken-t12");
    let v = check(&s, Some(c), None);
    assert_eq!(v["verdict"], "vacuous");
    term(&v, "held", 2);
    let got: Vec<_> = v["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["code"] == "W-RESP-0")
        .map(|d| d["message"].clone())
        .collect();
    assert_eq!(
        got,
        vec![
            json!("ApprovalRule has no declared responsibility"),
            json!("Ledger has no declared responsibility")
        ],
        "{c}"
    );
}
#[test]
fn unreachable_rule_u_keeps_responsibility_messages_as_is() {
    unreachable_rule_responsibility("AsIs");
}
#[test]
fn unreachable_rule_u_keeps_responsibility_messages_to_be_b() {
    unreachable_rule_responsibility("ToBeB");
}

#[test]
fn atomic_rule_u_is_after_process_u_and_not_j_person_or_generation_root() {
    for required in [false, true] {
        let v = check(&i::atomic_rule(required), None, None);
        assert_eq!(v["verdict"], "satisfied");
        term(&v, "posted", 3);
        if required {
            term(&v, "returned", 2);
        }
        let a = &v["agency"];
        let rows = a["viewpoints"]["effects"]["rows"].as_array().unwrap();
        assert_eq!(rows.len(), 1);
        let x = &rows[0];
        assert_eq!(x["e"], i::carrier());
        assert_eq!(
            x["state_refusers"],
            json!([{"actor":"Ledger","capability":"Ledger.post","reads":["Ledger.keys"],"on_normal_path":false}])
        );
        assert_eq!(x["process_judgements"], "derived");
        assert_eq!(x["j"], json!([i::atomic_judge(required)]));
        assert_eq!(
            x["u"],
            if required {
                json!([i::atomic_judge(true), i::atomic_rule_item()])
            } else {
                json!([])
            }
        );
        assert_eq!(
            a["refusal"],
            json!([]),
            "atomic legacy refusal remains underived"
        );
        assert!(
            a["unchecked"]
                .as_array()
                .unwrap()
                .contains(&json!("state_guards"))
        );
    }
}

#[test]
fn atomic_rule_u_responsibility_follows_actor_declaration_order() {
    for required in [false, true] {
        let source = i::atomic_rule(required);
        let v = check(&source, None, None);
        // ADR 0054 "範囲" / "意味": process U includes Approver when
        // the Rule requires approval, independently of atomic legacy refusal.
        let ast = relca_core::syntax::parse(&source).unwrap();
        let model = relca_core::lower::lower(&ast).unwrap();
        responsibility_oracle::assert_report(&model, &v);
        let got: Vec<_> = v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|d| d["code"] == "W-RESP-0")
            .map(|d| d["message"].clone())
            .collect();
        let names = if required {
            vec!["Approver", "Ledger", "R"]
        } else {
            vec!["Ledger"]
        };
        assert_eq!(
            got,
            names
                .iter()
                .map(|n| json!(format!("{n} has no declared responsibility")))
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn atomic_rule_is_not_a_concentration_carrier() {
    for required in [true, false] {
        let v = check(&i::atomic_rule(required), None, None);
        let x = &v["agency"]["viewpoints"]["effects"]["rows"][0];
        assert_eq!(x["concentration"]["single_actor"], false);
        assert_eq!(x["concentration"]["single_person"], "unknown");
        assert_eq!(x["concentration"]["single_generation_root"], true);
        assert_eq!(
            x["concentration"]["generation_roots"],
            json!([{"call":"e","capability":"Clerk.enter"}])
        );
        assert_eq!(x["j"], json!([i::atomic_judge(required)]));
    }
}
