//! ADR 0027 hand oracles. No production authority predicate is an oracle.
use relca_core::{engine, lower, reference, report, syntax};
use serde_json::Value;
use std::path::PathBuf;

#[path = "support/authority_population.rs"]
mod authority_population;
pub use authority_population::NAMES;
pub fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}
pub fn source(name: &str) -> String {
    std::fs::read_to_string(root().join(name).join("model.relca")).unwrap()
}
pub fn expected(name: &str) -> Value {
    serde_json::from_slice(&std::fs::read(root().join(name).join("expected.json")).unwrap())
        .unwrap()
}
pub fn check(src: &str) -> Value {
    serde_json::to_value(report::check(src)).unwrap()
}
pub fn subset(e: &Value, a: &Value) {
    match e {
        Value::Object(o) => {
            for (k, v) in o {
                subset(v, a.get(k).unwrap_or_else(|| panic!("missing {k}: {a}")));
            }
        }
        Value::Array(es) => {
            let actual = a.as_array().unwrap();
            assert_eq!(es.len(), actual.len(), "array inventory: {a}");
            for (x, y) in es.iter().zip(actual) {
                subset(x, y);
            }
        }
        _ => assert_eq!(e, a, "hand oracle"),
    }
}
pub fn hand_report(e: &Value, a: &Value) {
    let mut hand = e.clone();
    let o = hand.as_object_mut().unwrap();
    let diagnostics = o.remove("diagnostics");
    let absent = o.remove("absent_diagnostics");
    let assumptions = o.remove("assumptions");
    subset(&hand, a);
    for d in diagnostics.iter().filter_map(Value::as_array).flatten() {
        assert!(
            a["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|actual| actual["code"] == d["code"] && actual["severity"] == d["severity"]),
            "missing diagnostic {d}: {a}"
        );
    }
    for code in absent.iter().filter_map(Value::as_array).flatten() {
        assert!(
            !a["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["code"] == *code),
            "unexpected {code}"
        );
    }
    for row in assumptions.iter().filter_map(Value::as_array).flatten() {
        let actual: Vec<_> = a["assumptions"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|v| v["field"] == row["field"])
            .collect();
        assert_eq!(actual.len(), 1);
        subset(row, actual[0]);
    }
}
fn fixture(name: &str) {
    hand_report(&expected(name), &check(&source(name)));
}
macro_rules! cases { ($($test:ident => $name:literal),* $(,)?) => { $(#[test] fn $test() { fixture($name); })* }; }
cases! {
    a01_scope => "auth-scope", a01_false => "auth-scope.broken-false-branch",
    a02_post => "auth-scope-post", a02_false => "auth-scope-post.broken-finish-value",
    a03_observed => "auth-scope-latent", a03_latent => "auth-scope-latent.broken-latent",
    a04_exact => "auth-expiry", a04_early => "auth-expiry.broken-minute-early",
    a05_exact_clock => "auth-expiry-cut", a05_cut => "auth-expiry-cut.broken-short-clock",
    a06_absolute => "auth-expiry-absolute", a06_early => "auth-expiry-absolute.broken-earlier-expiry",
    a07_subset => "auth-delegation", a07_wider => "auth-delegation.broken-wider-grants",
    a08_forward => "auth-delegation-forward", a08_cycle => "auth-delegation-forward.broken-cycle",
    a09_union => "auth-delegation-not-by", a09_forbidden => "auth-delegation-not-by.broken-forbidden-person",
    a10_name => "auth-delegation-name", a10_missing => "auth-delegation-name.broken-missing-parent",
    a11_ceiling => "auth-delegation-expiry", a11_longer => "auth-delegation-expiry.broken-longer-child",
    a12_issuer => "auth-delegation-issuer", a12_wrong_cap => "auth-delegation-issuer.broken-issuer",
    a13_no_warning => "auth-self-grant", a13_warning => "auth-self-grant.broken-self-issued",
    a14_waive => "auth-waive", a14_restricted => "auth-waive.broken-restricted-authority",
    a15_contest => "auth-contest", a15_restricted => "auth-contest.broken-restricted-reversal",
    a16_record => "auth-revocation", a16_removed => "auth-revocation.broken-missing-record",
    a17_inherited => "auth-inherited-scope", a17_false => "auth-inherited-scope.broken-parent-scope",
    a18_inherited => "auth-inherited-expiry", a18_early => "auth-inherited-expiry.broken-parent-earlier",
    a19_rejected => "auth-requires-integration", a19_started => "auth-requires-integration.broken-starts",
    a20_violation => "auth-known-violation-cut", a20_cut => "auth-known-violation-cut.broken-clock",
}

/// A control, not a feature oracle: erase only the newly introduced clauses.
pub fn old_skeleton(src: &str) -> String {
    src.lines()
        .map(|line| {
            let mut s = line.to_owned();
            if line.trim_start().starts_with("authority ") {
                for word in ["scope", "expires", "revocable_by", "delegated_from"] {
                    while let Some(start) = s.find(&format!("; {word} ")) {
                        let end = s[start + 2..]
                            .find([';', '}'])
                            .map(|i| start + 2 + i)
                            .unwrap();
                        s.replace_range(start..end, "");
                    }
                }
            }
            for condition in ["true", "false"] {
                s = s.replace(
                    &format!("requires enabled == {condition} else rejected; "),
                    "",
                );
            }
            s
        })
        .collect::<Vec<_>>()
        .join("\n")
}
#[test]
fn all_forty_old_skeletons_parse_and_lower_before_feature_implementation() {
    for n in NAMES {
        let s = old_skeleton(&source(n));
        lower::lower(&syntax::parse(&s).unwrap_or_else(|e| panic!("{n}: {e:?}\n{s}")))
            .unwrap_or_else(|e| panic!("{n}: {e:?}\n{s}"));
    }
}

#[test]
fn old_skeleton_business_arithmetic_is_hand_derived_not_feature_dependent() {
    let mut checked = 0;
    for name in NAMES {
        let hand = expected(name);
        if hand["verdict"] == "invalid"
            || hand["verdict"] == "unsupported"
            || name.starts_with("auth-requires-integration")
        {
            continue;
        }
        let actual = check(&old_skeleton(&source(name)));
        subset(&hand["terminals"], &actual["terminals"]);
        if let Some(cases) = hand.get("case_outcomes") {
            subset(cases, &actual["case_outcomes"]);
        }
        checked += 1;
    }
    assert_eq!(
        checked, 29,
        "31 runtime models minus two ADR 0026 requires controls"
    );
}

#[test]
fn a09_effective_not_by_is_union_and_public_ir_cannot_drop_inheritance() {
    let m = lower::lower(&syntax::parse(&source("auth-delegation-not-by")).unwrap()).unwrap();
    let child = m
        .authorities
        .iter()
        .position(|a| a.name == "Child")
        .unwrap();
    let names: std::collections::BTreeSet<_> = m.authorities[child]
        .not_by
        .iter()
        .map(|p| m.people[p.0].name.as_str())
        .collect();
    assert_eq!(names, ["r", "z"].into_iter().collect());
    let mut bad = m.clone();
    bad.authorities[child].not_by.clear();
    assert!(
        engine::explore(&bad)
            .diagnostics
            .iter()
            .any(|d| d.code == "U-SEM-0")
    );
    assert_eq!(
        reference::explore(&bad).verdict(),
        relca_core::Verdict::Unsupported
    );
}

#[test]
fn scope_matrix_diagnostics_and_precise_name_spans() {
    let base = source("auth-scope-latent");
    for (from, to, code, token) in [
        (
            "scope approved == true",
            "scope missing == true",
            "E-NAME-0",
            "missing",
        ),
        ("scope approved == true", "scope a == pass", "U-SEM-0", "a"),
        (
            "scope approved == true",
            "scope approved == pass",
            "E-TYPE-0",
            "approved",
        ),
        ("observed case", "latent case", "E-POL-LATENT", "approved"),
    ] {
        let s = base.replace(from, to);
        let v = check(&s);
        assert_eq!(
            v["verdict"],
            if code.starts_with('E') {
                "invalid"
            } else {
                "unsupported"
            }
        );
        let d = v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .find(|d| d["code"] == code)
            .unwrap_or_else(|| panic!("{code}: {v}"));
        let a = d["span"]["start"].as_u64().unwrap() as usize;
        let b = d["span"]["end"].as_u64().unwrap() as usize;
        assert_eq!(&s[a..b], token, "scope token span {code}");
    }
    for condition in [
        "approved == true || missing == true",
        "approved == true || (approved == false && missing == true)",
    ] {
        let v = check(&base.replace("approved == true }", &format!("{condition} }}")));
        assert!(
            v["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["code"] == "E-NAME-0"),
            "all atoms: {v}"
        );
    }
    let latent = base
        .replace("observed case", "latent case")
        .replace(
            "scope approved == true",
            "scope approved == true || approved == false",
        )
        .replace("a = A.run(); ", "");
    assert_eq!(
        check(&latent)["verdict"],
        "invalid",
        "unused scope remains policy"
    );
}

#[test]
fn duplicate_fields_duration_units_overflow_and_revoker_names() {
    let base = source("auth-revocation");
    for clause in [
        "scope missing == true",
        "expires 3min",
        "revocable_by p",
        "delegated_from G",
    ] {
        let original = format!("{clause}; {clause}");
        let s = if clause.starts_with("expires") || clause.starts_with("revocable") {
            base.replace(clause, &original)
        } else {
            base.replace("; expires 3min", &format!("; {original}; expires 3min"))
        };
        assert!(
            check(&s)["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["code"] == "U-SEM-0"),
            "duplicate {clause}"
        );
    }
    for (duration, code) in [
        ("1.5min", "U-SYN-0"),
        // Reuse duration()/ticks(), as elsewhere.
        ("1week", "U-SEM-0"),
        ("18446744073709551616min", "U-SYN-0"),
        ("18446744073709551615d", "U-SEM-0"),
    ] {
        let v = check(&base.replace("expires 3min", &format!("expires {duration}")));
        assert!(
            v["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["code"] == code),
            "duration {duration}: {v}"
        );
    }
    for duration in ["1h", "1d"] {
        assert_eq!(
            check(&base.replace("expires 3min", &format!("expires {duration}")))["verdict"],
            "satisfied"
        );
    }
    for (s, code) in [
        (
            base.replace("revocable_by p", "revocable_by Missing"),
            "E-NAME-0",
        ),
        (base.replace("actor A", "actor p {}\nactor A"), "E-CON-0"),
    ] {
        assert!(
            check(&s)["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["code"] == code),
            "revoker: {s}"
        );
    }
}

#[test]
fn expiry_without_commits_zero_boundary_and_event_snapshot_not_final_now() {
    for n in ["auth-expiry", "auth-expiry.broken-minute-early"] {
        let s = source(n).replace("commits; ", "");
        hand_report(&expected(n), &check(&s));
    }
    let s = source("auth-expiry")
        .replace("fixed 5min", "fixed 0min")
        .replace("expires 5min", "expires 0min");
    assert_eq!(
        check(&s)["properties"][1]["verdict"],
        "satisfied",
        "t0 <= D0; engine-only zero-duration boundary"
    );
    let zero = source("auth-expiry").replace("expires 5min", "expires 0min");
    assert_eq!(
        check(&zero)["properties"][1]["verdict"],
        "violated",
        "zero is not infinity"
    );
    let s = source("auth-scope-post").replace("; scope enabled", "; expires 2min; scope enabled");
    let m = lower::lower(&syntax::parse(&s).unwrap()).unwrap();
    for o in [engine::explore(&m), reference::explore(&m).outcome] {
        assert_eq!(
            o.properties[1].verdict,
            relca_core::Verdict::Satisfied,
            "A finish2, Reset finish3"
        );
    }
}

#[test]
fn delegation_additional_capability_issuer_cycle_and_equality_controls() {
    let base = source("auth-delegation-issuer");
    for s in [
        base.replace("issued_by q; expires 5min", "expires 5min"),
        base.replace(
            "grants q to A.run; issued_by q",
            "grants q to A.other; issued_by q",
        ),
    ] {
        assert_eq!(
            check(&s)["verdict"],
            "invalid",
            "issuer mandatory and subset keyed by capability"
        );
    }
    let d = source("auth-delegation");
    assert_eq!(
        check(&d.replace("expires 5min", "expires 10min"))["verdict"],
        "satisfied"
    );
    assert_eq!(
        check(&d.replace("delegated_from Parent", "delegated_from Child"))["verdict"],
        "invalid"
    );
    let split = d.replace(
        "grants [p,q] to A.run",
        "grants p to A.run; grants q to A.run",
    );
    assert_eq!(
        check(&split)["verdict"],
        "satisfied",
        "same-cap lines form a union"
    );
    assert_eq!(
        check(
            &d.replace("; expires 10min", "")
                .replace("; expires 5min", "")
        )["verdict"],
        "satisfied"
    );
}

#[test]
fn delegation_subset_is_capability_person_pairs_not_person_union() {
    // Parent has q for run, p for other. Child gives q other, issued by p.
    // Issuer DOES hold other; only the child's (other,q) subset rule fails.
    let s = source("auth-delegation-issuer")
        .replace(
            "grants q to A.run; issued_by q",
            "grants q to A.other; issued_by p",
        )
        .replace("a = A.run()", "a = A.other()")
        .replace(
            "authority_basis A.run Child",
            "authority_basis A.other Child",
        );
    let value = check(&s);
    assert_eq!(value["verdict"], "invalid");
    assert!(
        value["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "E-CON-0")
    );
    let old = old_skeleton(&s);
    lower::lower(&syntax::parse(&old).unwrap()).unwrap();
}

#[test]
fn warning_gate_is_model_wide_and_never_changes_exit() {
    let old = old_skeleton(&source("auth-self-grant.broken-self-issued"));
    let warning = |v: Value| {
        v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|d| d["code"] == "W-AUTH-0")
            .cloned()
            .collect::<Vec<_>>()
    };
    assert!(warning(check(&old)).is_empty());
    let mixed = old.replace(
        "process Flow",
        "authority Record { basis \"record\"; grants q to A.run; revocable_by r }\nprocess Flow",
    );
    let v = check(&mixed);
    assert_eq!(v["verdict"], "satisfied");
    let ds = warning(v);
    assert_eq!(ds.len(), 1);
    assert_eq!(ds[0]["severity"], "warning");
    let a = ds[0]["span"]["start"].as_u64().unwrap() as usize;
    let b = ds[0]["span"]["end"].as_u64().unwrap() as usize;
    assert_eq!(&mixed[a..b], "issued_by p");
}

#[test]
fn waive_and_contest_restrictions_are_reference_specific_not_revocation() {
    let waive = source("auth-waive").replace(
        "  people",
        "  observed case { approved: bool = true }\n  people",
    );
    for clause in ["scope approved == true", "expires 100min"] {
        assert_eq!(
            check(&waive.replace(
                "issued_by issuer }",
                &format!("issued_by issuer; {clause} }}")
            ))["verdict"],
            "unsupported"
        );
    }
    let delegated=waive.replace("authority G", "authority Parent { basis \"parent\"; grants [issuer,p,q] to Officer.release }\n  authority G").replace("issued_by issuer }", "issued_by issuer; delegated_from Parent }");
    assert_eq!(check(&delegated)["verdict"], "unsupported");
    let w =
        source("auth-waive").replace("issued_by issuer }", "issued_by issuer; revocable_by p }");
    assert_eq!(
        check(&w)["verdict"],
        "satisfied",
        "record-only waive remains supported"
    );
    let c = source("auth-contest.broken-restricted-reversal").replace(
        "process Flow",
        "authority H { basis \"alternative\"; grants j to Judge.judge }\nprocess Flow",
    );
    assert_eq!(
        check(&c)["verdict"],
        "unsupported",
        "unrestricted alternative cannot hide restricted candidate"
    );
    for target in ["Route.respond", "Judge.judge"] {
        let s = c.replace(
            "grants j to Judge.judge; expires 100min",
            &format!("grants p to {target}; expires 100min"),
        );
        assert_eq!(
            check(&s)["verdict"],
            "satisfied",
            "unrelated candidate must not contaminate contest"
        );
    }
}

#[test]
fn all_twenty_pairs_are_exact_single_source_edits() {
    let edits = [
        (
            "result nondet { pass }; writes approval",
            "result nondet { pass, fail }; writes approval",
        ),
        ("finish { enabled = true }", "finish { enabled = false }"),
        ("observed case", "latent case"),
        ("expires 5min", "expires 4min"),
        ("clock 5min", "clock 4min"),
        ("expires 7min", "expires 6min"),
        (
            "grants q to A.run; issued_by p",
            "grants [q,r] to A.run; issued_by p",
        ),
        ("expires 10min }", "expires 10min; delegated_from Child }"),
        (
            "grants q to A.run; issued_by p",
            "grants z to A.run; issued_by p",
        ),
        ("delegated_from Parent", "delegated_from Missing"),
        ("expires 5min", "expires 11min"),
        ("issued_by q; expires 5min", "issued_by p; expires 5min"),
        ("issued_by r", "issued_by p"),
        ("issued_by issuer }", "issued_by issuer; expires 100min }"),
        (
            "grants j to Judge.judge }",
            "grants j to Judge.judge; expires 100min }",
        ),
        ("; revocable_by p", ""),
        ("scope approved == true", "scope approved == false"),
        ("expires 5min", "expires 4min"),
        ("requires enabled == true", "requires enabled == false"),
        ("clock 10min", "clock 3min"),
    ];
    for (pair, (from, to)) in NAMES.as_chunks::<2>().0.iter().zip(edits) {
        let good = source(pair[0]);
        assert_eq!(good.matches(from).count(), 1, "one edit site {}", pair[0]);
        assert_eq!(
            good.replacen(from, to, 1),
            source(pair[1]),
            "single semantic fault {}",
            pair[1]
        );
    }
}

#[test]
fn unknown_authority_fixture_cannot_be_silently_excluded_from_old_cohorts() {
    for name in NAMES {
        assert!(authority_population::fixture(name));
    }
    assert!(!authority_population::fixture("serial-time"));
    assert!(
        std::panic::catch_unwind(|| authority_population::fixture("auth-not-registered")).is_err()
    );
}

#[test]
fn budgets_never_turn_unobserved_finish_into_expiry_or_hide_known_violation() {
    let s = source("auth-expiry");
    let m = lower::lower(&syntax::parse(&s).unwrap()).unwrap();
    for budget in [0, 1] {
        let o = engine::explore_with_budget(&m, budget);
        assert!(o.truncated);
        assert_eq!(o.properties[1].verdict, relca_core::Verdict::Inconclusive);
    }
    let s = source("auth-known-violation-cut");
    let m = lower::lower(&syntax::parse(&s).unwrap()).unwrap();
    let retained = (1..256)
        .map(|n| engine::explore_with_budget(&m, n))
        .find(|o| o.truncated && o.properties[1].verdict == relca_core::Verdict::Violated);
    assert!(
        retained.is_some(),
        "known finish violation must survive later budget cut"
    );
}

#[test]
fn scope_optional_none_boolean_groups_and_actor_latent_are_not_silently_omitted() {
    let none = source("auth-scope")
        .replace("scope approval == pass", "scope approval == none")
        .replace("j = Judge.judge(); ", "");
    assert_eq!(check(&none)["properties"][1]["verdict"], "satisfied");
    let base = source("auth-scope-latent");
    let grouped = base.replace(
        "scope approved == true",
        "scope (approved == true) && (approved == true || approved == false)",
    );
    assert_eq!(check(&grouped)["verdict"], "satisfied");
    let local = base
        .replace(
            "actor A",
            "actor Hidden { latent truth: bool = true }\n  actor A",
        )
        .replace("scope approved == true", "scope Hidden.truth == true");
    let value = check(&local);
    assert_eq!(value["verdict"], "invalid");
    assert!(
        value["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "E-POL-LATENT")
    );
    // Assert old syntax scaffolding separately, before any new scope rule.
    lower::lower(&syntax::parse(&old_skeleton(&local)).unwrap()).unwrap();
}

#[test]
fn three_level_scope_and_not_by_inheritance_and_independent_cycles() {
    let src=source("auth-inherited-scope").replace("authority Parent", "authority Grand { basis \"grand\"; grants [r,p,q] to A.run; issued_by z; expires 10min; scope approved == false }\n  authority Parent").replace("scope approved == true }", "scope approved == true; delegated_from Grand }");
    assert_eq!(
        check(&src)["properties"][1]["verdict"],
        "violated",
        "grandparent false cannot be replaced by parent true"
    );
    let src=source("auth-delegation-not-by").replace("people [r,p,q,z]","people [r,p,q,z,x]").replace("authority Parent", "authority Grand { basis \"grand\"; grants [r,p,q] to A.run; issued_by z; not_by [x]; expires 10min }\n  authority Parent").replace("not_by [z]; expires 10min }", "not_by [z]; expires 10min; delegated_from Grand }");
    let m = lower::lower(&syntax::parse(&src).unwrap()).unwrap();
    let child = m.authorities.iter().find(|a| a.name == "Child").unwrap();
    let names: std::collections::BTreeSet<_> = child
        .not_by
        .iter()
        .map(|p| m.people[p.0].name.as_str())
        .collect();
    assert_eq!(names, ["r", "z", "x"].into_iter().collect());
    let cycle=source("auth-delegation-forward.broken-cycle").replace("process Flow", "authority X { basis \"x\"; grants q to A.run; issued_by q; delegated_from Y }\n  authority Y { basis \"y\"; grants q to A.run; issued_by q; delegated_from X }\n  process Flow");
    let v = check(&cycle);
    assert_eq!(
        v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|d| d["code"] == "E-CON-0")
            .count(),
        2,
        "one diagnostic per independent cycle; suppress derivative rules"
    );
}

#[test]
fn cancellation_ack_is_not_authority_finish_even_after_expiry() {
    let src = std::fs::read_to_string(root().join("exec-cancel/model.relca"))
        .unwrap()
        .replace(
            "  people",
            "  observed case { allowed: bool = false }\n  people",
        )
        .replace(
            "grants q to Slow.work }",
            "grants q to Slow.work; scope allowed == true; expires 0min }",
        );
    assert_eq!(check(&src)["properties"][2]["verdict"], "vacuous");
    let m = lower::lower(&syntax::parse(&src).unwrap()).unwrap();
    let r = reference::explore(&m);
    assert_eq!(r.stop, reference::StopReason::Complete);
    assert_eq!(
        r.outcome.properties[2].verdict,
        relca_core::Verdict::Vacuous
    );
    assert!(r.statistics.cancellation_completions > 0);
}

#[test]
fn kernel_finish_override_is_in_authority_event_snapshot() {
    let src = source("auth-scope-post")
        .replace(
            "result nondet { pass }; writes enabled; finish { enabled = true }",
            "outcomes kernel K; writes enabled",
        )
        .replace(
            "  process Flow",
            "  kernel K { * -> { 1: pass with enabled = true } }\n  process Flow",
        );
    lower::lower(&syntax::parse(&old_skeleton(&src)).unwrap()).unwrap();
    let m = lower::lower(&syntax::parse(&src).unwrap()).unwrap();
    for o in [engine::explore(&m), reference::explore(&m).outcome] {
        assert_eq!(
            o.properties[1].verdict,
            relca_core::Verdict::Satisfied,
            "kernel writes true at2; Reset writes false at3"
        );
    }
}

// Boundary review minor-2 (ADR 0027 裁定 U1): an omitted expiry in the middle
// of a three-level chain inherits the grandparent's effective expiry. Hand:
// A.run finishes at 5; Grand expires at 4 -> violated; with Grand at 5 the
// finish at 5 is inside (D inclusive) -> satisfied.
#[test]
fn three_level_expiry_inheritance_through_an_omitted_middle() {
    let src = "model Authority version \"0.2\" {\n  type T = enum { pass }\n  people [r,p,q]\n  actor A { executors [q]; capability run() -> T { duration fixed 5min; result nondet { pass } } }\n  permit q to A.run\n  authority Grand { basis \"g\"; grants [p,q] to A.run; issued_by r; expires 4min }\n  authority Parent { basis \"p\"; grants [p,q] to A.run; issued_by p; delegated_from Grand }\n  authority Child { basis \"c\"; grants q to A.run; issued_by p; delegated_from Parent }\n  process Flow { a = A.run(); end done }\n  bounds { cases 1; live_tasks 2; clock 20min }\n  verify safety no_resource_overcommit\n  contract Controls { authority_basis A.run Child }\n}\n";
    let basis = |s: &str| {
        check(s)["properties"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["id"].as_str().unwrap().contains("authority_basis"))
            .unwrap()["verdict"]
            .clone()
    };
    assert_eq!(basis(src), "violated");
    assert_eq!(
        basis(&src.replace("expires 4min", "expires 5min")),
        "satisfied"
    );
}

// Own mutation M12a (ADR 0027 × ADR 0026): on the arrivals path too, a call
// refused by requires emits no Finish, so authority_basis never starts. Hand:
// the requires guard is false, the call is refused, nothing finishes or
// commits -> authority_basis vacuous and the root vacuous (as A19's single case).
#[test]
fn refused_call_on_the_arrivals_path_is_not_an_authority_event() {
    let src = source("auth-requires-integration").replacen(
        "  bounds",
        "  arrivals fixed [0min]\n  bounds",
        1,
    );
    let a = check(&src);
    assert_eq!(a["verdict"], "vacuous", "{a}");
    let basis = a["properties"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"].as_str().unwrap().contains("authority_basis"))
        .unwrap();
    assert_eq!(basis["verdict"], "vacuous", "{a}");
}
