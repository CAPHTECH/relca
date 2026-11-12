//! Named provenance for executable If branches (ADR 0064).
use crate::ir;

pub(crate) fn calls(b: &ir::Block) -> Vec<ir::Call> {
    let mut out = Vec::new();
    for s in b {
        match &s.kind {
            ir::Stmt::Call(c) => out.push(c.clone()),
            ir::Stmt::Response(_, r) => out.push(r.command_call.clone()),
            ir::Stmt::If {
                then_body,
                else_body,
                ..
            } => {
                out.extend(calls(then_body));
                out.extend(calls(else_body));
            }
            _ => (),
        }
    }
    out
}

#[cfg(test)]
fn derive_json(m: &ir::Model, limit: usize) -> serde_json::Value {
    serde_json::to_value(crate::agency::degradation::derive(m, limit)).unwrap()
}

#[cfg(test)]
#[path = "../tests/support/q6b_oracle.rs"]
mod hand;
#[cfg(test)]
#[path = "../tests/support/q6b_inputs.rs"]
mod input;

#[cfg(test)]
mod tests {
    use super::*;
    fn model(name: &str) -> ir::Model {
        crate::lower::lower(&crate::syntax::parse(&input::source(name)).unwrap()).unwrap()
    }
    #[test]
    fn budget_seam() {
        hand::budget_contract(|limit| derive_json(&model("a8"), limit));
    }
    #[test]
    fn static_hand_tables() {
        for name in input::RUNTIME.iter().filter(|n| **n != "qualified") {
            let src = input::source(name);
            let ast = crate::syntax::parse(&src).unwrap();
            let m =
                crate::lower::lower_with_selection(&ast, input::configuration(name), None).unwrap();
            assert_eq!(derive_json(&m, 1_000_000), hand::expected(name), "{name}");
        }
    }
    #[test]
    fn kernel_declared_domain_excludes_unproduced_enum_values() {
        let src = input::edit(
            &input::source("latent-ok"),
            "true -> { 1:fail }",
            "true -> { 1:ok }",
        );
        let m = crate::lower::lower(&crate::syntax::parse(&src).unwrap()).unwrap();
        let actual = derive_json(&m, 1_000_000);
        let mut expected = hand::expected("latent-ok");
        expected["routes"][0]["degraded"] = serde_json::json!({"status":"derived","reason":null,
            "ends":[{"end":"done","reachable":false,"counts":hand::counts(&[]),"may_require":[],"must_require":[]}],"observation":[]});
        assert_eq!(
            actual, expected,
            "kernel does not declare fail: no Clerk may dependency"
        );
    }
    #[test]
    fn structural_domain_ignores_branch_probabilities_including_zero() {
        let src = input::edit(
            &input::source("latent-ok"),
            "false -> { 1:ok }; true -> { 1:fail }",
            "false -> { 1:ok; 0:fail }; true -> { 1:ok; 0:fail }",
        );
        let changed = src.replace("1:ok; 0:fail", "0:ok; 1:fail");
        for source in [&src, &changed] {
            let m = crate::lower::lower(&crate::syntax::parse(source).unwrap()).unwrap();
            assert_eq!(
                derive_json(&m, 1_000_000),
                hand::expected("latent-ok"),
                "declared zero-mass branches remain structural hypotheses"
            );
        }
        // Corrected U2 ruling: a non-kernel declaration supplies its own domain,
        // rather than the return enum's unproduced alternatives.
        let src = input::edit(
            &input::source("a7"),
            "result nondet { ok, fail }",
            "result nondet { ok }",
        );
        let m = crate::lower::lower(&crate::syntax::parse(&src).unwrap()).unwrap();
        let mut expected = hand::expected("a7");
        expected["routes"][0]["degraded"] = serde_json::json!({"status":"derived","reason":null,
            "ends":[{"end":"done","reachable":false,"counts":hand::counts(&[]),"may_require":[],"must_require":[]}],"observation":[]});
        assert_eq!(
            derive_json(&m, 1_000_000),
            expected,
            "non-kernel declared result domain"
        );
    }
    fn prefix_pair(prefix: &str, required_acceptance: Option<bool>) {
        let src = input::edit(&input::source("a7"), "status = Probe.check();", prefix);
        let ordinary = src.replace(input::ROUTE, "if status == fail");
        let lower = |s: &str| crate::syntax::parse(s).and_then(|a| crate::lower::lower(&a));
        let a = lower(&ordinary);
        let b = lower(&src);
        if let Some(required) = required_acceptance {
            assert_eq!(a.is_ok(), required, "ordinary If control");
        }
        match (a, b) {
            (Err(a), Err(b)) => {
                let errors = |ds: Vec<crate::diag::Diagnostic>| {
                    ds.into_iter()
                        .filter(|d| d.severity == crate::diag::Severity::Error)
                        .map(|d| d.code)
                        .collect::<Vec<_>>()
                };
                let a = errors(a);
                let b = errors(b);
                assert!(!a.is_empty());
                assert_eq!(a, b, "same lower knownness codes");
                eprintln!("prefix pair: ordinary If and degrade rejected with {a:?}");
            }
            (Ok(a), Ok(b)) => {
                fn ends(o: &crate::outcome::Outcome) -> Vec<(usize, u64, u64, u64, u64)> {
                    let mut rows = if let Some(cs) = &o.case_outcomes {
                        cs.iter()
                            .flat_map(|c| &c.terminals)
                            .map(|t| {
                                let crate::outcome::CaseTerminalKind::End(e) = t.kind else {
                                    panic!("expected End")
                                };
                                let time = t.elapsed.unwrap();
                                let work = t.work.unwrap();
                                (e.0, time.min, time.max, work.min, work.max)
                            })
                            .collect::<Vec<_>>()
                    } else {
                        o.terminals
                            .iter()
                            .map(|t| {
                                (
                                    t.end.0,
                                    t.elapsed.min,
                                    t.elapsed.max,
                                    t.work.min,
                                    t.work.max,
                                )
                            })
                            .collect()
                    };
                    rows.sort();
                    rows
                }
                let mut all = Vec::new();
                for m in [&a, &b] {
                    let e = crate::engine::explore(m);
                    let r = crate::reference::explore(m);
                    assert_eq!(
                        r.stop,
                        crate::reference::StopReason::Complete,
                        "accepted source uses ordinary If semantics"
                    );
                    assert!(!e.truncated && !r.outcome.truncated);
                    assert!(
                        !e.diagnostics
                            .iter()
                            .any(|d| d.severity == crate::diag::Severity::Error),
                        "{:?}",
                        e.diagnostics
                    );
                    assert_eq!(ends(&e), ends(&r.outcome));
                    assert_eq!(e.properties, r.outcome.properties);
                    all.push(ends(&e));
                }
                assert_eq!(all[0], all[1], "degrade_to_if endpoints/work");
                if required_acceptance == Some(true) {
                    assert_eq!(
                        all[0],
                        vec![(0, 1, 5, 1, 5)],
                        "Review1; early End1, or Probe1 + Fast1/Clerk3"
                    );
                }
                eprintln!("prefix pair: both accepted and independently executed");
            }
            (a, b) => {
                panic!("degrade_to_if lower acceptance differs: ordinary={a:?}, degradation={b:?}")
            }
        }
    }
    #[test]
    fn prefix_one_producer_other_arm_ends_matches_if() {
        prefix_pair(
            "prefix_choice = Review.choice(); if prefix_choice == pass { status = Probe.check() } else { end done }",
            Some(true),
        );
    }
    #[test]
    fn prefix_both_arms_produce_matches_if() {
        prefix_pair(
            "prefix_choice = Review.choice(); if prefix_choice == pass { status = Probe.check() } else { status = Probe.check() }",
            None,
        );
    }
    #[test]
    fn prefix_one_producer_other_arm_continues_matches_if() {
        prefix_pair(
            "prefix_choice = Review.choice(); if prefix_choice == pass { status = Probe.check() } else { }",
            Some(false),
        );
    }
    #[test]
    fn qualification_absence_withdraws_side_and_prefix_starts() {
        let src = input::edit(
            &input::source("qualified"),
            "qualified_executors [z]",
            "qualified_executors []",
        );
        let r = serde_json::to_value(crate::report::check(&src)).unwrap();
        let side = &r["agency"]["degradation"]["routes"][0]["degraded"];
        assert_eq!(side["status"], "not_derived");
        assert_eq!(side["reason"], "qualification_unavailable");
        assert!(side["ends"].is_null() && side["observation"].is_null());
        let src = input::edit(
            &input::base(),
            "capability check() -> Status {",
            "capability check() -> Status { qualified_executors []; ",
        );
        let r = serde_json::to_value(crate::report::check(&src)).unwrap();
        assert_eq!(r["agency"]["degradation"]["status"], "not_derived");
        assert_eq!(
            r["agency"]["degradation"]["reason"],
            "qualification_unavailable"
        );
        assert!(r["agency"]["degradation"]["routes"].is_null());
    }
    #[test]
    fn unknown_arrival_coverage_withdraws_only_when_calendar_is_present() {
        // Artificial declaration-boundary state: this is a private derivation
        // test, not a claim that empty arrivals is accepted by either backend.
        let mut calendar = model("calendar");
        calendar.arrivals = Some(Vec::new());
        let mut no_calendar = calendar.clone();
        no_calendar.resource_time = None;
        assert_eq!(
            derive_json(&no_calendar, 1_000_000),
            hand::expected("a7"),
            "without calendars, a temporal envelope is not needed for structural counts"
        );
        let mut absent = hand::expected("a7");
        absent["status"] = serde_json::json!("not_derived");
        absent["reason"] = serde_json::json!("arrival_coverage");
        absent["routes"] = serde_json::Value::Null;
        assert_eq!(derive_json(&calendar, 1_000_000), absent);
    }
    #[test]
    fn qualification_wire_matches_work_profile_declarations_in_both_directions() {
        let r = crate::report::check(&input::source("qualified"));
        serde_json::from_value::<crate::report::Report>(serde_json::to_value(&r).unwrap()).unwrap();
        for fault in ["observation", "declaration", "missing_profiles"] {
            let mut bad = r.clone();
            match fault {
                "observation" => {
                    bad.agency
                        .as_mut()
                        .unwrap()
                        .degradation
                        .as_mut()
                        .unwrap()
                        .routes
                        .as_mut()
                        .unwrap()[0]
                        .degraded
                        .observation
                        .as_mut()
                        .unwrap()[0]
                        .qualified = Some(vec!["c".into()])
                }
                "declaration" => {
                    bad.work_profiles
                        .as_mut()
                        .unwrap()
                        .qualifications
                        .iter_mut()
                        .find(|q| q.capability == "Clerk.enter")
                        .unwrap()
                        .qualified_executors = Some(vec!["c".into()])
                }
                "missing_profiles" => bad.work_profiles = None,
                _ => unreachable!(),
            }
            let bad = serde_json::to_value(crate::report::seal(bad)).unwrap();
            assert!(
                serde_json::from_value::<crate::report::Report>(bad).is_err(),
                "{fault}"
            );
        }
    }
    #[test]
    fn performance_declared_branches_remain_the_structural_result_domain() {
        let src = input::edit(
            &input::base(),
            "  people",
            "  kernel K { * -> { 1:ok; 0:fail } }\n  performance P basis \"fixture assumption\" { load none; executor p { kernel K } }\n  people",
        );
        let src = input::edit(&src, "result nondet { ok, fail }", "outcomes performance P");
        let r = crate::report::check(&src);
        let v = serde_json::to_value(&r).unwrap();
        hand::validate_report("a7", &v).unwrap();
        serde_json::from_value::<crate::report::Report>(v).unwrap();
    }
    #[test]
    fn route_ir_seam() {
        hand::route_ir_contract(|fault| {
            let mut m = model("a7");
            let r = &mut m.process.degradations[0];
            match fault {
                "valid" => (),
                "producer_outside_model" => r.producer = ir::CapabilityId(m.capabilities.len()),
                "condition_producer_mismatch" => r.producer = ir::CapabilityId(3),
                "degraded_calls_disagree_with_ir" => {
                    r.degraded_calls[0].capability = ir::CapabilityId(3)
                }
                _ => unreachable!(),
            }
            (
                crate::engine::explore(&m)
                    .diagnostics
                    .into_iter()
                    .map(|d| (d.code, d.message))
                    .collect(),
                format!("{:?}", crate::reference::explore(&m).stop),
            )
        });
    }
}
