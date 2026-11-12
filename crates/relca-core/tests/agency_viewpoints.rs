//! ADR0034 hand oracles. Missing viewpoints is intentionally RED before C2.
#[path = "support/agency_viewpoints_budget.rs"]
mod budget_driver;
#[path = "support/agency_viewpoints_acceptance.rs"]
mod hand;
use relca_core::{lower, report, syntax};
use serde_json::{Value, json};

fn pair(name: &str, fault: &str) {
    for n in [format!("agy-{name}"), format!("agy-{name}.broken-{fault}")] {
        hand::assert_report(&n, &hand::check(&n));
    }
}
macro_rules! pairs {($($test:ident:$name:literal,$fault:literal;)+)=>{$(#[test]fn $test(){pair($name,$fault);})+};}
pairs! {
 a01_o_people:"o-people","no-read";
 a02_o_does_not_prune_permission:"o-permit","one-executor";
 a03_anonymous_is_not_a_person:"o-unattributed","declared-executor";
 a04_declared_contest_route:"o-contest","route-no-read";
 a05_end_label_union:"f-select","merged-ends";
 a06_all_and_some_contexts:"f-or","and";
 a07_state_end_is_explicitly_not_derived:"f-state","binding-guard";
 a08_singleton_and_unreachable_end:"f-singleton","two-results";
 a09_zero_and_conditional_counts:"l-counts","unconditional";
 a10_requires_refusal_is_not_an_exercise_or_selector:"l-requires","no-requires";
 a11_fork_counts_binding_sites:"l-fork","missing-second-call";
 a12_any_counts_cancelled_siblings_and_none_end:"l-any","sequential";
 a13_names_do_not_create_proposal_contracts:"p-name","proposal-name";
 a14_actor_name_does_not_assign_person:"label","actor-rename";
 a15_retention_state_taint_is_row_local:"l-state-local","binding-guard";
 a16_soft_budget_public_controls:"soft-budget","one-unit-short";
 a17_legacy_budget_public_controls:"old-budget","legacy-limit";
 a18_invoice_entry_transfer:"invoice","ai-transfer";
 a19_compatibility_output_controls:"report-compat","old-key";
 a20_new_binding_is_existential:"f-new-binding","only-fail";
 a21_route_mutation_controls:"mutate","off-path-viewpoints";
 a22_cut_source_follows_legacy_origin:"cut-source","excluded-caller";
 a23_end_stops_f_l_but_not_lexical_o:"no-end-after-end","enable-suffix";
}

#[test]
fn every_input_is_preflighted_without_new_syntax_and_only_one_is_statically_rejected() {
    let p = hand::population();
    let mut accepted = 0;
    let mut rejected = vec![];
    for n in p["names"].as_array().unwrap() {
        let n = n.as_str().unwrap();
        let a = hand::check(n);
        let e = hand::expected(n);
        assert_eq!(a["verdict"], e["verdict"], "preflight {n}: {a}");
        if a["agency"].is_object() {
            accepted += 1;
        } else {
            rejected.push(n);
        }
        assert!(!hand::source(n).contains("//"));
        assert!(!hand::source(n).contains('#'));
    }
    assert_eq!(accepted, 45);
    assert_eq!(rejected, ["agy-cut-source.broken-excluded-caller"]);
}

#[test]
fn invoice_pair_keeps_original_structure_and_only_transfers_one_entry_line() {
    let original =
        std::fs::read_to_string(hand::root().join("examples/invoice/configurations.relca"))
            .unwrap();
    assert_eq!(hand::source("agy-invoice"), original);
    let broken = hand::replace(
        &original,
        "process AsIsFlow {\n    entry = Clerk.enter()",
        "process AsIsFlow {\n    entry = Extractor.extract()",
    );
    assert_eq!(hand::source("agy-invoice.broken-ai-transfer"), broken);
}

#[test]
fn runtime_regression_keeps_lower_diagnostic_prefix_out_of_engine_outcomes() {
    let mut complete = 0;
    for name in hand::population()["names"].as_array().unwrap() {
        let name = name.as_str().unwrap();
        if name == "agy-cut-source.broken-excluded-caller" {
            continue;
        }
        let s = hand::source(name);
        let ast = syntax::parse(&s).unwrap();
        let model = if name.starts_with("agy-invoice") {
            lower::lower_configuration(&ast, "AsIs").unwrap().model
        } else {
            lower::lower(&ast).unwrap()
        };
        let reference = relca_core::reference::explore(&model);
        assert_eq!(
            reference.stop,
            relca_core::reference::StopReason::Complete,
            "{name}: no hidden reference exclusions"
        );
        complete += 1;
        let engine = relca_core::engine::explore(&model);
        let prefix = model.diagnostics.len();
        assert_eq!(
            &reference.outcome.diagnostics[..prefix],
            model.diagnostics.as_slice(),
            "known lower prefix {name}"
        );
        assert_eq!(
            &reference.outcome.diagnostics[prefix..],
            engine.diagnostics.as_slice(),
            "engine must not prepend lower diagnostics {name}"
        );
        assert_eq!(
            reference.outcome.properties, engine.properties,
            "runtime properties {name}"
        );
    }
    assert_eq!(complete, 45);
    // Runtime equivalence is not the O/F/P/L oracle: both engines carry the
    // same static Agency. The A01..A23 hand payloads remain separate obligations.
}

fn check(s: &str) -> Value {
    serde_json::to_value(report::check(s)).unwrap()
}
fn vp(s: &str) -> Value {
    let a = check(s);
    assert!(a["agency"].is_object(), "auxiliary input must lower: {a}");
    assert!(a["agency"]["viewpoints"].is_object(), "missing viewpoints");
    hand::legacy_viewpoints(&a["agency"]["viewpoints"])
}

#[test]
fn metadata_source_kind_and_actor_rename_are_not_semantics() {
    let s = hand::source("agy-o-unattributed");
    let expected = hand::expected("agy-o-unattributed")["agency"]["viewpoints"].clone();
    for (from, to) in [
        ("actor Extractor {", "actor Extractor { grain model;"),
        ("actor Extractor {", "actor Extractor { grain person;"),
        (
            "actor Extractor {",
            "actor Extractor { boundary \"different boundary\";",
        ),
        (
            "actor Extractor {",
            "actor Extractor { via model \"Parser@v1\";",
        ),
        ("source document", "source record"),
        ("source document", "source tacit \"declared source\""),
    ] {
        let changed = hand::replace(&s, from, to);
        assert_eq!(vp(&changed), expected, "{to}");
    }
    let renamed = hand::replace(&s, "Extractor", "Worker");
    let inverse = vp(&renamed).to_string().replace("Worker", "Extractor");
    assert_eq!(serde_json::from_str::<Value>(&inverse).unwrap(), expected);
}

#[test]
fn runtime_clock_and_cases_do_not_cut_or_multiply_static_counts() {
    let s = hand::source("agy-o-people");
    let expected = hand::expected("agy-o-people")["agency"]["viewpoints"].clone();
    let clock = hand::replace(&s, "clock 100min", "clock 0min");
    let a = check(&clock);
    assert_eq!(a["verdict"], "inconclusive");
    assert_eq!(vp(&clock), expected);
    let cases = hand::replace(
        &s,
        "bounds { cases 1;",
        "arrivals fixed [0min,0min]\n policy fifo tie_by case_id\n bounds { cases 2;",
    );
    let a = check(&cases);
    assert!(a["agency"].is_object(), "{a}");
    assert_eq!(vp(&cases), expected);
    assert!(
        a["agency"]["unchecked"]
            .as_array()
            .unwrap()
            .contains(&json!("multi_case_interference"))
    );
}

#[test]
fn declaration_order_shared_person_and_direct_indirect_overlap_are_preserved() {
    let s = hand::source("agy-o-people");
    let s = hand::replace(&s, "people [c1,c2,a1]", "people [zeta,alpha]");
    let s = hand::replace(&s, "executors [c1,c2]", "executors [zeta,alpha]");
    let s = hand::replace(&s, "executors [a1]", "executors [alpha]");
    let s = hand::replace(
        &s,
        "approve(e:T) -> T {",
        "approve(e:T) -> T { reads [Invoice];",
    );
    let v = vp(&s);
    let o = &v["observation"];
    assert_eq!(
        o["sources"][0]["normal"]["actors"],
        json!(["Clerk", "Approver"])
    );
    assert_eq!(
        o["sources"][0]["normal"]["people"],
        json!(["zeta", "alpha"])
    );
    assert_eq!(
        o["people"],
        json!([
            {"person":"zeta","normal":{"direct":["Invoice"],"indirect":[]},"contest":{"direct":[]}},
            {"person":"alpha","normal":{"direct":["Invoice"],"indirect":["Invoice"]},"contest":{"direct":[]}}
        ])
    );
}

#[test]
fn authority_and_separation_checks_do_not_reduce_possible_observers() {
    let s = hand::source("agy-o-people");
    let s = hand::replace(
        &s,
        "process Flow {",
        r#"permit c1 to Clerk.enter
      authority Grant { basis "table"; grants c1 to Clerk.enter; issued_by a1 }
      contract Controls { permitted Clerk.enter; authority_basis Clerk.enter Grant; separation Clerk.enter.finish, Approver.approve.finish }
      process Flow {"#,
    );
    let a = check(&s);
    assert_eq!(
        a["verdict"], "violated",
        "unpermitted c2 is still explored: {a}"
    );
    assert_eq!(
        vp(&s),
        hand::expected("agy-o-people")["agency"]["viewpoints"]
    );
}

#[test]
fn requires_result_guard_keeps_singleton_post_read_only_and_writer_selectable() {
    let s = hand::source("agy-l-requires");
    let s = hand::replace(
        &s,
        "p = Post.post(); end done",
        "p = Post.post(); if p == posted { end done } else { end returned }",
    );
    let mut want = hand::expected("agy-l-requires")["agency"]["viewpoints"].clone();
    // posted/denied follow j, but denied is an otherwise value, not a result
    // the singleton Post operation can choose. Explicitly reading p makes p
    // context support; j selects both labels, p never selects either.
    want["framing"]["ends"] = json!(["done", "returned"].map(
        |end| json!({"end":end,"status":"derived","reachable":true,
        "selectors":[{"call":"j","capability":"Judge.check","selects":"all_contexts"}],
        "read_only":[{"call":"p","capability":"Post.post"}]})
    ));
    want["framing"]["actors"] = json!(["Judge"]);
    assert_eq!(vp(&s), want);
}

#[test]
fn unknown_requires_taints_only_its_exercise_and_result_dependent_suffix() {
    let s = hand::source("agy-l-requires");
    let s = hand::replace(&s, "writes approval;", "");
    let s = hand::replace(
        &s,
        "process Flow {",
        r#"actor Worker { capacity 1; capability work() -> T { duration fixed 1min; result nondet { posted } } }
      actor Always { capacity 1; capability work() -> T { duration fixed 1min; result nondet { posted } } }
      process Flow {"#,
    );
    let s = hand::replace(
        &s,
        "include [Judge,Post]",
        "include [Judge,Post,Worker,Always]",
    );
    let s = hand::replace(
        &s,
        "p = Post.post(); end done",
        "p = Post.post(); if p == posted { w = Worker.work() } else {}; z = Always.work(); end done",
    );
    let v = vp(&s);
    assert_eq!(
        v["framing"]["ends"],
        json!([{"end":"done","status":"not_derived","reason":"state_guards"}])
    );
    assert_eq!(v["framing"]["complete"], false);
    assert_eq!(
        v["retention"]["capabilities"],
        json!([
            {"capability":"Judge.check","status":"derived","normal":{"min":1,"max":1},"contest_route":false,"basis":"unknown","fallback":"unchecked","join_any_cancellation":false},
            {"capability":"Post.post","status":"not_derived","reason":"state_guards","contest_route":false,"basis":"unknown","fallback":"unchecked","join_any_cancellation":false},
            {"capability":"Worker.work","status":"not_derived","reason":"state_guards","contest_route":false,"basis":"unknown","fallback":"unchecked","join_any_cancellation":false},
            {"capability":"Always.work","status":"derived","normal":{"min":1,"max":1},"contest_route":false,"basis":"unknown","fallback":"unchecked","join_any_cancellation":false}
        ])
    );
}

#[test]
fn sources_capabilities_and_end_rows_follow_declaration_not_lexical_name_order() {
    let s = r#"model Order version "0.2" {
      type V = enum { pass,fail }
      actor ZSource { source record }
      actor ASource { source document }
      actor ZWorker { capacity 1; capability zebra() -> V { reads [ZSource,ASource]; duration fixed 1min; result nondet { pass,fail } } }
      actor AWorker { capacity 1; capability alpha() -> V { duration fixed 1min; result nondet { pass,fail } } }
      process P { z = ZWorker.zebra(); a = AWorker.alpha(); if a == pass { end zebra } else { end alpha } }
      cut K { include [ZSource,ASource,ZWorker,AWorker] }
      bounds { cases 1; live_tasks 2; clock 10min }
      verify safety no_resource_overcommit
    }"#;
    let v = vp(s);
    assert_eq!(
        v["observation"]["sources"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["actor"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["ZSource", "ASource"]
    );
    assert_eq!(
        v["retention"]["capabilities"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["capability"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["ZWorker.zebra", "AWorker.alpha"]
    );
    assert_eq!(
        v["framing"]["ends"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["end"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["zebra", "alpha"]
    );
    assert_eq!(v["framing"]["actors"], json!(["AWorker"]));
    for r in v["retention"]["capabilities"].as_array().unwrap() {
        assert_eq!(r["normal"], json!({"min":1,"max":1}));
    }
}

#[test]
fn drain_only_any_has_no_cancellation_flag_and_shared_executor_does_not_prune_sibling() {
    let s = hand::source("agy-l-any");
    let drain = hand::replace(&s, "remaining cancel_then_drain", "remaining drain");
    let mut expected = hand::expected("agy-l-any")["agency"]["viewpoints"].clone();
    for c in expected["retention"]["capabilities"]
        .as_array_mut()
        .unwrap()
    {
        c["join_any_cancellation"] = json!(false);
    }
    assert_eq!(vp(&drain), expected);
    let shared = hand::replace(&s, "type V =", "people [p]\n type V =");
    let shared = hand::replace(&shared, "capacity 1", "executors [p]");
    assert_eq!(
        vp(&shared)["retention"],
        hand::expected("agy-l-any")["agency"]["viewpoints"]["retention"]
    );
}

#[test]
fn latent_policy_and_actor_state_reads_stay_rejected_without_viewpoints() {
    let s = hand::source("agy-l-state-local");
    let latent = hand::replace(&s, "observed case", "latent case");
    let reads = hand::replace(&s, "writes flag;", "writes flag; reads [Post.flag];");
    for (s, verdict, code) in [
        (latent, "invalid", "E-POL-LATENT"),
        (reads, "unsupported", "U-SYN-0"),
    ] {
        let a = check(&s);
        assert_eq!(a["verdict"], verdict, "{a}");
        assert!(a["agency"].is_null());
        assert!(
            a["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["code"] == code)
        );
    }
}

#[test]
fn empty_builder_has_no_viewpoints_and_old_json_deserializes() {
    let empty = serde_json::to_value(relca_core::agency::Agency::empty()).unwrap();
    assert!(empty.get("viewpoints").is_none());
    let mut m = lower::lower(&syntax::parse(&hand::source("agy-f-or")).unwrap()).unwrap();
    let mut old = serde_json::to_value(&m.agency).unwrap();
    old.as_object_mut().unwrap().remove("viewpoints");
    // Infer the public Agency type from the IR without importing an internal module.
    m.agency = serde_json::from_value(old.clone()).unwrap();
    assert_eq!(serde_json::to_value(&m.agency).unwrap(), old);
}

#[test]
fn public_serde_accepts_new_table_but_rejects_unknown_nested_fields_and_status() {
    let mut a = serde_json::to_value(relca_core::agency::Agency::empty()).unwrap();
    a["viewpoints"] = hand::expected("agy-o-people")["agency"]["viewpoints"].clone();
    let parsed: Result<relca_core::agency::Agency, _> = serde_json::from_value(a.clone());
    assert!(
        parsed.is_ok(),
        "ADR0034 public serde viewpoints is not implemented: {parsed:?}"
    );
    assert_eq!(serde_json::to_value(parsed.unwrap()).unwrap(), a);
    for pointer in [
        "/viewpoints",
        "/viewpoints/observation",
        "/viewpoints/observation/sources/0",
        "/viewpoints/observation/sources/0/normal",
        "/viewpoints/framing",
        "/viewpoints/framing/ends/0",
        "/viewpoints/proposal",
        "/viewpoints/retention",
        "/viewpoints/retention/capabilities/0",
    ] {
        let mut bad = a.clone();
        bad.pointer_mut(pointer)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("unknown_field".into(), json!(true));
        assert!(
            serde_json::from_value::<relca_core::agency::Agency>(bad).is_err(),
            "unknown field {pointer}"
        );
    }
    let mut bad = a;
    bad["viewpoints"]["observation"]["status"] = json!("maybe");
    assert!(serde_json::from_value::<relca_core::agency::Agency>(bad).is_err());
}

fn auxiliary_inputs() -> Vec<String> {
    let s = hand::source("agy-o-unattributed");
    let mut out = vec![];
    for (from, to) in [
        ("actor Extractor {", "actor Extractor { grain model;"),
        ("actor Extractor {", "actor Extractor { grain person;"),
        (
            "actor Extractor {",
            "actor Extractor { boundary \"different boundary\";",
        ),
        (
            "actor Extractor {",
            "actor Extractor { via model \"Parser@v1\";",
        ),
        ("source document", "source record"),
        ("source document", "source tacit \"declared source\""),
    ] {
        out.push(hand::replace(&s, from, to));
    }
    let s = hand::source("agy-o-people");
    out.push(hand::replace(&s, "clock 100min", "clock 0min"));
    out.push(hand::replace(
        &s,
        "bounds { cases 1;",
        "arrivals fixed [0min,0min]\n policy fifo tie_by case_id\n bounds { cases 2;",
    ));
    let s = hand::source("agy-l-any");
    out.push(hand::replace(
        &s,
        "remaining cancel_then_drain",
        "remaining drain",
    ));
    out.push(hand::replace(
        &hand::replace(&s, "type V =", "people [p]\n type V ="),
        "capacity 1",
        "executors [p]",
    ));
    let s = hand::source("agy-o-people");
    let s = hand::replace(&s, "people [c1,c2,a1]", "people [zeta,alpha]");
    let s = hand::replace(&s, "executors [c1,c2]", "executors [zeta,alpha]");
    let s = hand::replace(&s, "executors [a1]", "executors [alpha]");
    out.push(hand::replace(
        &s,
        "approve(e:T) -> T {",
        "approve(e:T) -> T { reads [Invoice];",
    ));
    out
}
#[test]
fn all_auxiliary_models_are_preflighted_before_new_table_assertions() {
    for s in auxiliary_inputs() {
        let a = check(&s);
        assert!(
            a["agency"].is_object(),
            "auxiliary input must lower: {a}\n{s}"
        );
    }
}

#[test]
fn empty_source_people_and_capability_collections_are_derived_not_unknown() {
    let s = r#"model Empty version "0.2" {
      process P { end done }
      bounds { cases 1; live_tasks 1; clock 1min }
      verify safety no_resource_overcommit
    }"#;
    let v = vp(s);
    assert_eq!(
        v["observation"],
        json!({"status":"derived","sources":[],"people":[],"fallback":"unchecked"})
    );
    assert_eq!(
        v["retention"],
        json!({"status":"derived","capabilities":[],"notice":"行使回数から不要とは判定しない","unchecked":["fallback_dependency","retention_basis","actual_skill"]})
    );
    assert_eq!(
        v["framing"]["ends"],
        json!([{"end":"done","status":"derived","reachable":true,"selectors":[],"read_only":[]}])
    );
}

#[test]
fn declared_route_is_deduplicated_and_survives_bad_authority_or_unreachable_target() {
    let s = hand::source("agy-o-contest");
    let want = hand::expected("agy-o-contest")["agency"]["viewpoints"]["observation"].clone();
    let duplicate = hand::replace(
        &s,
        "contest Vendor on end.returned route Clerk.recheck reversible_by [a1] respond_by 5min",
        "contest Vendor on end.returned route Clerk.recheck reversible_by [a1] respond_by 5min\n contest Vendor on end.returned route Clerk.recheck reversible_by [a1] respond_by 5min",
    );
    let bad_authority = hand::replace(
        &s,
        "grants a1 to Approver.approve",
        "grants c1 to Approver.approve",
    );
    let unreachable = hand::replace(
        &s,
        "capability approve(e:Entry) -> Verdict { duration fixed 1min; result nondet { pass,fail }",
        "capability approve(e:Entry) -> Verdict { duration fixed 1min; result nondet { pass }",
    );
    for changed in [duplicate, bad_authority, unreachable] {
        let a = check(&changed);
        assert!(a["agency"].is_object(), "route input must lower: {a}");
        assert_eq!(vp(&changed)["observation"], want);
    }
}

#[test]
fn kernel_positive_support_not_weights_or_latent_reads_controls_static_viewpoints() {
    let s = r#"model KernelSupport version "0.2" {
      type V = enum { pass,fail }
      actor Secret { source document; latent h: bool = false }
      kernel K given [Secret.h] { false -> { 0.25: pass; 0.75: fail }; true -> { 0.25: pass; 0.75: fail } }
      actor Checker { capacity 1; capability check() -> V { duration fixed 1min; outcomes kernel K; kernel_reads latent [Secret.h] } }
      process P { j = Checker.check(); if j == pass { end done } else { end returned } }
      cut KCut { include [Secret,Checker] }
      bounds { cases 1; live_tasks 1; clock 2min }
      verify safety no_resource_overcommit
    }"#;
    let mut expected = hand::expected("agy-f-select")["agency"]["viewpoints"].clone();
    expected["observation"]["sources"] = json!([{"actor":"Secret","normal":{"actors":[],"people":[],"unattributed_actors":[]},"contest":{"actors":[],"people":[],"unattributed_actors":[],"routes":[]}}]);
    assert_eq!(vp(s), expected);
    let weights = hand::replace(s, "0.25: pass; 0.75: fail", "0.9: pass; 0.1: fail");
    assert_eq!(vp(&weights), expected);
    let singleton = hand::replace(s, "0.25: pass; 0.75: fail", "1: pass; 0: fail");
    expected["framing"]["actors"] = json!([]);
    expected["framing"]["ends"] = json!([
        {"end":"done","status":"derived","reachable":true,"selectors":[],"read_only":[{"call":"j","capability":"Checker.check"}]},
        {"end":"returned","status":"derived","reachable":false,"selectors":[],"read_only":[]}
    ]);
    assert_eq!(vp(&singleton), expected);
}
