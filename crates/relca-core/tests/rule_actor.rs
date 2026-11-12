//! ADR0044 independently calculated acceptance; intentionally RED pre-implementation.
#[path = "support/rule_acceptance.rs"]
mod h;
use relca_core::{engine, lower, reference, syntax};
use serde_json::{Value, json};

include!("support/rule_cases.rs");

#[test]
fn all_seventy_independently_erased_old_syntax_substrates_lower() {
    let ss: Vec<Value> =
        serde_json::from_str(include_str!("support/rule_substrates.json")).unwrap();
    assert_eq!(ss.len(), 70);
    let mut errors = Vec::new();
    for s in ss {
        let src = s["source"].as_str().unwrap();
        match syntax::parse(src) {
            Ok(ast) => {
                if let Err(ds) = lower::lower(&ast) {
                    errors.push(format!("{}: {ds:?}", s["name"]));
                }
            }
            Err(ds) => errors.push(format!("{}: {ds:?}", s["name"])),
        }
    }
    assert!(errors.is_empty(), "unrelated substrate errors: {errors:?}");
}

fn fixture(name: &str) {
    let r = h::row(name);
    let src = h::source(name);
    h::verify(name, &h::actual(&src, &r["invocation"]));
}

#[test]
fn closed_population_has_exact_single_edit_or_invocation_witnesses() {
    let pairs: Vec<Value> = serde_json::from_str(include_str!("support/rule_pairs.json")).unwrap();
    let names = h::names();
    assert_eq!(names.len(), pairs.len() * 2);
    for p in pairs {
        let a = h::source(p["good"].as_str().unwrap());
        let b = h::source(p["broken"].as_str().unwrap());
        assert!(!a.contains("//") && !b.contains("//"));
        if p["invocation_only"] == true {
            assert_eq!(a, b);
            assert_ne!(
                h::row(p["good"].as_str().unwrap())["invocation"],
                h::row(p["broken"].as_str().unwrap())["invocation"]
            );
        } else {
            assert_eq!(
                h::replace(
                    &a,
                    p["before"].as_str().unwrap(),
                    p["after"].as_str().unwrap()
                ),
                b
            );
        }
    }
    let mut actual: Vec<_> = std::fs::read_dir(h::root().join("fixtures"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("rule-"))
        .collect();
    actual.sort();
    assert_eq!(actual, names);
}

#[test]
fn every_runtime_input_matches_hand_arithmetic_in_each_interpreter() {
    let mut completed = 0;
    let mut static_count = 0;
    let mut failures = Vec::new();
    for r in h::rows() {
        let n = r["name"].as_str().unwrap();
        if r["invocation"]["command"] == "diff" {
            continue;
        }
        if ["invalid", "unsupported"].contains(&r["expected"]["verdict"].as_str().unwrap()) {
            static_count += 1;
            continue;
        }
        // Catch per input, so one parser RED does not conceal the denominator.
        let result = std::panic::catch_unwind(|| {
            let m = h::lowered(&h::source(n), &r["invocation"]);
            let e = engine::explore(&m);
            let rr = reference::explore(&m);
            assert_eq!(
                rr.stop,
                reference::StopReason::Complete,
                "reference exclusion forbidden {n}"
            );
            assert_eq!(
                json!(rr.verdict()),
                r["expected"]["verdict"],
                "reference hand verdict {n}"
            );
            for (who, o) in [("engine", &e), ("reference", &rr.outcome)] {
                if let Some(ts) = r["expected"]["terminals"].as_array() {
                    let actual:Vec<_>=o.terminals.iter().map(|t|json!({"end":m.ends[t.end.0],"elapsed":t.elapsed,"work":t.work,"result_at":t.result_at})).collect();
                    h::subset(&json!({"terminals":ts}), &json!({"terminals":actual}));
                }
                if let Some(fifo) = r["oracle"]["cases"].as_str() {
                    let cases: Vec<_> = o
                        .case_outcomes
                        .as_ref()
                        .unwrap()
                        .iter()
                        .map(|c| {
                            let terminals: Vec<_> = c
                                .terminals
                                .iter()
                                .map(|t| {
                                    let relca_core::outcome::CaseTerminalKind::End(end) = t.kind
                                    else {
                                        panic!("unexpected rejection/future case {who} {n}")
                                    };
                                    json!({"end":m.ends[end.0],"elapsed":t.elapsed,"work":t.work})
                                })
                                .collect();
                            json!({"terminals":terminals})
                        })
                        .collect();
                    h::verify_case_times(&json!({"case_outcomes":cases}), fifo);
                }
            }
            let mut ro = rr.outcome;
            assert!(ro.diagnostics.starts_with(&m.diagnostics));
            ro.diagnostics.drain(..m.diagnostics.len());
            assert_eq!(e, ro, "independent complete Outcome, not just verdict {n}");
            assert!(
                !e.diagnostics.iter().any(|d| d.code == "W-RESP-0"),
                "engine has no prepended lower warnings"
            );
        });
        if result.is_err() {
            failures.push(n.to_owned());
        } else {
            completed += 1;
        }
    }
    assert!(
        failures.is_empty(),
        "runtime RED inputs: {failures:?}; completed={completed}; static={static_count}"
    );
    assert_eq!(
        (completed, static_count),
        (73, 53),
        "zero runtime exclusions, seven diff pairs handled separately"
    );
}

#[test]
fn all_twenty_six_lowerable_diff_sides_match_both_interpreters_before_comparison() {
    let mut complete = 0;
    let mut excluded = 0;
    let mut failures = Vec::new();
    for r in h::rows() {
        if r["invocation"]["command"] != "diff" {
            continue;
        }
        if r["oracle"]["error_in_sides"] == true {
            excluded += 2;
            continue;
        }
        let name = r["name"].as_str().unwrap();
        for side in ["from", "to"] {
            let result = std::panic::catch_unwind(|| {
                let selection = r["invocation"][side].as_str().unwrap();
                let m = h::lowered(
                    &h::source(name),
                    &json!({"command":"check","configuration":selection}),
                );
                let e = engine::explore(&m);
                let rr = reference::explore(&m);
                let hand = &r["oracle"]["side_expected"][side];
                assert_eq!(rr.stop, reference::StopReason::Complete);
                assert_eq!(json!(rr.verdict()), hand["verdict"]);
                for o in [&e, &rr.outcome] {
                    let terminals:Vec<_>=o.terminals.iter().map(|t|json!({"end":m.ends[t.end.0],"elapsed":t.elapsed,"work":t.work,"result_at":t.result_at})).collect();
                    h::subset(
                        &json!({"terminals":hand["terminals"]}),
                        &json!({"terminals":terminals}),
                    );
                }
                let mut ro = rr.outcome;
                assert!(ro.diagnostics.starts_with(&m.diagnostics));
                ro.diagnostics.drain(..m.diagnostics.len());
                assert_eq!(e, ro, "whole diff-side Outcome {name}/{selection}");
            });
            if result.is_err() {
                failures.push(format!("{name}/{side}"));
            } else {
                complete += 1;
            }
        }
    }
    assert!(failures.is_empty(), "RED diff sides: {failures:?}");
    assert_eq!(
        (complete, excluded),
        (26, 2),
        "only the two unsupported override sides are excluded"
    );
}

#[test]
fn lower_rejects_each_rule_write_even_in_unused_capabilities() {
    for label in ["finish", "start", "result", "atomic"] {
        let n = format!("rule-write-{label}.broken-s04{label}");
        let src = h::source(&n);
        let a = syntax::parse(&src).expect("write is syntactically valid");
        let ds = lower::lower(&a).expect_err("lower itself must reject before execution");
        assert!(
            ds.iter().any(|d| d.code == "U-SEM-0"
                && d.message == "rule values cannot be written during a run")
        );
    }
}

#[test]
fn clock_commit_equality_is_not_a_cut_before_commit() {
    let src = h::source("rule-basic.broken-t01");
    let c = h::replace(&src, "clock 20min", "clock 3min");
    let a = h::actual(&c, &json!({"command":"check","configuration":"ToBeB"}));
    assert_eq!(a["verdict"], "violated");
    assert_eq!(a["bounds"]["truncated"], false);
    h::subset(
        &json!({"terminals":[{"end":"posted","elapsed":{"min":3,"max":3},"work":{"min":3,"max":3}}]}),
        &a,
    );
}

#[test]
fn public_ir_non_fixed_initials_and_wrong_owners_are_rejected_independently() {
    let m = h::lowered(&h::source("rule-agency"), &json!({"command":"check"}));
    let index = m
        .state_variables
        .iter()
        .position(|s| s.name == "ApprovalRule.approval_required" || s.name == "approval_required")
        .unwrap();
    let value = &m.state_variables[index];
    assert!(value.kind.is_observed());
    assert!(!value.kind.is_latent());
    assert!(
        !matches!(
            value.kind,
            relca_core::ir::StateKind::ActorObserved(_) | relca_core::ir::StateKind::ActorLatent(_)
        ),
        "Rule is not mutable state or latent"
    );
    let owner = m
        .actors
        .iter()
        .position(|a| a.name == "ApprovalRule")
        .unwrap();
    assert_eq!(value.kind.actor(), Some(relca_core::ir::ActorId(owner)));
    assert!(!value.optional);
    assert_eq!(value.integer_max, None);
    assert_eq!(value.case_set_size, None);
    let relca_core::ir::InitialValue::Fixed(fixed) = value.initial else {
        panic!("lowered Rule control must be Fixed")
    };
    for initial in [
        relca_core::ir::InitialValue::Fixed(relca_core::ir::StateValue::None),
        relca_core::ir::InitialValue::Nondet,
        relca_core::ir::InitialValue::Distribution(0),
    ] {
        let mut bad = m.clone();
        if matches!(initial, relca_core::ir::InitialValue::Distribution(_)) {
            // A well-formed normalized point mass: rejection must be because
            // Rule is non-Fixed, not because a distribution index is invalid.
            bad.initial_distributions
                .push(vec![(fixed, relca_core::probability::Rational::one())]);
        }
        bad.state_variables[index].initial = initial;
        let e = engine::explore(&bad);
        assert!(e.diagnostics.iter().any(|d| d.code == "U-SEM-0"));
        assert!(e.terminals.is_empty());
        let r = reference::explore(&bad);
        assert_eq!(r.stop, reference::StopReason::SemanticAbort);
    }
}

fn assert_bad_ir(m: &relca_core::ir::Model, label: &str) {
    let e = engine::explore(m);
    assert!(
        e.diagnostics.iter().any(|d| d.code == "U-SEM-0"),
        "engine must reject {label}: {e:?}"
    );
    assert!(
        e.terminals.is_empty(),
        "engine must not return successful numeric results {label}"
    );
    let r = reference::explore(m);
    assert_eq!(
        r.stop,
        reference::StopReason::SemanticAbort,
        "reference independently rejects {label}"
    );
}

#[test]
fn public_ir_start_finish_atomic_and_kernel_writes_cannot_change_rule() {
    use relca_core::ir::*;
    // A same-typed enum Rule bit and scratch, so a type mismatch is not the
    // reason these corruptions fail. The uninvoked Capability must be checked.
    let m = h::lowered(&h::source("rule-write-atomic"), &json!({"command":"check"}));
    let index = m
        .state_variables
        .iter()
        .position(|s| s.name.ends_with(".bit") || s.name == "bit")
        .unwrap();
    let cap = m
        .capabilities
        .iter()
        .position(|c| c.name == "probe")
        .unwrap();
    let mut write = m.capabilities[cap].atomic.as_ref().unwrap().writes[0];
    write.variable = StateVarId(index);
    for phase in ["start", "finish", "atomic", "kernel_writes"] {
        let mut b = m.clone();
        match phase {
            "start" => b.capabilities[cap].start_writes.push(write),
            "finish" => b.capabilities[cap].writes.push(write),
            "atomic" => b.capabilities[cap].atomic.as_mut().unwrap().writes[0] = write,
            _ => b.capabilities[cap].kernel_writes.push(StateVarId(index)),
        }
        assert_bad_ir(&b, phase);
    }
    let mut mismatch = m.clone();
    mismatch.state_variables[index].kind = StateKind::ActorObserved(ActorId(usize::MAX));
    assert_bad_ir(&mismatch, "Rule metadata / StateKind / owner mismatch");
    let mut wrong = m.clone();
    wrong.state_variables[index].initial =
        InitialValue::Fixed(StateValue::Value(ValueId(usize::MAX)));
    assert_bad_ir(&wrong, "out of domain Fixed");
    let mut optional = m.clone();
    optional.state_variables[index].optional = true;
    assert_bad_ir(&optional, "optional Rule");
    let kernel = h::lowered(&h::source("rule-kernel-with"), &json!({"command":"check"}));
    let i = kernel
        .state_variables
        .iter()
        .position(|s| s.name.ends_with("approval_required"))
        .unwrap();
    let c = kernel
        .capabilities
        .iter()
        .position(|c| c.name == "probe")
        .unwrap();
    for site in ["given", "kernel_reads", "override"] {
        let mut b = kernel.clone();
        match site {
            "given" => b.capabilities[c]
                .kernel
                .as_mut()
                .unwrap()
                .given
                .push(StateVarId(i)),
            "kernel_reads" => b.capabilities[c].kernel_reads_latent.push(StateVarId(i)),
            _ => b.capabilities[c].kernel.as_mut().unwrap().rows[0].branches[0]
                .overrides
                .push((StateVarId(i), StateValue::Value(ValueId(0)))),
        }
        assert_bad_ir(&b, site);
    }
}

#[test]
fn all_configuration_authority_order_spans_and_cut_error_order_are_fixed() {
    for (n, token) in [
        ("rule-not-by.broken-t04", "a1 under RuleChange"),
        (
            "rule-self-authorize.broken-t05",
            "RuleChange2 { approval_required",
        ),
        ("rule-from.broken-s07", "\"r0\" to"),
        ("rule-same-version.broken-s08", "\"r1\" by"),
    ] {
        let src = h::source(n);
        for selector in ["AsIs", "ToBeB"] {
            let r = h::actual(&src, &json!({"command":"check","configuration":selector}));
            assert_eq!(r["verdict"], "invalid");
            let ds: Vec<_> = r["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|d| d["severity"] == "error")
                .collect();
            assert_eq!(ds.len(), 1);
            assert_eq!(ds[0]["code"], "E-CON-0");
            let span = ds[0]["span"]["start"].as_u64().unwrap() as usize;
            assert_eq!(
                span,
                src.find(token).unwrap(),
                "authority error points at first failed step {n}"
            );
        }
    }
    let src = h::source("rule-cut-both.broken-t09d");
    let r = h::actual(&src, &json!({"command":"check","configuration":"AsIs"}));
    let ds: Vec<_> = r["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["code"] == "E-CUT-1")
        .collect();
    assert_eq!(ds.len(), 2);
    assert_eq!(
        ds[0]["message"],
        "condition reads an Actor excluded by the cut"
    );
    assert_eq!(
        ds[1]["message"],
        "amendment targets an Actor excluded by the cut"
    );
    for (d, fragment) in ds
        .iter()
        .zip(["ApprovalRule.approval_required", "ApprovalRule from"])
    {
        assert_eq!(d["span"]["start"], json!(src.find(fragment).unwrap()));
    }
    // Order controls are not counted as single-fault fixture pairs: multiple
    // deliberately conflicting preconditions isolate first-error precedence.
    let from = h::replace(
        &h::source("rule-not-by.broken-t04"),
        "from \"r1\"",
        "from \"wrong\"",
    );
    let r = h::actual(&from, &json!({"command":"check","configuration":"AsIs"}));
    let ds: Vec<_> = r["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["severity"] == "error")
        .collect();
    assert_eq!(ds.len(), 1);
    assert_eq!(
        ds[0]["message"],
        "amendment must start from the declared rule version"
    );
    let grant = h::replace(
        &h::source("rule-self-authorize.broken-t05"),
        "grants a1 to change ApprovalRule",
        "grants cfo to change ApprovalRule",
    );
    let r = h::actual(&grant, &json!({"command":"check","configuration":"AsIs"}));
    let ds: Vec<_> = r["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["severity"] == "error")
        .collect();
    assert_eq!(ds.len(), 1);
    assert_eq!(
        ds[0]["message"],
        "Authority does not grant change of the rule to the amender"
    );
}

#[test]
fn unused_change_grants_and_prior_binding_are_not_enactment_or_runtime_authority() {
    let src = h::source("rule-prior-version");
    for selector in ["AsIs", "ToBeB"] {
        let v = h::actual(&src, &json!({"command":"check","configuration":selector}));
        let rule = &h::actor(&v, "ApprovalRule")["rule"];
        if selector == "AsIs" {
            assert_eq!(rule["amendment"], Value::Null);
            assert!(v["assumptions"].as_array().unwrap().iter().any(|a|a==&json!({"field":"authority.RuleChange.change.ApprovalRule.cfo","value":"unused_in_selected_configuration","affects":"declared rule-change grant is not used by the selected configuration; no enactment or exercise is inferred"})));
        } else {
            assert_eq!(rule["amendment"]["under_version"], "r1");
            assert!(
                !v["assumptions"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|a| a["field"] == "authority.RuleChange.change.ApprovalRule.cfo")
            );
        }
    }
}

#[test]
fn revocable_by_alone_is_a_record_not_a_restricted_rule_authority() {
    let src = h::replace(
        &h::source("rule-basic"),
        "not_by [a1]",
        "not_by [a1]; revocable_by cfo",
    );
    for (selector, verdict) in [("AsIs", "satisfied"), ("ToBeB", "violated")] {
        let r = h::actual(&src, &json!({"command":"check","configuration":selector}));
        assert_eq!(r["verdict"], verdict);
        assert_eq!(h::count(&r, "U-SEM-0"), 0);
        assert!(
            r["assumptions"]
                .as_array()
                .unwrap()
                .iter()
                .any(|a| a["field"].as_str().unwrap().contains("revocable_by"))
        );
    }
}

#[test]
fn self_issued_warning_and_enables_warning_are_positive_not_self_authorization_errors() {
    for (name, expected) in [("rule-self-issued", 1), ("rule-self-issued.broken-t15", 0)] {
        let src = h::source(name);
        for selector in ["AsIs", "ToBeB"] {
            let v = h::actual(&src, &json!({"command":"check","configuration":selector}));
            assert_eq!(
                v["verdict"],
                if selector == "AsIs" {
                    "satisfied"
                } else {
                    "violated"
                }
            );
            assert_eq!(h::count(&v, "W-AUTH-0"), expected);
            assert_eq!(h::count(&v, "E-CON-0"), 0);
        }
    }
    let src = h::source("rule-enables");
    let v = h::actual(&src, &json!({"command":"check","configuration":"ToBeB"}));
    let ds: Vec<_> = v["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["code"] == "W-AUTH-0")
        .collect();
    assert_eq!(ds.len(), 1);
    assert_eq!(
        ds[0]["message"],
        "amendment of ApprovalRule enables Authority Future granting to amender cfo"
    );
    let a = src.find("amend ApprovalRule").unwrap();
    let b = src[a..].find('}').unwrap() + a + 1;
    assert_eq!(ds[0]["span"], json!({"start":a,"end":b}));
    assert_eq!(h::count(&v, "E-CON-0"), 0);
    assert!(
        v["assumptions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a["field"] == "authority.Future.change.ApprovalRule.cfo"
                && a["value"] == "unused_in_selected_configuration")
    );
}

#[test]
fn read_by_is_lexical_requires_then_if_preorder_not_dynamic_or_analysis_reads() {
    let src = h::source("rule-agency");
    let src = h::replace(
        &src,
        "if ack == committed { end posted } else { end held }",
        "if ack == committed { if ApprovalRule.approval_required == true { end posted } else { end posted } } else { if ApprovalRule.approval_required == true { end held } else { end held } }",
    );
    let r = h::actual(&src, &json!({"command":"check"}));
    assert_eq!(r["verdict"], "satisfied");
    assert_eq!(
        h::actor(&r, "ApprovalRule")["rule"]["read_by"],
        json!(["Ledger.post.requires", "Flow.if#2", "Flow.if#3"])
    );
    let invariant = h::replace(
        &h::source("rule-agency"),
        "contract K { invariant Ledger.post.commit -> approval == pass }",
        "contract K { invariant Ledger.post.commit -> ApprovalRule.approval_required == true }",
    );
    let r = h::actual(&invariant, &json!({"command":"check"}));
    assert_eq!(
        h::actor(&r, "ApprovalRule")["rule"]["read_by"],
        json!(["Ledger.post.requires"])
    );
}

#[test]
fn rule_provenance_uses_pre_guard_paths_and_does_not_constant_fold_mutable_state() {
    let src = h::source("rule-agency");
    let false_guard = h::replace(
        &src,
        "ApprovalRule.approval_required == false || approval == pass",
        "ApprovalRule.approval_required == false && approval == pass",
    );
    let r = h::actual(&false_guard, &json!({"command":"check"}));
    assert_eq!(r["verdict"], "vacuous");
    assert_eq!(r["agency"]["refusal"][0]["reachable"], false);
    assert_eq!(
        r["agency"]["refusal"][0]["judgements"],
        json!([h::rule_item(
            "ApprovalRule",
            "r1",
            &["ApprovalRule.approval_required"]
        )])
    );
    let tautology = h::replace(
        &src,
        "ApprovalRule.approval_required == false || approval == pass",
        "ApprovalRule.approval_required == true || ApprovalRule.approval_required == false || approval == pass",
    );
    let r = h::actual(&tautology, &json!({"command":"check"}));
    assert_eq!(r["verdict"], "violated");
    assert_eq!(r["agency"]["refusal"], json!([]));
    let mixed = h::replace(
        &src,
        "actor ApprovalRule {",
        "actor ApprovalRule { state flag: bool = true;",
    );
    let mixed = h::replace(
        &mixed,
        "ApprovalRule.approval_required == false || approval == pass",
        "ApprovalRule.approval_required == true && ApprovalRule.flag == true && approval == pass",
    );
    let r = h::actual(&mixed, &json!({"command":"check"}));
    assert_eq!(r["verdict"], "satisfied");
    assert!(
        r["agency"]["unchecked"]
            .as_array()
            .unwrap()
            .contains(&json!("state_guards")),
        "Rule opt-in does not make ordinary ActorObserved constant"
    );
}

#[test]
fn unused_or_analysis_only_rule_reads_do_not_change_agency_rules_version() {
    let no_business = h::replace(
        &h::source("rule-agency"),
        "ApprovalRule.approval_required == false || approval == pass",
        "approval == pass",
    );
    let invariant = h::replace(
        &no_business,
        "invariant Ledger.post.commit -> approval == pass",
        "invariant Ledger.post.commit -> ApprovalRule.approval_required == true",
    );
    let r = h::actual(&invariant, &json!({"command":"check"}));
    assert_eq!(r["verdict"], "satisfied");
    assert_eq!(r["agency"]["rules"], "agency-p1-requires-1");
    assert_eq!(h::actor(&r, "ApprovalRule")["rule"]["read_by"], json!([]));
    let unused = h::replace(
        &no_business,
        "actor ApprovalRule {",
        "actor ApprovalRule { capacity 1; capability unused() -> V { requires ApprovalRule.approval_required == true else fail; duration fixed 1min; result nondet { pass } };",
    );
    let r = h::actual(&unused, &json!({"command":"check"}));
    assert_eq!(r["verdict"], "satisfied");
    assert_eq!(r["agency"]["rules"], "agency-p1-requires-1");
    assert_eq!(
        h::actor(&r, "ApprovalRule")["rule"]["read_by"],
        json!(["ApprovalRule.unused.requires"])
    );
}

/// Boundary review M1 (ADR0044 ruling after review). A refusal row may be
/// omitted only when the effect's own requires reads a Rule and is made a
/// tautology by the selected Rule values. A Rule-only path `if` that is
/// constantly true does not make a requires that ignores the Rule unrefusable.
/// Hand oracle: approve 0..2; pass -> post 2..3 posted (elapsed/work 3);
/// fail -> post refused at 2 before Start, held (elapsed/work 2). The else
/// block (W-UNREACH-1) and its end skipped (W-UNREACH-2) are unreachable
/// because approval_required is fixed true. Expected row equals the control
/// without the `if`: Ledger.post/ack reachable, judgements [] (approve takes no
/// argument, so it is not a process judgement; the requires reads no Rule).
#[test]
fn rule_only_path_if_does_not_omit_refusal_row_of_requires_that_ignores_rule() {
    let control = h::replace(
        &h::source("rule-agency"),
        "ApprovalRule.approval_required == false || approval == pass",
        "approval == pass",
    );
    let probe = h::replace(
        &control,
        "process Flow { a = Approver.approve(); ack = Ledger.post(); if ack == committed { end posted } else { end held } }",
        "process Flow { a = Approver.approve(); if ApprovalRule.approval_required == true { ack = Ledger.post(); if ack == committed { end posted } else { end held } } else { end skipped } }",
    );
    let row = json!([{"effect":"Ledger.post","call":"ack","reachable":true,"judgements":[]}]);
    let c = h::actual(&control, &json!({"command":"check"}));
    assert_eq!(c["verdict"], "satisfied");
    assert_eq!(c["agency"]["refusal"], row, "control");
    let r = h::actual(&probe, &json!({"command":"check"}));
    assert_eq!(r["verdict"], "satisfied");
    assert_eq!(r["agency"]["rules"], "agency-p1-rule-1");
    assert_eq!(r["agency"]["refusal"], row, "probe keeps the control row");
    assert_eq!(r["agency"]["refusal"], c["agency"]["refusal"]);
    for (end, t) in [("posted", 3), ("held", 2)] {
        let term = r["terminals"]
            .as_array()
            .unwrap()
            .iter()
            .find(|x| x["end"] == end)
            .unwrap();
        assert_eq!(term["elapsed"], json!({"min":t,"max":t}), "{end}");
        assert_eq!(term["work"], json!({"min":t,"max":t}), "{end}");
    }
    assert_eq!(h::count(&r, "W-UNREACH-1"), 1, "{}", r["diagnostics"]);
    assert_eq!(h::count(&r, "W-UNREACH-2"), 1, "{}", r["diagnostics"]);
}

/// Boundary review m3. The lexer has no single `!=` token, but the language
/// has the `!=` operator as adjacent `!` `=` (fixtures/int-ne: `n!=2`). A
/// decides default or an amend value that continues with adjacent `!=` is a
/// conditional rule value like `==`/`&&`/`||`/`.`: U-SYN-0 "conditional rule
/// value" (unsupported, exit 4), one error, not E-SYN-0 and not the generic
/// `amend` U-SYN-0.
#[test]
fn not_equal_rule_values_are_unsupported_conditional_rule_values() {
    let src = h::source("rule-basic");
    for bad in [
        h::replace(
            &src,
            "decides approval_required: bool = true }",
            "decides approval_required: bool = true != false }",
        ),
        h::replace(
            &src,
            "{ approval_required = false }",
            "{ approval_required = false != true }",
        ),
    ] {
        let ds = syntax::parse(&bad).unwrap_err();
        assert_eq!(ds.len(), 1, "{ds:?}");
        assert_eq!(ds[0].code, "U-SYN-0", "{ds:?}");
        assert_eq!(
            ds[0].message, "unsupported syntax: conditional rule value",
            "{ds:?}"
        );
        let r = h::actual(&bad, &json!({"command":"check"}));
        assert_eq!(r["verdict"], "unsupported");
        assert_eq!(h::count(&r, "U-SYN-0"), 1);
        assert_eq!(h::count(&r, "E-SYN-0"), 0);
    }
}

/// Boundary review m4. lower accepts a responsibility state condition that
/// reads a Rule value; its target names the Rule value qualified by the Rule
/// Actor, the same form as explain/trace/replay (`ApprovalRule.approval_required`).
/// The case variable `approval` stays unqualified as before.
#[test]
fn responsibility_target_qualifies_rule_values_like_explain() {
    for (formula, value) in [
        (
            "(ApprovalRule.approval_required == true)",
            "ApprovalRule.approval_required == true",
        ),
        (
            "(ApprovalRule.approval_required == true && approval == pass)",
            "(ApprovalRule.approval_required == true && approval == pass)",
        ),
        (
            "(ApprovalRule.approval_required == true -> approval == pass)",
            "(ApprovalRule.approval_required == true -> approval == pass)",
        ),
    ] {
        let src = h::replace(
            &h::source("rule-agency"),
            "verify safety no_resource_overcommit",
            &format!("responsible a1 for {formula}\nverify safety no_resource_overcommit"),
        );
        let r = h::actual(&src, &json!({"command":"check"}));
        assert_eq!(r["verdict"], "satisfied", "{formula}");
        assert_eq!(
            r["responsibilities"][0]["target"],
            json!({"kind":"state_condition","value":value}),
            "{formula}"
        );
    }
}

// Acceptance added for the mutation survivors of ADR0044 (mutlog-a6, ruling of
// the boundary review). Each input is derived in the test from a closed-
// population fixture; the closed population itself is not extended. Every
// expectation below was written by hand from the declared semantics before any
// run; columns that could not be derived by hand are deliberately not asserted.

fn errors(v: &Value) -> Vec<&Value> {
    v["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["severity"] == "error")
        .collect()
}

/// F12c. A `rule` intent target resolves only to an Actor that declares
/// `decides`. Approver and Ledger are Actors without `decides`, so both a
/// `change rule` claim and a `preserve ... on rule` target are E-CON-0 at the
/// target (the name is an Actor, so not E-NAME-0). Intent names are checked for
/// every configuration, so the AsIs selection reports the ToBeB intent too.
#[test]
fn rule_intent_target_must_declare_decides_not_just_name_an_actor() {
    let src = h::source("rule-basic");
    let claim = h::replace(
        &src,
        "change rule ApprovalRule reason",
        "change rule Approver reason",
    );
    let preserve = h::replace(
        &src,
        "intent { change rule ApprovalRule reason \"exempt approval\" }",
        "intent { preserve K on rule Ledger }",
    );
    for (bad, token, len) in [(&claim, "Approver reason", 8), (&preserve, "Ledger }", 6)] {
        for selector in ["AsIs", "ToBeB"] {
            let r = h::actual(bad, &json!({"command":"check","configuration":selector}));
            assert_eq!(r["verdict"], "invalid", "{token}/{selector}: {r}");
            let ds = errors(&r);
            assert_eq!(ds.len(), 1, "{token}/{selector}: {ds:?}");
            assert_eq!(ds[0]["code"], "E-CON-0");
            assert_eq!(
                ds[0]["message"],
                "undeclared intent target or wrong difference namespace"
            );
            let start = bad.find(token).unwrap();
            assert_eq!(
                ds[0]["span"],
                json!({"start":start,"end":start + len}),
                "{token}/{selector}"
            );
        }
    }
}

/// X05. The first failed step of the amendment checks is reported: `from` is
/// checked before `to != from`. `from "r0" to "r0"` fails the first step (the
/// declared version is r1), so the single error is the from error at the first
/// `"r0"`, not the same-version error at the second.
#[test]
fn amendment_from_check_precedes_same_version_check() {
    let src = h::replace(
        &h::source("rule-basic"),
        "from \"r1\" to \"r2\"",
        "from \"r0\" to \"r0\"",
    );
    for selector in ["AsIs", "ToBeB"] {
        let r = h::actual(&src, &json!({"command":"check","configuration":selector}));
        assert_eq!(r["verdict"], "invalid");
        let ds = errors(&r);
        assert_eq!(ds.len(), 1, "{selector}: {ds:?}");
        assert_eq!(ds[0]["code"], "E-CON-0");
        assert_eq!(
            ds[0]["message"],
            "amendment must start from the declared rule version"
        );
        assert_eq!(
            ds[0]["span"]["start"],
            json!(src.find("\"r0\" to \"r0\"").unwrap()),
            "{selector}"
        );
    }
}

/// X06. A change grant is for one Rule. RuleChange grants cfo the change of
/// OtherRule only, so the amendment of ApprovalRule by cfo under RuleChange is
/// not authorized: E-CON-0 at the `under` Authority, in every configuration.
#[test]
fn change_grant_for_another_rule_does_not_authorize_the_amendment() {
    let src = h::replace(
        &h::source("rule-basic"),
        "actor Approver {",
        "actor OtherRule { control policy; version \"r1\"; decides x: bool = true }\nactor Approver {",
    );
    let src = h::replace(
        &src,
        "grants cfo to change ApprovalRule;",
        "grants cfo to change OtherRule;",
    );
    let src = h::replace(
        &src,
        "include [ApprovalRule, Approver, Ledger]",
        "include [ApprovalRule, OtherRule, Approver, Ledger]",
    );
    for selector in ["AsIs", "ToBeB"] {
        let r = h::actual(&src, &json!({"command":"check","configuration":selector}));
        assert_eq!(r["verdict"], "invalid", "{selector}: {r}");
        let ds = errors(&r);
        assert_eq!(ds.len(), 1, "{selector}: {ds:?}");
        assert_eq!(ds[0]["code"], "E-CON-0");
        assert_eq!(
            ds[0]["message"],
            "Authority does not grant change of the rule to the amender"
        );
        assert_eq!(
            ds[0]["span"]["start"],
            json!(src.find("RuleChange { approval_required").unwrap()),
            "{selector}"
        );
    }
}

/// X10. An unused change grant is reported per person. RuleChange grants cfo
/// and a2; ToBeB amends by cfo, so only a2's grant is unused there. AsIs uses
/// neither. ToBeB makes approval optional, so K is violated; AsIs satisfied.
#[test]
fn unused_change_grant_rows_are_per_person_not_per_authority() {
    let src = h::replace(
        &h::source("rule-basic"),
        "people [a1, cfo]",
        "people [a1, cfo, a2]",
    );
    let src = h::replace(
        &src,
        "grants cfo to change ApprovalRule",
        "grants [cfo, a2] to change ApprovalRule",
    );
    let affects = "declared rule-change grant is not used by the selected configuration; no enactment or exercise is inferred";
    for (selector, verdict, people) in [
        ("AsIs", "satisfied", vec!["cfo", "a2"]),
        ("ToBeB", "violated", vec!["a2"]),
    ] {
        let r = h::actual(&src, &json!({"command":"check","configuration":selector}));
        assert_eq!(r["verdict"], verdict, "{selector}");
        let rows: Vec<_> = r["assumptions"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|a| {
                a["field"]
                    .as_str()
                    .unwrap()
                    .starts_with("authority.RuleChange.change.ApprovalRule.")
            })
            .collect();
        assert_eq!(rows.len(), people.len(), "{selector}: {rows:?}");
        for p in people {
            assert!(
                rows.contains(&&json!({"field":format!("authority.RuleChange.change.ApprovalRule.{p}"),"value":"unused_in_selected_configuration","affects":affects})),
                "{selector}/{p}: {rows:?}"
            );
        }
    }
}

/// X19. The Rule judgement names the effective version. A version-only
/// amendment (r1 -> r2, approval_required stays true) keeps the refusal row of
/// Ledger.post (requires `false || approval == pass` is not a tautology), and
/// its judgement carries r2 in ToBeB, r1 in AsIs. Satisfied: fail is refused.
#[test]
fn rule_judgement_version_is_effective_after_version_only_amendment() {
    let src = h::replace(
        &h::source("rule-basic"),
        "under RuleChange { approval_required = false }",
        "under RuleChange { approval_required = true }",
    );
    let row = |version: &str| json!([{"effect":"Ledger.post","call":"ack","reachable":true,"judgements":[h::rule_item("ApprovalRule",version,&["ApprovalRule.approval_required"])]}]);
    let r = h::actual(&src, &json!({"command":"check","configuration":"ToBeB"}));
    assert_eq!(r["verdict"], "satisfied");
    assert_eq!(r["agency"]["refusal"], row("r2"));
    let d = h::actual(&src, &json!({"command":"diff","from":"AsIs","to":"ToBeB"}));
    assert_eq!(d["from"]["report"]["agency"]["refusal"], row("r1"));
    assert_eq!(d["to"]["report"]["agency"]["refusal"], row("r2"));
}

/// X27. diff --from ToBeB --to AsIs reverses an amendment that AsIs does not
/// declare: one E-CON-0 "rule version change has no declared amendment". The
/// to-side (AsIs) intent on that Rule is not matched again, so no E-DIFF-0 is
/// stacked on the same Rule; the single error is the E-CON-0.
#[test]
fn intent_on_undeclared_reverse_rule_change_adds_no_intent_error() {
    let src = h::replace(
        &h::source("rule-reverse.broken-t11"),
        "configuration AsIs { cut C; process Flow }",
        "configuration AsIs { cut C; process Flow\n  intent { change rule ApprovalRule reason \"revert\" }\n}",
    );
    let r = h::actual(&src, &json!({"command":"diff","from":"ToBeB","to":"AsIs"}));
    assert_eq!(r["verdict"], "invalid");
    let ds = errors(&r);
    assert_eq!(ds.len(), 1, "{ds:?}");
    assert_eq!(ds[0]["code"], "E-CON-0");
    assert_eq!(
        ds[0]["message"],
        "rule version change has no declared amendment"
    );
    assert_eq!(h::count(&r, "E-DIFF-0"), 0);
}

/// F31b. A process `if` that mixes a Rule leaf with a non-Rule state leaf
/// (`approval`) stays an unknown state guard: the Rule does not unlock
/// case-writer substitution for the neighbouring leaf. The effect under (Or) or
/// after (And) that guard gets no refusal row and `state_guards` is unchecked.
/// Hand runtime (AsIs, approval_required true): pass -> post 2..3 posted
/// (elapsed/work 3); fail -> skipped at 2 (elapsed/work 2). K holds.
/// Framing columns are not asserted (not derived by hand).
#[test]
fn process_rule_guard_keeps_neighbouring_state_leaf_unknown() {
    let src = h::source("rule-basic");
    let base = "process Flow { a = Approver.approve(); ack = Ledger.post(); if ack == committed { end posted } else { end held } }";
    for (label, flow) in [
        (
            "or",
            "process Flow { a = Approver.approve(); if ApprovalRule.approval_required == false || approval == pass { ack = Ledger.post(); if ack == committed { end posted } else { end held } } else { end skipped } }",
        ),
        (
            "and",
            "process Flow { a = Approver.approve(); if ApprovalRule.approval_required == true && approval == fail { end skipped } else { ack = Ledger.post(); if ack == committed { end posted } else { end held } } }",
        ),
    ] {
        let probe = h::replace(&src, base, flow);
        let r = h::actual(&probe, &json!({"command":"check","configuration":"AsIs"}));
        assert_eq!(r["verdict"], "satisfied", "{label}");
        assert_eq!(r["agency"]["rules"], "agency-p1-rule-1", "{label}");
        assert_eq!(r["agency"]["refusal"], json!([]), "{label}");
        assert!(
            r["agency"]["unchecked"]
                .as_array()
                .unwrap()
                .contains(&json!("state_guards")),
            "{label}: {}",
            r["agency"]
        );
        for (end, t) in [("posted", 3), ("skipped", 2)] {
            let term = r["terminals"]
                .as_array()
                .unwrap()
                .iter()
                .find(|x| x["end"] == end)
                .unwrap_or_else(|| panic!("{label}/{end}: {}", r["terminals"]));
            assert_eq!(term["elapsed"], json!({"min":t,"max":t}), "{label}/{end}");
            assert_eq!(term["work"], json!({"min":t,"max":t}), "{label}/{end}");
        }
    }
}

fn explain_counterexample<'a>(v: &'a Value, id: &str) -> &'a Value {
    assert_eq!(
        v["explanation"]["status"], "complete",
        "{}",
        v["explanation"]
    );
    let cs = v["explanation"]["counterexamples"].as_array().unwrap();
    assert_eq!(cs.len(), 1, "{cs:?}");
    let c = &cs[0];
    assert_eq!(c["id"], id);
    assert_eq!(c["status"], "verified", "{c}");
    assert_eq!(c["replay"]["status"], "verified", "{c}");
    c
}

/// X12d / X12f. A state invariant that reads a Rule value: rule-explain fixes
/// approval_required false, so the antecedent holds in every reachable state
/// and the consequent `approval == pass` is false right after receipt
/// (approval is none). S is violated; the engine trace and the independent
/// replay must agree on the qualified name of the Rule value in the violation
/// values (`ApprovalRule.approval_required` = false). Which violating state is
/// shown, and the value of `approval` there, are not asserted.
#[test]
fn state_invariant_violation_values_qualify_rule_value_in_trace_and_replay() {
    let src = h::replace(
        &h::source("rule-explain"),
        "contract K { invariant Ledger.post.commit -> approval == pass }",
        "contract S { invariant ApprovalRule.approval_required == false -> approval == pass }",
    );
    let v = h::actual(&src, &json!({"command":"explain"}));
    assert_eq!(v["verdict"], "violated");
    let c = explain_counterexample(&v, "contract S #1 invariant");
    let rows: Vec<_> = c["steps"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|s| s["violation"]["state_invariant"]["values"].as_array())
        .flatten()
        .collect();
    assert!(
        rows.iter()
            .any(|x| x["name"] == "ApprovalRule.approval_required" && x["value"] == "false"),
        "{rows:?}"
    );
    assert!(
        rows.iter().all(|x| x["name"] != "approval_required"),
        "{rows:?}"
    );
}

/// X12e. With a nondeterministic latent initial value (Invoice.difficulty has
/// no default), the counterexample starts with an initialize step whose values
/// include the Rule value under its qualified name; the independent replay
/// must check that step. K is violated as in rule-explain (fail commits at 3).
#[test]
fn initialize_step_qualifies_rule_value_in_trace_and_replay() {
    let src = h::replace(
        &h::source("rule-explain"),
        "type Ack = enum { committed, held }",
        "type Ack = enum { committed, held }\ntype D = enum { routine, difficult }\nactor Invoice { latent difficulty: D }",
    );
    let src = h::replace(
        &src,
        "include [ApprovalRule, Approver, Ledger]",
        "include [ApprovalRule, Approver, Ledger, Invoice]",
    );
    let v = h::actual(&src, &json!({"command":"explain"}));
    assert_eq!(v["verdict"], "violated");
    let c = explain_counterexample(&v, "contract K #1 invariant");
    let init: Vec<_> = c["steps"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s["kind"] == "initialize")
        .collect();
    assert_eq!(init.len(), 1, "{}", c["steps"]);
    assert!(
        init[0]["values"]["observed"]
            .as_array()
            .unwrap()
            .iter()
            .any(|x| x["name"] == "ApprovalRule.approval_required" && x["value"] == "false"),
        "{}",
        init[0]
    );
}

fn rule_change(m: &relca_core::ir::Model) -> (usize, usize, usize, usize) {
    let person = |n: &str| m.people.iter().position(|p| p.name == n).unwrap();
    let rule = m
        .actors
        .iter()
        .position(|a| a.name == "ApprovalRule")
        .unwrap();
    let auth = m
        .authorities
        .iter()
        .position(|a| a.name == "RuleChange")
        .unwrap();
    (rule, auth, person("a1"), person("cfo"))
}

fn assert_good_ir(m: &relca_core::ir::Model, label: &str) {
    let e = engine::explore(m);
    assert!(
        !e.diagnostics.iter().any(|d| d.code == "U-SEM-0") && !e.terminals.is_empty(),
        "control {label}: {e:?}"
    );
    assert_eq!(
        reference::explore(m).stop,
        reference::StopReason::Complete,
        "control {label}"
    );
}

/// X13. A forged amendment is rejected by the engine and, independently, by
/// the reference: (a) the amender is in the Authority's not_by (even when also
/// granted), (b) the Authority grants the change to someone else only.
#[test]
fn public_ir_amendment_by_not_by_or_ungranted_person_is_rejected_independently() {
    use relca_core::ir::PersonId;
    let m = h::lowered(
        &h::source("rule-basic"),
        &json!({"command":"check","configuration":"ToBeB"}),
    );
    assert_good_ir(&m, "rule-basic ToBeB");
    let (rule, auth, a1, cfo) = rule_change(&m);
    let amendment = |b: &mut relca_core::ir::Model| {
        b.actors[rule]
            .metadata
            .rule
            .as_mut()
            .unwrap()
            .amendment
            .as_mut()
            .unwrap()
            .by = PersonId(a1)
    };
    assert_eq!(m.authorities[auth].not_by, vec![PersonId(a1)]);
    assert_eq!(
        m.authorities[auth].rule_grants[0].persons,
        vec![PersonId(cfo)]
    );
    let mut not_by = m.clone();
    amendment(&mut not_by);
    not_by.authorities[auth].rule_grants[0]
        .persons
        .push(PersonId(a1));
    assert_bad_ir(&not_by, "amender in not_by although granted");
    let mut ungranted = m.clone();
    ungranted.authorities[auth].not_by.clear();
    ungranted.authorities[auth].rule_grants[0].persons = vec![PersonId(a1)];
    assert_bad_ir(&ungranted, "amender cfo not granted");
}

/// X14 / X28. An Authority bound `under` a prior Rule version is valid only
/// for that Rule's declared version. Forging the bound version to r2 (the
/// effective, not the declared, version) is rejected by both interpreters.
#[test]
fn public_ir_under_binding_must_name_declared_rule_version() {
    let m = h::lowered(
        &h::source("rule-prior-version"),
        &json!({"command":"check","configuration":"ToBeB"}),
    );
    assert_good_ir(&m, "rule-prior-version ToBeB");
    let (_, auth, _, _) = rule_change(&m);
    let mut b = m.clone();
    let under = b.authorities[auth].under.as_mut().unwrap();
    assert_eq!(under.version, "r1");
    under.version = "r2".into();
    assert_bad_ir(&b, "under bound to a non-declared version");
}

/// X15 / X16. Without an amendment a Rule value starts at its declared value,
/// and every Rule-kind state variable is owned by a Rule's metadata. (a) a
/// well-typed Fixed initial that differs from the declared value, (b) an extra
/// Rule-kind variable that no RuleInfo lists, are each rejected by both.
#[test]
fn public_ir_undeclared_rule_initial_and_orphan_rule_variable_are_rejected_independently() {
    use relca_core::ir::*;
    let m = h::lowered(
        &h::source("rule-basic"),
        &json!({"command":"check","configuration":"AsIs"}),
    );
    assert_good_ir(&m, "rule-basic AsIs");
    let (rule, _, _, _) = rule_change(&m);
    let info = m.actors[rule].metadata.rule.as_ref().unwrap();
    assert!(info.amendment.is_none());
    let index = info.values[0].variable.0;
    let StateValue::Value(declared) = info.values[0].declared else {
        panic!("bool rule value")
    };
    assert_eq!(m.types[m.state_variables[index].base.0].members.len(), 2);
    let mut initial = m.clone();
    initial.state_variables[index].initial =
        InitialValue::Fixed(StateValue::Value(ValueId(1 - declared.0)));
    assert_bad_ir(&initial, "in-domain Fixed initial differing from declared");
    let mut orphan = m.clone();
    let mut v = m.state_variables[index].clone();
    v.name = "orphan".into();
    orphan.state_variables.push(v);
    assert_bad_ir(&orphan, "Rule variable not listed by any RuleInfo");
}
