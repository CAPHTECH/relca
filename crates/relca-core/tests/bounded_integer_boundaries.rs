//! Supplemental ADR 0029 obligations: each number is a hand-derived oracle.
#[path = "support/integer_acceptance.rs"]
mod hand;
use relca_core::{engine, ir, lower, reference, report, syntax};
use serde_json::{Value, json};
fn check(s: &str) -> Value {
    serde_json::to_value(report::check(s)).unwrap()
}
fn model(s: &str) -> ir::Model {
    lower::lower(&syntax::parse(s).unwrap_or_else(|e| panic!("{e:?}\n{s}")))
        .unwrap_or_else(|e| panic!("{e:?}\n{s}"))
}
fn program(state: &str, cap: &str, flow: &str, extra: &str) -> String {
    let result = if cap.contains("outcomes kernel") {
        ""
    } else {
        "result nondet { ok };"
    };
    format!(
        r#"model Boundary version "0.2" {{ type T = enum {{ ok, no }}; {state}; actor A {{ capacity 1; capability run() -> T {{ duration fixed 1min; {result} {cap} }} }}; process P {{ {flow} }}; bounds {{ cases 1; live_tasks 4; clock 10min }}; verify safety no_resource_overcommit; {extra} }}"#
    )
}
fn both(s: &str, id: &str, verdict: &str) {
    let m = model(s);
    let public = check(s);
    let p = hand::property(&public, id);
    assert_eq!(p["verdict"], verdict, "{s}\n{public}");
    let i = public["properties"]
        .as_array()
        .unwrap()
        .iter()
        .position(|p| p["id"] == id)
        .unwrap();
    let a = engine::explore(&m);
    let r = reference::explore(&m);
    assert_eq!(r.stop, reference::StopReason::Complete);
    for (label, o) in [("engine", a), ("reference", r.outcome)] {
        assert_eq!(
            serde_json::to_value(o.properties[i].verdict).unwrap(),
            verdict,
            "{label}: {s}"
        );
    }
}
macro_rules! reject { ($($name:ident: $state:literal, $cap:literal, $flow:literal, $extra:literal => $code:literal;)+) => {$(#[test] fn $name() { hand::rejection(&program($state,$cap,$flow,$extra),$code); })+}; }
reject! {
 negative_default: "observed case { n: 0..2 = -1 }", "", "end done", "" => "E-TYPE-0";
 enum_compared_to_integer: "observed case { n: T = ok }", "", "if n == 1 {} else {}; end done", "" => "E-TYPE-0";
 add_to_enum: "observed case { n: T = ok }", "writes n; finish { n += 1 }", "end done", "" => "E-TYPE-0";
 subtract_from_bool: "observed case { n: bool = true }", "start { n -= 1 }", "end done", "" => "E-TYPE-0";
 implicit_enum_result_into_integer: "observed case { n: 0..2 = 0 }", "writes n", "a = A.run(); end done", "" => "E-TYPE-0";
 effect_outside_writes: "observed case { n: 0..2 = 0 }", "finish { n += 1 }", "end done", "" => "E-NAME-0";
 effect_unknown_target: "observed case { n: 0..2 = 0 }", "start { missing += 1 }", "end done", "" => "E-NAME-0";
 latent_numeric_write: "latent case { n: 0..2 = 0 }", "writes n; finish { n += 1 }", "end done", "" => "U-SYN-0";
 duplicate_numeric_target: "observed case { n: 0..2 = 0 }", "writes n; finish { n += 0; n += 0 }", "end done", "" => "U-SEM-0";
 missing_observed_default: "observed case { n: 0..2 }", "", "end done", "" => "U-SEM-0";
 optional_integer: "observed case { n: optional<0..2> }", "", "end done", "" => "U-SYN-0";
 actor_integer: "actor D { latent h: 0..2 = 0 }", "", "end done", "" => "U-SYN-0";
 integer_parameter: "actor D { capacity 1; capability c(k: 0..2) -> T { duration fixed 1min; result nondet { ok } } }", "", "end done", "" => "U-SYN-0";
 integer_return: "actor D { capacity 1; capability c() -> 0..2 { duration fixed 1min; result nondet { 0 } } }", "", "end done", "" => "U-SYN-0";
 nonzero_lower: "observed case { n: 1..2 = 1 }", "", "end done", "" => "U-SYN-0";
 negative_lower: "observed case { n: -1..2 = 1 }", "", "end done", "" => "U-SYN-0";
 negative_upper: "observed case { n: 0..-1 = 0 }", "", "end done", "" => "U-SYN-0";
 expression_upper: "observed case { n: 0..(1+1) = 0 }", "", "end done", "" => "U-SYN-0";
 too_large_upper: "observed case { n: 0..18446744073709551616 = 0 }", "", "end done", "" => "U-SYN-0";
 too_large_operand: "observed case { n: 0..2 = 0 }", "start { n += 18446744073709551616 }", "end done", "" => "U-SYN-0";
 missing_upper: "observed case { n: 0.. = 0 }", "", "end done", "" => "E-SYN-0";
 missing_dot: "observed case { n: 0.2 = 0 }", "", "end done", "" => "E-SYN-0";
 variable_rhs: "observed case { n: 0..2 = 0; m: 0..2 = 1 }", "writes n; finish { n = m }", "end done", "" => "U-SYN-0";
 multiplication: "observed case { n: 0..2 = 0 }", "writes n; finish { n *= 2 }", "end done", "" => "U-SYN-0";
 division: "observed case { n: 0..2 = 0 }", "writes n; finish { n /= 2 }", "end done", "" => "U-SYN-0";
 general_expression: "observed case { n: 0..2 = 0 }", "writes n; finish { n = 1 + 1 }", "end done", "" => "U-SYN-0";
 split_add: "observed case { n: 0..2 = 0 }", "start { n + = 1 }", "end done", "" => "E-SYN-0";
 split_le: "observed case { n: 0..2 = 0 }", "", "if n < = 1 {} else {}; end done", "" => "E-SYN-0";
 split_ge: "observed case { n: 0..2 = 0 }", "", "if n > = 1 {} else {}; end done", "" => "E-SYN-0";
 policy_latent_dead_suffix: "latent case { n: 0..2 = 1 }", "", "end done; if n>=1 {} else {}", "" => "E-POL-LATENT";
 policy_latent_and_false: "latent case { n: 0..2 = 1 }; observed case { f: bool = false }", "", "if f == true && n<2 {} else {}; end done", "" => "E-POL-LATENT";
 policy_latent_or_true: "latent case { n: 0..2 = 1 }; observed case { f: bool = true }", "", "if f == true || n!=2 {} else {}; end done", "" => "E-POL-LATENT";
 requires_latent: "latent case { n: 0..2 = 1 }", "requires n>=1 else no", "a = A.run(); end done", "" => "E-POL-LATENT";
 distribution_outside: "latent case { h: 0..1 ~ { 0:0.5, 2:0.5 } }", "", "end done", "" => "E-TYPE-0";
 distribution_missing: "latent case { h: 0..1 ~ { 0:1 } }", "", "end done", "" => "E-PROB-0";
 distribution_duplicate_canonical: "latent case { h: 0..1 ~ { 0:0.5, 00:0.5 } }", "", "end done", "" => "E-PROB-0";
 distribution_duplicate_negative_zero: "latent case { h: 0..1 ~ { 0:0.5, -0:0.5 } }", "", "end done", "" => "E-PROB-0";
 distribution_bad_mass: "latent case { h: 0..1 ~ { 0:0.5, 1:0.49 } }", "", "end done", "" => "E-PROB-0";
 distribution_with_default: "latent case { h: 0..1 = 0 ~ { 0:1, 1:0 } }", "", "end done", "" => "U-SEM-0";
 observed_distribution: "observed case { h: 0..1 ~ { 0:1, 1:0 } }", "", "end done", "" => "U-SEM-0";
 numeric_kernel_result: "kernel K { * -> { 1: 0 } }", "outcomes kernel K", "a = A.run(); end done", "" => "U-SEM-0";
 integer_kernel_write: "observed case { n: 0..1 = 0 }; kernel K { * -> { 1: ok with n = 1 } }", "outcomes kernel K; writes n", "a = A.run(); end done", "" => "U-SEM-0";
 integer_kernel_zero_mass_write: "observed case { n: 0..1 = 0 }; kernel K { * -> { 1: ok; 0: no with n = 1 } }", "", "end done", "" => "U-SEM-0";
}
#[test]
fn all_six_comparisons_in_if_requires_invariant_and_authority_scope_have_hand_boundaries() {
    for (op, rhs, values) in [
        ("==", 2, [(2, true), (1, false)]),
        ("!=", 2, [(1, true), (2, false)]),
        ("<", 2, [(1, true), (2, false)]),
        ("<=", 2, [(2, true), (3, false)]),
        (">", 1, [(2, true), (1, false)]),
        (">=", 2, [(2, true), (1, false)]),
    ] {
        for (n, truth) in values {
            let state = format!("observed case {{ n: 0..12 = {n} }}");
            let cond = format!("n{op}{rhs}");
            both(
                &program(
                    &state,
                    "",
                    &format!("if {cond} {{ end done }} else {{ end wrong }}"),
                    "verify reachable end.done",
                ),
                "reachable end.done",
                if truth { "satisfied" } else { "violated" },
            );
            let s = program(
                &state,
                &format!("requires {cond} else no"),
                "a = A.run(); if a == ok { end done } else { end refused }",
                "verify reachable end.done",
            );
            both(
                &s,
                "reachable end.done",
                if truth { "satisfied" } else { "violated" },
            );
            both(
                &program(
                    &state,
                    "",
                    "a = A.run(); end done",
                    &format!("contract C {{ invariant A.run.finish -> {cond} }}"),
                ),
                "contract C #1 invariant",
                if truth { "satisfied" } else { "violated" },
            );
            let scope = authority(&state, &cond);
            both(
                &scope,
                "contract C #1 authority_basis",
                if truth { "satisfied" } else { "violated" },
            );
        }
    }
}
fn authority(state: &str, cond: &str) -> String {
    program(&format!("people [r,q]; {state}"),"","a = A.run(); end done",&format!("permit q to A.run; authority G {{ basis \"整数の範囲\"; grants q to A.run; issued_by r; scope {cond} }}; contract C {{ authority_basis A.run G }}")).replace("capacity 1", "executors [q]")
}
#[test]
fn latent_scope_and_unselected_process_are_rejected_without_short_circuit() {
    hand::rejection(
        &authority("latent case { n: 0..2 = 1 }", "n>=1"),
        "E-POL-LATENT",
    );
    let s = program("latent case { n: 0..2 = 1 }","","end done","").replace("bounds {", "process Other { if n>=1 {} else {}; end done }; cut C { include [A] }; configuration Chosen { cut C; process P }; bounds {");
    let v = serde_json::to_value(report::check_configuration(&s, "Chosen")).unwrap();
    assert_eq!(v["verdict"], "invalid");
    assert!(
        v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "E-POL-LATENT")
    );
}
#[test]
fn analyst_reads_latent_and_zero_mass_support_is_not_executed() {
    let s = program(
        "latent case { n: 0..2 = 1 }",
        "",
        "a = A.run(); end done",
        "contract C { invariant A.run.finish -> n>=1 }; verify probability reached n == 1 >= 1",
    );
    both(&s, "contract C #1 invariant", "satisfied");
    both(&s, "probability reached #1", "satisfied");
    let s = program(
        "latent case { h: 0..1 ~ { 0:1, 1:0 } }",
        "",
        "end done",
        "verify probability reached h == 1 <= 0",
    );
    both(&s, "probability reached #1", "satisfied");
    assert_eq!(
        hand::property(&check(&s), "probability reached #1")["probability"],
        json!({"min":"0","max":"0"})
    );
}
#[test]
fn signed_exact_arithmetic_huge_fixed_domain_and_singleton() {
    for (initial, op, operand, attempted, upper, verdict) in [
        (
            "18446744073709551615",
            "+=",
            "1",
            "18446744073709551616",
            "18446744073709551615",
            "violated",
        ),
        (
            "18446744073709551615",
            "+=",
            "18446744073709551615",
            "36893488147419103230",
            "18446744073709551615",
            "violated",
        ),
        ("0", "-=", "-1", "1", "2", "satisfied"),
        ("0", "+=", "-1", "-1", "2", "violated"),
        ("0", "=", "0", "0", "0", "satisfied"),
        ("0", "+=", "1", "1", "0", "violated"),
    ] {
        let s = program(
            &format!("observed case {{ n: 0..{upper} = {initial} }}"),
            &format!("writes n; finish {{ n {op} {operand} }}"),
            "a = A.run(); end done",
            "",
        );
        both(&s, "state_in_range", verdict);
        if verdict == "violated" {
            let v = check(&s);
            assert_eq!(v["state_range_violations"][0]["attempted"], attempted);
        }
    }
}
#[test]
fn numeric_not_lexicographic_comparison_and_out_of_domain_comparison_constants() {
    for (cond, want) in [
        ("n<2", "violated"),
        ("n>2", "satisfied"),
        ("n<0", "violated"),
        ("n<=13", "satisfied"),
    ] {
        both(
            &program(
                "observed case { n: 0..12 = 10 }",
                "",
                &format!("if {cond} {{ end done }} else {{ end wrong }}"),
                "verify reachable end.done",
            ),
            "reachable end.done",
            want,
        );
    }
}
#[test]
fn grouping_optional_adjacency_and_probability_threshold_operators_are_not_confused() {
    let s = program(
        "observed case { n: 0 .. 3 = +2; empty: optional<T>=none }",
        "",
        "if (n>=2 &&\n n<=2) || (n!=2 && n<0) { end done } else { end wrong }",
        "verify reachable end.done; verify probability reached n <= 2 >= 0.9; verify probability reached n == 2 <= 1",
    );
    both(&s, "reachable end.done", "satisfied");
    both(&s, "probability reached #1", "satisfied");
    both(&s, "probability reached #2", "satisfied");
}
#[test]
fn implicit_range_property_without_explicit_verify_and_legacy_empty_verify_rejection() {
    let s = "model M version \"0.2\" { observed case { n: 0..0 = 0 }; process P { end done }; bounds { cases 1; live_tasks 1; clock 1min } }";
    let v = check(s);
    assert_eq!(v["verdict"], "satisfied");
    assert_eq!(
        v["properties"],
        json!([{"id":"state_in_range","verdict":"satisfied"}])
    );
    assert_ne!(
        check(&s.replace("observed case { n: 0..0 = 0 };", ""))["verdict"],
        "satisfied"
    );
}
#[test]
fn initialization_huge_nondet_is_budgeted_before_materialization_in_both_routes() {
    for arrivals in [false, true] {
        let mut s = program(
            "latent case { h: 0..18446744073709551615 }",
            "",
            "end done",
            "",
        );
        if arrivals {
            s = s
                .replace("bounds {", "arrivals fixed [0min,0min]; bounds {")
                .replace("cases 1", "cases 2");
        }
        let m = model(&s);
        for budget in [0, 1, 2, 3] {
            let a = engine::explore_with_budget(&m, budget);
            assert!(a.truncated);
            assert!(a.terminals.is_empty());
            let r = reference::explore_with_limits(
                &m,
                reference::Limits {
                    frames: budget,
                    ..Default::default()
                },
            );
            assert_eq!(r.stop, reference::StopReason::EnumerationLimit);
            assert!(r.outcome.terminals.is_empty());
        }
    }
}
#[test]
fn invalid_direct_ir_integer_initial_transplanted_into_enum_fails_closed_in_both_runtimes() {
    let int = model(&program(
        "observed case { n: 0..2 = 1 }",
        "",
        "end done",
        "",
    ));
    let mut en = model(&program("observed case { n: T = ok }", "", "end done", ""));
    en.state_variables[0].initial = int.state_variables[0].initial;
    for o in [engine::explore(&en), reference::explore(&en).outcome] {
        assert!(
            o.diagnostics
                .iter()
                .any(|d| d.severity == relca_core::diag::Severity::Error)
        );
        assert!(o.terminals.is_empty());
    }
}
#[test]
fn classification_is_local_and_declared_enum_names_keep_precedence() {
    {
        let (base, fault, token) = ("int-class-int", "unbounded", "int");
        let s = hand::source(&format!("{base}.broken-{fault}"));
        let changed = s.replace(
            "type T = enum",
            &format!("type {token} = enum {{ ok, no }}; type T = enum"),
        );
        assert_ne!(s, changed);
        assert_eq!(
            check(&changed)["verdict"],
            "satisfied",
            "declared {token}: {changed}"
        );
    }
    let reads = hand::source("int-class-reads.broken-observed");
    for target in ["Unknown", "A"] {
        hand::rejection(
            &reads.replace("reads [flag]", &format!("reads [{target}]")),
            "E-NAME-0",
        );
    }
    let shadow = reads.replace("actor S", "actor flag");
    assert_eq!(check(&shadow)["verdict"], "satisfied");
    let arity = hand::source("int-class-param.broken-caseid").replace(
        "process P { end done }",
        "process P { a = A.unused(); end done }",
    );
    let v = check(&arity);
    assert_eq!(v["verdict"], "invalid");
    for code in ["U-SYN-0", "E-TYPE-0"] {
        assert!(
            v["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["code"] == code),
            "{v}"
        );
    }
    for name in ["int", "CaseId"] {
        hand::rejection(
            &program(
                &format!("observed case {{ n: {name} }}"),
                "",
                "end done",
                "",
            ),
            "U-SYN-0",
        );
    }
    hand::rejection(
        &hand::source("int-class-int").replace("n: T", "n: Unknown"),
        "E-NAME-0",
    );
}
#[test]
fn fork_conflict_does_not_depend_on_operand_zero_capacity_one_or_zero_duration() {
    let s = hand::source("int-fork.broken-same-target");
    for s in [
        s.clone(),
        s.replace("+= 1", "+= 0"),
        s.replace("fixed 1min", "fixed 0min"),
        s.replace("b = B.inc()", "b = A.inc()"),
    ] {
        hand::rejection(&s, "U-SEM-0");
    }
}

#[test]
fn mixed_success_range_and_clock_mass_is_not_renormalized_and_known_hit_survives() {
    let s = r#"model Mixed version "0.2" {
      type T = enum { ok, bad, slow }
      observed case { n: 0..1 = 1; seen: bool = false; route: optional<T> }
      kernel K { * -> { 0.5: ok; 0.25: bad; 0.25: slow } }
      actor Choice { capacity 1; capability run() -> T { duration fixed 1min; outcomes kernel K; writes [route,seen]; finish { seen = true } } }
      actor A { capacity 1; capability run() -> T { duration fixed 1min; result nondet { ok }; writes n; finish { n += 1 } } }
      process P { a = Choice.run(); if route == ok { end done } else { if route == bad { b = A.run(); end range_end } else { wait 20min; end late } } }
      bounds { cases 1; live_tasks 2; clock 3min }
      verify probability reached end.done >= 0.5
      verify probability reached seen == true >= 1
    }"#;
    both(s, "state_in_range", "violated");
    let v = check(s);
    assert_eq!(v["bounds"]["truncated"], true);
    assert_eq!(v["bounds"]["range_violation_stops"], true);
    assert_eq!(
        hand::property(&v, "probability reached #1")["probability"],
        json!({"min":"1/2","max":"1"})
    );
    assert_eq!(
        hand::property(&v, "probability reached #2")["probability"],
        json!({"min":"1","max":"1"})
    );
    assert!(
        v["terminals"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["end"] == "done")
    );
    assert!(
        !v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"].as_str().unwrap().starts_with("W-UNREACH-"))
    );
    let safe = s.replace("finish { n += 1 }", "finish { n += 0 }");
    both(&safe, "state_in_range", "inconclusive");
    assert!(
        check(&safe)["bounds"]
            .get("range_violation_stops")
            .is_none()
    );
}

#[test]
fn integer_guards_flow_through_agency_and_responsibility_visitors_without_becoming_result_evidence()
{
    let s = hand::source("state-agency.broken-state-guard")
        .replace("decision: optional<Verdict>", "n: 0..2 = 1")
        .replace("; writes decision", "")
        .replace("decision == pass", "n>=1");
    let v = check(&s);
    assert_eq!(v["verdict"], "satisfied");
    assert!(
        v["agency"]["unchecked"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w == "state_guards")
    );
    assert_eq!(v["agency"]["refusal"], json!([]));
    let s = program(
        "people [p]; observed case { n: 0..2 = 1 }; responsible p for (n>=1)",
        "",
        "end done",
        "",
    );
    let v = check(&s);
    assert_eq!(v["verdict"], "satisfied");
    let row = &v["responsibilities"][0];
    assert_eq!(row["target"]["kind"], "state_condition");
    assert_eq!(row["observation"]["status"], "not_derived");
    assert_eq!(row["refusal"]["status"], "not_derived");
    assert_eq!(row["time_frame"]["status"], "unchecked");
}

#[test]
fn unused_integer_kernel_given_is_rejected() {
    let s = hand::source("int-kernel-given.broken-integer").replace("a = A.inc(); ", "");
    hand::rejection(&s, "U-SEM-0");
}

#[test]
fn legacy_enum_comparisons_and_unbounded_return_name_errors_are_not_migrated() {
    for condition in ["n!=ok", "n<ok", "n<=ok", "n>ok", "n>=ok"] {
        hand::rejection(
            &program(
                "observed case { n: T = ok }",
                "",
                &format!("if {condition} {{}} else {{}}; end done"),
                "",
            ),
            "E-SYN-0",
        );
    }
    hand::rejection(
        &program("", "", "end done", "").replace("run() -> T", "run() -> int"),
        "E-NAME-0",
    );
}

#[test]
fn integer_values_distinguish_zero_one_and_enum_zero_in_eq_and_hash() {
    let value = |declaration: &str| {
        let m = model(&program(declaration, "", "end done", ""));
        match m.state_variables[0].initial {
            ir::InitialValue::Fixed(v) => v,
            _ => panic!("fixed declaration must remain fixed"),
        }
    };
    let zero = value("observed case { n: 0..2 = 0 }");
    let one = value("observed case { n: 0..2 = 1 }");
    let enum_zero = value("observed case { n: T = ok }");
    assert_ne!(zero, one);
    assert_ne!(zero, enum_zero);
    assert_eq!(std::collections::HashSet::from([zero, one]).len(), 2);
    assert_eq!(
        std::collections::HashSet::from([zero, one, enum_zero]).len(),
        3
    );
}

#[test]
fn start_range_violation_does_not_publish_a_start_or_partially_apply_the_bundle() {
    let s = program(
        "observed case { n: 0..1 = 1; flag: bool = false }",
        "start { flag = true; n += 1 }",
        "a = A.run(); end done",
        "verify reachable end.done",
    );
    both(&s, "state_in_range", "violated");
    let v = check(&s);
    assert_eq!(v["state_range_violations"][0]["phase"], "start");
    assert_eq!(v["state_range_violations"][0]["time"], 0);
    assert_eq!(v["terminals"], json!([]));
    let ex = serde_json::to_value(relca_core::explain::explain(&s)).unwrap();
    let c = ex["explanation"]["counterexamples"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "state_in_range")
        .unwrap();
    assert_eq!(c["replay"]["status"], "verified");
    assert_eq!(c["transition_count"], 2);
    let stages = c["steps"].as_array().unwrap();
    assert_eq!(
        stages
            .iter()
            .map(|s| s["kind"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["initial", "queue", "range_violation"]
    );
    let last = stages.last().unwrap();
    assert_eq!(last["events"], json!([]));
    assert!(last["result"].is_null());
    let values = last["values"]["observed"].as_array().unwrap();
    assert_eq!(
        values.iter().find(|v| v["name"] == "flag").unwrap()["value"],
        "false"
    );
    assert_eq!(
        values.iter().find(|v| v["name"] == "n").unwrap()["value"],
        "1"
    );
}
