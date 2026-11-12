//! ADR0032: hand arithmetic precedes both execution implementations.
#[path = "support/actor_observed_acceptance.rs"]
mod h;
use relca_core::{engine, explain, ir, reference, report};
use serde_json::{Value, json};

#[test]
fn classification_repair_for_qualified_noninteger_comparison_without_new_state() {
    for op in ["!=", "<", "<=", ">", ">="] {
        let s = format!(
            "model M version \"0.2\" {{ type T=enum{{ok}}; actor A{{latent x:T=ok}} process P{{if A.x {op} ok {{end yes}} else {{end no}}}} bounds{{cases 1;live_tasks 1;clock 2min}} verify safety no_resource_overcommit }}"
        );
        h::reject(&s, "U-SYN-0");
    }
}

#[test]
fn classification_repair_for_qualified_latent_finish_without_new_state() {
    let s = "model M version \"0.2\" { type T=enum{ok}; actor A{latent x:T=ok;capacity 1;capability c()->T{writes x;finish{A.x=ok};duration fixed 1min;result nondet{ok}}} process P{a=A.c();end done} bounds{cases 1;live_tasks 1;clock 2min} verify safety no_resource_overcommit }";
    h::reject(s, "U-SYN-0");
}

macro_rules! accept { ($($test:ident => $base:literal),+ $(,)?) => { $(#[test] fn $test() {
    for name in h::names().into_iter().filter(|n|n==$base || n.starts_with(&format!("{}.broken-",$base))) {
        h::subset(&h::expected(&name),&h::check(&name));
        let want=h::expected(&name);
        if ["unsupported","invalid"].contains(&want["verdict"].as_str().unwrap()) {
            h::reject(&h::source(&name),want["diagnostics"][0]["code"].as_str().unwrap());
        } else { h::both(&name); }
    }
} )+ }; }
accept! {
 f01_counter_range => "aobs-counter", f02_optional_result => "aobs-optional",
 f03_owner => "aobs-owner", f04_capacity => "aobs-capacity",
 f05_shared_orders_and_refusal => "aobs-shared-order", f06_start_visible => "aobs-start-visible",
 f07_policy_and_condition_support => "aobs-policy", f08_cut => "aobs-cut",
 f09_late_admission => "aobs-late", f10_cancel => "aobs-cancel",
 f11_symbolic_initial_runtime => "aobs-symbolic-initial", f12_symbolic_prior_writer_runtime => "aobs-symbolic-writer",
 f13_reserved => "aobs-reserved", f14_labels => "aobs-labels",
 f15_authority_scope => "aobs-scope", f16_responsibility => "aobs-responsibility",
 f17_read_write_order => "aobs-read-write-order", f18_invariant_postwrite => "aobs-postwrite",
 f19_truncation => "aobs-truncation", f20_per_case => "aobs-per-case",
 f21_names => "aobs-names", f22_comparison => "aobs-integer-compare",
 f23_kernel => "aobs-kernel"
}

#[test]
fn population48_pairs24_and_every_replacement_takes_effect() {
    let mut actual: Vec<_> = std::fs::read_dir(h::root().join("fixtures"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("aobs-"))
        .collect();
    actual.sort();
    assert_eq!(actual, h::names());
    assert_eq!(actual.len(), 48);
    assert_eq!(h::pairs().len(), 24);
    for p in h::pairs() {
        let name = p["name"].as_str().unwrap();
        let s = h::source(name);
        let t = h::change(&s, p["from"].as_str().unwrap(), p["to"].as_str().unwrap());
        assert_eq!(
            t,
            h::source(&format!("{name}.broken-{}", p["fault"].as_str().unwrap()))
        );
    }
    for name in actual {
        let s = h::source(&name);
        assert!(!s.contains("//") && !s.contains('#'));
    }
}

#[test]
fn all24_legacy_grammar_controls_are_source_bound_and_accepted() {
    let controls: Vec<Value> =
        serde_json::from_str(include_str!("support/actor_observed_legacy_controls.json")).unwrap();
    assert_eq!(controls.len(), 24);
    for row in controls {
        let name = row["name"].as_str().unwrap();
        assert_eq!(
            h::source(name),
            row["original"].as_str().unwrap(),
            "control stale {name}"
        );
        let s = row["legacy_control"].as_str().unwrap();
        assert_ne!(s, row["original"].as_str().unwrap());
        h::model(s);
    }
    let (static_count, sim_count) = h::names().iter().fold((0, 0), |(st, sim), name| {
        if name.starts_with("aobs-simulation") {
            (st, sim + 1)
        } else if ["invalid", "unsupported"]
            .contains(&h::expected(name)["verdict"].as_str().unwrap())
        {
            (st + 1, sim)
        } else {
            (st, sim)
        }
    });
    assert_eq!(
        (static_count, sim_count, 48 - static_count - sim_count),
        // ADR0041: aobs-capacity.broken-two-writers moves static -> runtime
        // (commutative Add); total48 and all other hand times are unchanged.
        (9, 2, 37)
    );
}

#[test]
fn symbolic_unknown_precedes_initial_and_prior_store_lookup() {
    for name in [
        "aobs-symbolic-initial",
        "aobs-symbolic-initial.broken-initial-no",
        "aobs-symbolic-writer",
    ] {
        let v = h::check(name);
        assert_eq!(v["verdict"], "satisfied");
        assert_eq!(
            v["agency"]["refusal"],
            json!([]),
            "Unknown must not manufacture a refusal proof: {name}"
        );
        assert_eq!(
            v["agency"]["unchecked"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|w| **w == "state_guards")
                .count(),
            1
        );
    }
    let control = h::check("aobs-symbolic-writer.broken-case-storage");
    let rs = control["agency"]["refusal"].as_array().unwrap();
    assert_eq!(rs.len(), 1);
    assert_eq!(rs[0]["reachable"], true);
    for name in ["aobs-policy", "aobs-integer-compare"] {
        let v = h::check(name);
        assert_eq!(v["verdict"], "satisfied");
        assert_eq!(
            v["agency"]["unchecked"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|w| **w == "state_guards")
                .count(),
            1
        );
    }
}

#[test]
fn six_integer_comparisons_and_strict_legacy_classification_boundaries() {
    let s = h::source("aobs-integer-compare");
    for (op, truth) in [
        ("==", true),
        ("!=", false),
        ("<", false),
        ("<=", true),
        (">", false),
        (">=", true),
    ] {
        let t = h::change(&s, "A.n < 2", &format!("A.n {op} 1"));
        let m = h::model(&t);
        let r = reference::explore(&m);
        assert_eq!(r.stop, reference::StopReason::Complete);
        for out in [&engine::explore(&m), &r.outcome] {
            assert_eq!(out.terminals.len(), 1);
            assert_eq!(out.terminals[0].elapsed.min, if truth { 1 } else { 3 });
            assert_eq!(out.terminals[0].elapsed.max, if truth { 1 } else { 3 });
        }
    }
    for op in ["!=", "<", "<=", ">", ">="] {
        let src = format!(
            "model M version \"0.2\" {{ type T=enum {{ok}}; actor A {{latent x:T=ok}}; process P {{if A.x {op} ok {{end done}} else {{end other}}}} bounds {{cases 1;live_tasks 1;clock 2min}} verify safety no_resource_overcommit }}"
        );
        h::reject(&src, "U-SYN-0");
        let naked = h::change(&src, "actor A {latent x:T=ok}", "observed case {x:T=ok}");
        let naked = h::change(&naked, "A.x", "x");
        h::reject(&naked, "E-SYN-0");
    }
    let t = h::change(&s, "A.n < 2", "A.n == ok");
    h::reject(&t, "E-TYPE-0");
    let t = h::change(&s, "A.n < 2", "A.n < A.n");
    h::reject(&t, "U-SYN-0");
    let t = h::change(&s, "A.n < 2", "A.n <");
    h::reject(&t, "E-SYN-0");
    let legacy = "model M version \"0.2\" { type T=enum{ok}; actor L {latent n:T=ok; capacity 1; capability c()->T {writes n;finish {L.n=ok};duration fixed 1min;result nondet{ok}}} process P{a=L.c();end done} bounds{cases 1;live_tasks 1;clock 2min} verify safety no_resource_overcommit }";
    h::reject(legacy, "U-SYN-0");
}

#[test]
fn aliases_owners_defaults_and_per_case_static_matrix() {
    let s = h::source("aobs-owner");
    for writes in ["A.x", "x"] {
        for finish in ["A.x", "x"] {
            for start in ["A.x", "x"] {
                let t = h::change(
                    &s,
                    "writes A.x; finish { A.x = ok }",
                    &format!(
                        "writes {writes}; finish {{ {finish} = ok }}; start {{ {start} = ok }}"
                    ),
                );
                assert_eq!(report::check(&t).verdict, relca_core::Verdict::Satisfied);
            }
        }
    }
    for (a, b, code) in [
        ("writes A.x", "writes B.x", "U-SYN-0"),
        ("finish { A.x = ok }", "finish { B.x = ok }", "U-SYN-0"),
        ("writes A.x", "start { B.x = ok }; writes A.x", "U-SYN-0"),
        ("writes A.x", "writes [x,A.x]", "U-SEM-0"),
        (
            "finish { A.x = ok }",
            "finish { x = ok; A.x = ok }",
            "U-SEM-0",
        ),
        (
            "finish { A.x = ok }",
            "finish { A.missing = ok }",
            "E-NAME-0",
        ),
        (
            "state x: R = ok; capability",
            "state x: R = ok; latent x: R = ok; capability",
            "U-SEM-0",
        ),
        (
            "state x: R = ok; capability",
            "state x: R ~ { ok:1 }; capability",
            "U-SEM-0",
        ),
        (
            "state x: R = ok; capability",
            "state x: R = true; capability",
            "E-TYPE-0",
        ),
    ] {
        h::reject(&h::change(&s, a, b), code);
    }
    let s = h::source("aobs-per-case");
    for addition in [
        "capacity 0;",
        "executors [p];",
        "capability illegal() -> R { duration fixed 0min; result nondet { ok } };",
    ] {
        let with_person = h::change(&s, "actor Doc", "people [p]; actor Doc");
        h::reject(
            &h::change(
                &with_person,
                "instance per_case;",
                &format!("instance per_case; {addition}"),
            ),
            "U-SEM-0",
        );
    }
    // ADR0041 accepts Add through C. Preserve the old unresolved-writer
    // obligation with the still-unsupported constant overwrite, not Add.
    let s = h::change(&h::source("aobs-capacity"), "posted += 1", "posted = 1");
    for cap in ["capacity 0", "capacity 2"] {
        h::reject(&h::change(&s, "capacity 1", cap), "U-SEM-0");
    }
    let t = h::change(&s, "duration fixed 1min", "duration fixed 0min");
    h::reject(&h::change(&t, "capacity 1", ""), "U-SEM-0");
    let t = h::change(&s, "capacity 1", "executors [p,q]");
    h::reject(&h::change(&t, "type R", "people [p,q]; type R"), "U-SEM-0");
}

#[test]
fn all_condition_contexts_keep_cut_and_latent_boundaries() {
    for name in [
        "aobs-policy",
        "aobs-scope",
        "aobs-responsibility",
        "aobs-integer-compare",
    ] {
        let s = h::source(name);
        let (from, to) = match name {
            "aobs-policy" => (
                "include [Desk]",
                "exclude Desk reason \"out\" loses \"state\"",
            ),
            "aobs-scope" => (
                "include [Doc,A]",
                "include [A]; exclude Doc reason \"out\" loses \"state\"",
            ),
            "aobs-responsibility" => (
                "include [Doc,Reader]",
                "include [Reader]; exclude Doc reason \"out\" loses \"state\"",
            ),
            _ => ("include [A]", "exclude A reason \"out\" loses \"state\""),
        };
        // Remove Doc's capability evidence read to isolate the duty-condition Cut.
        let s = if name == "aobs-responsibility" {
            h::change(&s, "reads [Doc];", "")
        } else {
            s
        };
        h::reject(&h::change(&s, from, to), "E-CUT-1");
    }
    let s = h::source("aobs-symbolic-initial");
    // Without Actor state this is a pre-ADR 0032 model: the bare requires name
    // keeps the old case-variable diagnostic (boundary-review fix 1).
    h::reject(&h::change(&s, "state permit", "latent permit"), "E-NAME-0");
    let qualified = h::change(&s, "requires permit == ok", "requires A.permit == ok");
    assert_eq!(
        report::check(&qualified).verdict,
        relca_core::Verdict::Satisfied
    );
    // An excluded state owner read from another capability's requires.
    let s = h::source("aobs-cut");
    let s = h::change(
        &s,
        "duration fixed 1min",
        "requires D.flag == true else no; duration fixed 1min",
    );
    h::reject(
        &h::change(
            &s,
            "include [D,Probe]",
            "include [Probe]; exclude D reason \"out\" loses \"state\"",
        ),
        "E-CUT-1",
    );
    let s = h::source("aobs-policy");
    let s = h::change(&s, "state mode", "latent mode");
    let s = h::change(&s, "process P {", "process Unselected {");
    let s = h::change(
        &s,
        "cut C",
        "process Selected { end done }; configuration Active { process Selected; cut C }; cut C",
    );
    assert_eq!(
        report::check_configuration(&s, "Active").verdict,
        relca_core::Verdict::Invalid
    );
}

#[test]
fn observed_case_name_collision_is_independent_of_declaration_order() {
    let s = h::source("aobs-names.broken-case-clash");
    h::reject(&s, "U-SEM-0");
    let t = h::change(
        &s,
        "observed case { flag: bool = false }; actor A { state flag: bool = false }",
        "actor A { state flag: bool = false }; observed case { flag: bool = false }",
    );
    h::reject(&t, "U-SEM-0");
}

#[test]
fn counter_legacy_and_one_arrival_and_budget_zero_are_distinct() {
    for name in ["aobs-counter", "aobs-counter.broken-overflow"] {
        let s = h::source(name);
        let t = h::change(&s, "cut C", "arrivals fixed [0min]; cut C");
        let m = h::model(&t);
        let e = engine::explore(&m);
        let r = reference::explore(&m);
        assert_eq!(r.stop, reference::StopReason::Complete);
        assert_eq!(e.properties, r.outcome.properties);
        assert_eq!(e.state_range_violations, r.outcome.state_range_violations);
        if name.contains("broken") {
            assert_eq!(e.state_range_violations.len(), 1);
            let w = &e.state_range_violations[0];
            assert_eq!(
                (
                    w.variable.as_str(),
                    w.before.as_str(),
                    w.attempted.as_str(),
                    w.time
                ),
                ("Desk.handled", "2", "3", 6)
            );
            assert!(!e.truncated);
        } else {
            let v = h::projection(&m, &e);
            h::terminal(&v, 1, "done", (4, 4), (4, 4), (4, 4));
        }
    }
    let m = h::model(&h::source("aobs-counter"));
    let e = engine::explore_with_budget(&m, 0);
    assert!(e.truncated);
    assert!(e.terminals.is_empty());
    assert!(e.state_range_violations.is_empty());
    assert!(e.diagnostics.iter().any(|d| d.code == "W-BUDGET-0"));
    assert!(
        e.properties
            .iter()
            .all(|p| p.verdict == relca_core::Verdict::Inconclusive)
    );
}

#[test]
fn kernel_given_with_and_independent_result_write() {
    let s = h::source("aobs-kernel");
    let s = h::change(&s, "state flag: bool = true", "state flag: R = no");
    let s = h::change(&s, "outcomes kernel K", "writes flag; outcomes kernel K");
    let m = h::model(&s);
    let e = engine::explore(&m);
    let r = reference::explore(&m);
    assert_eq!(r.stop, reference::StopReason::Complete);
    assert_eq!(e.properties, r.outcome.properties);
    h::reject(&h::change(&s, "1: ok", "1: ok with flag = ok"), "U-SEM-0");
    h::reject(
        &h::change(
            &s,
            "kernel K { * -> { 1: ok } }",
            "kernel K given [A.flag] { ok -> { 1: ok }; no -> { 1: ok } }",
        ),
        "U-SEM-0",
    );
    let s = h::source("aobs-counter");
    h::reject(&h::change(&s, "state handled", "latent handled"), "U-SYN-0");
}

fn reject_ir(m: &ir::Model) {
    // Neither entry may rely on lowering or Agency derivation to detect forgery.
    for out in [engine::explore(m), reference::explore(m).outcome] {
        assert!(
            out.diagnostics.iter().any(|d| d.code == "U-SEM-0"),
            "forged IR: {out:?}"
        );
    }
}
#[test]
fn direct_ir_owner_capacity_initial_and_kernel_guards_fail_closed_without_panics() {
    let good = h::model(&h::source("aobs-owner"));
    let mut m = good.clone();
    m.capabilities[0].actor = ir::ActorId(1);
    reject_ir(&m);
    let mut m = good.clone();
    let removed_owner = m.state_variables[1].kind;
    m.actors.pop();
    m.state_variables.truncate(1);
    m.state_variables[0].kind = removed_owner;
    m.capabilities[0].writes.clear();
    m.cut = None;
    m.agency = relca_core::agency::Agency::empty();
    // All remaining calls/targets have valid IDs. Only the unused state owner
    // is out of range, so a different malformed Capability cannot mask it.
    reject_ir(&m);
    for initial in [ir::InitialValue::Nondet, ir::InitialValue::Distribution(0)] {
        let mut m = good.clone();
        if matches!(initial, ir::InitialValue::Distribution(_)) {
            let ir::InitialValue::Fixed(value) = good.state_variables[0].initial else {
                unreachable!()
            };
            m.initial_distributions = vec![vec![(value, relca_core::probability::Rational::one())]];
        }
        m.state_variables[0].initial = initial;
        reject_ir(&m);
    }
    let mut m = good.clone();
    m.capabilities[0].writes[0].variable = ir::StateVarId(usize::MAX);
    reject_ir(&m);
    let mut m = good.clone();
    m.state_variables[0].base = ir::TypeId(usize::MAX);
    reject_ir(&m);
    let mut m = good.clone();
    m.actors[0].metadata.instance = Some(ir::ActorInstance::PerCase);
    reject_ir(&m);
    let mut m = good.clone();
    m.capabilities[0].start_writes = m.capabilities[0].writes.clone();
    m.capabilities[0].start_writes[0].source = ir::WriteSource::Result;
    reject_ir(&m);
    let mut m = h::model(&h::change(
        &h::source("aobs-capacity"),
        "posted += 1",
        "posted = 1",
    ));
    m.actors[0].capacity = Some(2);
    reject_ir(&m);
    let good = h::model(&h::source("aobs-kernel"));
    let mut m = good.clone();
    m.capabilities[0].kernel_reads_latent = vec![ir::StateVarId(0)];
    reject_ir(&m);
    let actor_kind = good.state_variables[0].kind;
    for s in [CASE_GIVEN, CASE_WITH] {
        let mut m = h::model(s);
        let control = reference::explore(&m);
        assert_eq!(control.stop, reference::StopReason::Complete);
        assert_eq!(control.verdict(), relca_core::Verdict::Satisfied);
        // Change only the storage-kind tag of an otherwise well-formed kernel.
        // Row arity, declared writes, value types and owner remain valid.
        m.state_variables[0].kind = actor_kind;
        reject_ir(&m);
    }
}

const CASE_GIVEN: &str = "model M version \"0.2\" {type R=enum{ok};observed case{flag:bool=true};kernel K given [flag] {true -> {1:ok};false -> {1:ok}} actor A{capacity 1;capability run()->R{duration fixed 1min;outcomes kernel K}} process P{a=A.run();end done} cut C{include[A]} bounds{cases 1;live_tasks 1;clock 3min} verify safety no_resource_overcommit}";
const CASE_WITH: &str = "model M version \"0.2\" {type R=enum{ok};observed case{flag:bool=false};kernel K {* -> {1:ok with flag=true}} actor A{capacity 1;capability run()->R{duration fixed 1min;writes flag;outcomes kernel K}} process P{a=A.run();end done} cut C{include[A]} bounds{cases 1;live_tasks 1;clock 3min} verify safety no_resource_overcommit}";

#[test]
fn actor_kernel_ir_negative_controls_have_valid_legacy_rows_and_types() {
    for s in [CASE_GIVEN, CASE_WITH] {
        let m = h::model(s);
        let e = engine::explore(&m);
        let r = reference::explore(&m);
        assert_eq!(r.stop, reference::StopReason::Complete);
        assert_eq!(r.verdict(), relca_core::Verdict::Satisfied);
        assert_eq!(e.properties, r.outcome.properties);
    }
}

#[test]
fn f24_simulation_never_randomizes_actor_state_or_scheduler() {
    for name in ["aobs-simulation", "aobs-simulation.broken-nondeterminism"] {
        let m = h::model(&h::source(name));
        let e = engine::simulate(&m);
        let r = reference::simulate(&m);
        let bad = name.contains("broken");
        for out in [&e, &r.outcome] {
            assert_eq!(out.completed_replications, if bad { 0 } else { 1 });
            assert_eq!(out.diagnostics.iter().any(|d| d.code == "U-SEM-0"), bad);
            if !bad {
                assert_eq!(out.replications.len(), 1);
                assert!(
                    out.replications[0].draws.is_empty(),
                    "fixed Actor state is not random"
                );
                assert_eq!(out.replications[0].cases.len(), 2);
            }
        }
        assert_eq!(e.replications, r.outcome.replications);
        let v = h::check(name);
        assert_eq!(
            v["verdict"],
            if bad { "unsupported" } else { "inconclusive" }
        );
        assert_eq!(
            v["simulation"]["completed_replications"],
            if bad { 0 } else { 1 }
        );
    }
}

#[test]
fn actor_table_assumptions_labels_and_responsibility_source_are_not_semantic_shortcuts() {
    let v = h::check("aobs-counter");
    assert_eq!(
        v["actors"][0]["state"],
        json!({"value":["Desk.handled"],"status":"declared"})
    );
    assert!(
        !v["assumptions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["field"] == "Desk.instance")
    );
    let v = h::check("aobs-shared-order");
    let a: Vec<_> = v["assumptions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["field"] == "Ledger.instance")
        .collect();
    assert_eq!(a.len(), 1);
    assert_eq!(
        a[0],
        &json!({"field":"Ledger.instance","value":"shared(default)","affects":"局所の観測値は全案件で1組を共有し、書込みは所有Actorのstart/finishの遷移で原子的に起きる。案件の間の順序は全部展開する"})
    );
    let s = h::source("aobs-labels");
    for grain in ["document", "ledger", "person"] {
        for source in ["", "source record;"] {
            let t = h::change(
                &s,
                "actor Ledger {",
                &format!("actor Ledger {{ grain {grain}; {source}"),
            );
            let m = h::model(&t);
            let e = engine::explore(&m);
            let r = reference::explore(&m);
            assert_eq!(r.stop, reference::StopReason::Complete);
            h::hand_outcome("aobs-labels", &m, &e);
            h::hand_outcome("aobs-labels", &m, &r.outcome);
        }
    }
    let v = h::check("aobs-per-case");
    assert!(
        !v["assumptions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["field"] == "Doc.instance" && r["value"] == "shared(default)")
    );
    for name in ["aobs-responsibility", "aobs-responsibility.broken-no-read"] {
        let v = h::check(name);
        let rows = v["responsibilities"].as_array().unwrap();
        assert_eq!(rows.len(), 1);
        let o = &rows[0]["observation"];
        assert_eq!(o["required_sources"], json!(["Doc"]));
        assert_eq!(
            o["direct"],
            if name.contains("broken") {
                json!([])
            } else {
                json!(["Doc"])
            }
        );
        assert_eq!(
            o["missing_sources"],
            if name.contains("broken") {
                json!(["Doc"])
            } else {
                json!([])
            }
        );
        assert_eq!(
            v["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["code"] == "W-RESP-1"),
            name.contains("broken")
        );
    }
}

#[test]
fn explain_range_and_same_tick_paths_verify_actor_observed_identity() {
    for name in [
        "aobs-counter.broken-overflow",
        "aobs-postwrite.broken-same-tick",
    ] {
        let src = h::source(name);
        assert!(!src.contains("configuration"));
        let v = serde_json::to_value(explain::explain(&src)).unwrap();
        assert_eq!(v["verdict"], "violated");
        assert_eq!(v["explanation"]["status"], "complete");
        let cs = v["explanation"]["counterexamples"].as_array().unwrap();
        assert!(!cs.is_empty());
        for c in cs {
            assert_eq!(c["replay"]["status"], "verified");
            let mut seen = false;
            for stage in c["steps"].as_array().unwrap() {
                assert!(
                    !stage["values"]["latent"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|v| v["name"].as_str().unwrap().starts_with("Desk."))
                );
                for value in stage["values"]["observed"].as_array().unwrap() {
                    let name = value["name"].as_str().unwrap();
                    if !name.starts_with("Desk.") {
                        continue;
                    }
                    seen = true;
                    assert_eq!(value["visibility"], "observed");
                    let a = value["location"]["span"]["start"].as_u64().unwrap() as usize;
                    let b = value["location"]["span"]["end"].as_u64().unwrap() as usize;
                    assert!(src[a..b].contains(name.strip_prefix("Desk.").unwrap()));
                    assert_eq!(
                        value["location"]["line"].as_u64().unwrap() as usize,
                        src[..a].bytes().filter(|b| *b == b'\n').count() + 1
                    );
                    assert_ne!(
                        value["value"], "3",
                        "attempted overflow never becomes a stored state"
                    );
                }
            }
            assert!(seen, "Actor.x observation required in trace");
        }
    }
}

fn atomic_source(phase: &str, effects: &str, arrivals: &str) -> String {
    let writes = if phase == "finish" {
        "writes [n,flag];"
    } else {
        ""
    };
    format!(
        "model M version \"0.2\" {{ type R=enum{{ok}}; actor A {{capacity 1;state n:0..0=0;state flag:bool=false;capability run()->R {{duration fixed 1min;result nondet{{ok}};{writes}{phase} {{{effects}}}}}}} process P{{a=A.run();end done}} cut C{{include[A]}} bounds{{cases 1;live_tasks 1;clock 4min}} {arrivals} verify reachable end.done }}"
    )
}

#[test]
fn atomic_effect_legacy_controls_are_well_typed_in_both_phases_and_routes() {
    for phase in ["start", "finish"] {
        for effects in ["flag = true; n += 1", "n += 1; flag = true"] {
            for arrivals in ["", "arrivals fixed [0min];"] {
                let s = atomic_source(phase, effects, arrivals);
                let control = h::change(
                    &s,
                    "actor A {capacity 1;state n:0..0=0;state flag:bool=false;",
                    "observed case {n:0..0=0;flag:bool=false}; actor A {capacity 1;",
                );
                let m = h::model(&control);
                let e = engine::explore(&m);
                let r = reference::explore(&m);
                assert_eq!(r.stop, reference::StopReason::Complete);
                for out in [&e, &r.outcome] {
                    assert_eq!(out.state_range_violations.len(), 1);
                    assert_eq!(out.state_range_violations[0].attempted, "1");
                    assert!(!out.truncated);
                }
            }
        }
    }
}

#[test]
fn overflow_is_atomic_in_both_effect_orders_phases_routes_and_interpreters() {
    for phase in ["start", "finish"] {
        for effects in ["flag = true; n += 1", "n += 1; flag = true"] {
            for arrivals in ["", "arrivals fixed [0min];"] {
                let s = atomic_source(phase, effects, arrivals);
                let m = h::model(&s);
                let e = engine::explore(&m);
                let r = reference::explore(&m);
                assert_eq!(r.stop, reference::StopReason::Complete);
                for out in [&e, &r.outcome] {
                    assert!(!out.truncated);
                    assert_eq!(out.state_range_violations.len(), 1);
                    assert_eq!(
                        out.state_range_violations[0].time,
                        if phase == "start" { 0 } else { 1 }
                    );
                    assert_eq!(out.state_range_violations[0].attempted, "1");
                    assert!(out.terminals.is_empty());
                }
                let v = serde_json::to_value(explain::explain(&s)).unwrap();
                assert_eq!(v["explanation"]["status"], "complete");
                for c in v["explanation"]["counterexamples"].as_array().unwrap() {
                    assert_eq!(c["replay"]["status"], "verified");
                    let last = c["steps"].as_array().unwrap().last().unwrap();
                    let values = last["values"]["observed"].as_array().unwrap();
                    assert!(
                        values
                            .iter()
                            .any(|v| v["name"] == "A.flag" && v["value"] == "false"),
                        "no partial effect: {last}"
                    );
                    assert!(
                        values
                            .iter()
                            .any(|v| v["name"] == "A.n" && v["value"] == "0")
                    );
                }
            }
        }
    }
}

#[test]
fn integer_lower_upper_overflow_and_initial_type_boundaries_remain_distinct() {
    let s = h::source("aobs-counter");
    for (declaration, effect, attempt) in [
        ("0..1 = 0", "handled -= 1", "-1"),
        ("0..1 = 0", "handled = 2", "2"),
        (
            "0..18446744073709551615 = 18446744073709551615",
            "handled += 1",
            "18446744073709551616",
        ),
    ] {
        let t = h::change(&s, "0..2 = 0", declaration);
        // The overflow row keeps the fixture's own effect; only its declaration changes.
        let t = if effect == "handled += 1" {
            t
        } else {
            h::change(&t, "handled += 1", effect)
        };
        let m = h::model(&t);
        let e = engine::explore(&m);
        let r = reference::explore(&m);
        assert_eq!(r.stop, reference::StopReason::Complete);
        for out in [&e, &r.outcome] {
            assert_eq!(out.state_range_violations.len(), 1);
            assert_eq!(out.state_range_violations[0].attempted, attempt);
            assert_eq!(out.state_range_violations[0].variable, "Desk.handled");
            assert!(!out.truncated);
        }
    }
    h::reject(&h::change(&s, "0..2 = 0", "0..2 = -1"), "E-TYPE-0");
}

#[test]
fn read_only_capacity_two_and_single_case_serial_writes_do_not_trigger_shared_guard() {
    let s = h::source("aobs-capacity");
    let single = h::change(&s, "cases 2", "cases 1");
    let single = h::change(&single, "[0min,0min]", "[0min]");
    let single = h::change(&single, "capacity 1", "capacity 2");
    let m = h::model(&single);
    let r = reference::explore(&m);
    assert_eq!(r.stop, reference::StopReason::Complete);
    assert!(!engine::explore(&m).properties.is_empty());
    let s = h::source("aobs-per-case");
    let s = h::change(&s, "instance per_case;", "capacity 2;");
    let m = h::model(&s);
    let r = reference::explore(&m);
    assert_eq!(r.stop, reference::StopReason::Complete);
    h::hand_outcome("aobs-per-case", &m, &r.outcome);
    let s = h::source("aobs-counter");
    let s = h::change(
        &s,
        "a = Desk.take(); b = Desk.take();",
        "fork f { a = Desk.take(); b = Desk.take() }; join all f;",
    );
    h::reject(&s, "U-SEM-0");
}

#[test]
fn fixed_actor_state_does_not_add_a_seed_or_control_state() {
    for arrivals in ["", "arrivals fixed [0min];"] {
        let base = format!(
            "model M version \"0.2\" {{ actor A {{}} process P{{end done}} cut C{{include[A]}} bounds{{cases 1;live_tasks 1;clock 2min}} {arrivals} verify reachable end.done }}"
        );
        let state = h::change(&base, "actor A {}", "actor A {state x:bool=true}");
        let a = h::model(&base);
        let b = h::model(&state);
        let threshold = |m: &ir::Model| {
            (0..50)
                .find(|n| !engine::explore_with_budget(m, *n).truncated)
                .unwrap()
        };
        assert_eq!(
            threshold(&a),
            threshold(&b),
            "no extra fixed-state initialization successor"
        );
        if arrivals.is_empty() {
            assert_eq!(threshold(&b), 1, "legacy end-only holds just its root");
        }
        let r = reference::explore(&a);
        let t = reference::explore(&b);
        assert_eq!(r.stop, reference::StopReason::Complete);
        assert_eq!(t.stop, reference::StopReason::Complete);
        assert_eq!(r.statistics.frames, t.statistics.frames);
    }
}

#[test]
fn optional_initial_and_result_snapshots_are_observed_not_latent() {
    let s = h::source("aobs-optional");
    for (from, to, kind, answer, flag) in [
        (
            "A.run.start -> A.answer == none",
            "A.run.start -> A.answer == ok",
            "start",
            "none",
            "false",
        ),
        (
            "A.run.finish -> A.answer == ok && A.flag == true",
            "A.run.finish -> A.answer == ok && A.flag == false",
            "finish",
            "ok",
            "true",
        ),
    ] {
        let t = h::change(&s, from, to);
        let v = serde_json::to_value(explain::explain(&t)).unwrap();
        assert_eq!(v["explanation"]["status"], "complete");
        let cs = v["explanation"]["counterexamples"].as_array().unwrap();
        assert_eq!(cs.len(), 1);
        assert_eq!(cs[0]["replay"]["status"], "verified");
        let last = cs[0]["steps"].as_array().unwrap().last().unwrap();
        assert_eq!(last["kind"], kind);
        let values = last["values"]["observed"].as_array().unwrap();
        assert_eq!(values.len(), 2);
        assert!(
            values
                .iter()
                .any(|v| v["name"] == "A.answer" && v["value"] == answer)
        );
        assert!(
            values
                .iter()
                .any(|v| v["name"] == "A.flag" && v["value"] == flag)
        );
        assert_eq!(last["values"]["latent"], json!([]));
    }
}

#[test]
fn same_short_names_keep_distinct_actor_declaration_identities_in_replay() {
    let s = h::source("aobs-owner");
    let s = h::change(&s, "process P", "actor C { latent x: R = no }; process P");
    let s = h::change(&s, "include [A,B]", "include [A,B,C]");
    let s = h::change(
        &s,
        "verify reachable",
        "contract K { invariant A.run.finish -> B.x == no }; verify reachable",
    );
    let v = serde_json::to_value(explain::explain(&s)).unwrap();
    assert_eq!(v["explanation"]["status"], "complete");
    let cs = v["explanation"]["counterexamples"].as_array().unwrap();
    assert_eq!(cs.len(), 1);
    assert_eq!(cs[0]["replay"]["status"], "verified");
    for stage in cs[0]["steps"].as_array().unwrap() {
        let obs = stage["values"]["observed"].as_array().unwrap();
        assert_eq!(obs.len(), 2);
        assert_eq!(
            obs.iter()
                .map(|v| v["name"].as_str().unwrap())
                .collect::<std::collections::BTreeSet<_>>(),
            std::collections::BTreeSet::from(["A.x", "B.x"])
        );
        assert_ne!(obs[0]["location"]["span"], obs[1]["location"]["span"]);
        let lat = stage["values"]["latent"].as_array().unwrap();
        assert_eq!(lat.len(), 1);
        assert_eq!(
            lat[0]["name"], "x",
            "old latent short-name schema is unchanged"
        );
    }
}

#[test]
fn authority_false_observation_is_a_contract_violation_not_a_policy_type_error() {
    let t = h::change(
        &h::source("aobs-scope"),
        "allow: bool = true",
        "allow: bool = false",
    );
    let m = h::model(&t);
    let e = engine::explore(&m);
    let r = reference::explore(&m);
    assert_eq!(r.stop, reference::StopReason::Complete);
    assert_eq!(r.verdict(), relca_core::Verdict::Violated);
    assert_eq!(e.properties, r.outcome.properties);
    assert_eq!(report::check(&t).verdict, relca_core::Verdict::Violated);
}
