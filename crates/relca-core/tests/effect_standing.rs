//! ADR0040 acceptance. All arithmetic precedes implementation; new targets intentionally RED.
#[path = "support/effect_standing.rs"]
mod hand;
use relca_core::{engine, lower, reference, report, syntax};
use serde_json::{Value, json};

fn check(src: &str) -> Value {
    serde_json::to_value(report::check(src)).unwrap()
}
fn model(src: &str) -> relca_core::ir::Model {
    let ast = syntax::parse(src).unwrap_or_else(|e| panic!("new commit syntax must parse: {e:?}"));
    lower::lower(&ast).unwrap_or_else(|e| panic!("legal effect model must lower: {e:?}"))
}

#[test]
fn all_pairs_are_exact_single_edits_and_closed() {
    let mut found = std::collections::BTreeSet::new();
    for p in hand::pairs().as_array().unwrap() {
        let good = p["good"].as_str().unwrap();
        let bad = p["bad"].as_str().unwrap();
        let src = hand::source(good);
        let old = p["old"].as_str().unwrap();
        let new = p["new"].as_str().unwrap();
        let changed = if p["all"] == true {
            let c = src.replace(old, new);
            assert_ne!(src, c);
            c
        } else {
            hand::replace(&src, old, new)
        };
        assert_eq!(changed, hand::source(bad), "{} {bad}", p["id"]);
        found.insert(good.to_owned());
        found.insert(bad.to_owned());
    }
    assert_eq!(found, hand::names().into_iter().collect());
    assert_eq!(found.len(), 86);
}

#[test]
fn old_end_duplicate_keeps_its_terminal_specific_diagnostic() {
    let old = hand::source("resp-contest");
    let src = hand::replace(
        &old,
        "standing over [end.returned]",
        "standing over [end.returned, end.returned]",
    );
    let a = check(&src);
    assert_eq!(a["verdict"], "unsupported");
    assert!(a["diagnostics"].as_array().unwrap().iter().any(|d| {
        d["code"] == "U-SEM-0"
            && d["message"]
                .as_str()
                .unwrap()
                .contains("duplicate Standing terminal")
    }));
}

#[test]
fn old_syntax_skeletons_are_legal_before_effect_acceptance() {
    for n in hand::names() {
        let mut src = hand::skeleton(&hand::source(&n));
        if src.contains("change contest Ledger.post.commit reason \"effect route\"") {
            src = hand::replace(
                &src,
                "change contest Ledger.post.commit reason \"effect route\"",
                "",
            );
        }
        assert!(!src.contains("standing over") && !src.contains("contract Controls"));
        let ast =
            syntax::parse(&src).unwrap_or_else(|ds| panic!("unrelated syntax error {n}: {ds:?}"));
        if let Some(inv) = hand::invocation(&n).filter(|v| v["command"] == "diff") {
            for key in ["from", "to"] {
                // New effect intent is a separate C4 syntax surface.
                lower::lower_configuration(&ast, inv[key].as_str().unwrap()).unwrap();
            }
        } else {
            lower::lower(&ast).unwrap_or_else(|ds| panic!("unrelated lowering error {n}: {ds:?}"));
        }
    }
}

#[test]
fn all_check_reports_match_hand_oracles_with_all_failures_collected() {
    let mut failures = vec![];
    for n in hand::names() {
        if hand::invocation(&n).is_some() {
            continue;
        }
        let actual = check(&hand::source(&n));
        if std::panic::catch_unwind(|| hand::subset(&hand::expected(&n), &actual, &n)).is_err() {
            failures.push(n);
        }
    }
    assert!(
        failures.is_empty(),
        "effect acceptance mismatches (all): {failures:?}"
    );
}

#[test]
fn independent_engine_reference_both_routes_match_business_and_contest_tables() {
    // P20 exception: existing reference semantics reject
    // invoked zero-duration Tasks. No other runtime input is excluded.
    const REFERENCE_EXCLUSIONS: &[(&str, &str)] = &[(
        "sov-end-open.broken-zero-tail",
        "existing zero-duration Task -> SemanticAbort",
    )];
    assert_eq!(REFERENCE_EXCLUSIONS.len(), 1);
    assert_eq!(
        hand::names()
            .iter()
            .filter(|n| REFERENCE_EXCLUSIONS.iter().any(|(id, _)| n == id))
            .count(),
        1
    );
    let mut failures = vec![];
    let mut complete = 0;
    let mut selections = 0;
    for n in hand::names() {
        if hand::invocation(&n).is_some() {
            continue;
        }
        let expected = hand::expected(&n);
        if matches!(
            expected["verdict"].as_str(),
            Some("unsupported" | "invalid")
        ) {
            continue;
        }
        if REFERENCE_EXCLUSIONS.iter().any(|(id, _)| *id == n) {
            continue;
        }
        let src = hand::source(&n);
        for arrivals in [false, true] {
            // Explicit two-case FIFO inputs already select the arrivals engine.
            if arrivals && src.contains("arrivals fixed") {
                continue;
            }
            let src = if arrivals {
                hand::replace(&src, "  bounds", "  arrivals fixed [0min]\n  bounds")
            } else {
                src.clone()
            };
            selections += 1;
            let result = std::panic::catch_unwind(|| {
                let m = model(&src);
                let e = engine::explore(&m);
                let mut r = reference::explore(&m);
                assert_eq!(
                    r.stop,
                    reference::StopReason::Complete,
                    "{n} arrivals={arrivals}"
                );
                assert!(r.outcome.diagnostics.starts_with(&m.diagnostics));
                r.outcome.diagnostics.drain(..m.diagnostics.len());
                assert_eq!(e, r.outcome, "all runtime fields {n} arrivals={arrivals}");
                // Reports, unlike explore Outcomes, include lower diagnostics.
                let report = check(&src);
                let mut exp = expected.clone();
                if arrivals {
                    let terms = exp
                        .as_object_mut()
                        .unwrap()
                        .remove("terminals")
                        .unwrap_or(json!([]));
                    assert_eq!(report["terminals"], json!([]));
                    for t in terms.as_array().unwrap() {
                        let actual = report["case_outcomes"][0]["terminals"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .find(|r| r["end"] == t["end"])
                            .unwrap();
                        hand::subset(t, actual, &n);
                    }
                }
                hand::subset(&exp, &report, &n);
            });
            if result.is_ok() {
                complete += 1;
            } else {
                failures.push(format!("{n}/arrivals={arrivals}"));
            }
        }
    }
    assert_eq!(
        selections, 130,
        "66 nonzero exhaustive inputs, each on both engines except the two already-two-case FIFO inputs"
    );
    assert!(
        failures.is_empty(),
        "Complete {complete}/{selections}; runtime failures: {failures:?}"
    );
}

#[test]
fn p20_closed_zero_time_reference_exception_has_equivalent_wait0_control() {
    let zero = hand::source("sov-end-open.broken-zero-tail");
    let waiting = hand::replace(&zero, "t = Tail.tail();", "wait 0min;");
    // Verify the inherited refusal even without the new effect feature.
    let skeleton = hand::skeleton(&zero);
    let old = lower::lower(&syntax::parse(&skeleton).unwrap()).unwrap();
    assert_eq!(
        reference::explore(&old).stop,
        reference::StopReason::SemanticAbort
    );
    let skeleton = hand::skeleton(&waiting);
    let old = lower::lower(&syntax::parse(&skeleton).unwrap()).unwrap();
    assert_eq!(
        reference::explore(&old).stop,
        reference::StopReason::Complete
    );
    for arrivals in [false, true] {
        let src = if arrivals {
            hand::replace(&waiting, "  bounds", "  arrivals fixed [0min]\n  bounds")
        } else {
            waiting.clone()
        };
        let m = model(&src);
        let e = engine::explore(&m);
        let mut r = reference::explore(&m);
        assert_eq!(r.stop, reference::StopReason::Complete);
        r.outcome.diagnostics.drain(..m.diagnostics.len());
        assert_eq!(e, r.outcome);
        let a = check(&src);
        assert_eq!(
            a["contest_summary"]["business_ended_open"],
            json!({"min":"0","max":"1","reason":null})
        );
        let rows = if arrivals {
            &a["case_outcomes"][0]["terminals"]
        } else {
            &a["terminals"]
        };
        let post = rows
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["end"] == "posted")
            .unwrap();
        assert_eq!(post["elapsed"], json!({"min":3,"max":3}));
        assert_eq!(post["work"], json!({"min":3,"max":3}));
    }
}

fn property(v: &Value, id: &str) -> String {
    v["properties"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == id)
        .unwrap()["verdict"]
        .as_str()
        .unwrap()
        .into()
}

#[test]
fn authority_candidates_and_reasons_are_derived_not_actor_labels() {
    for (n, candidates, person, reason) in [
        ("sov-basic", vec!["Approver.approve"], "a1", None),
        (
            "sov-basic.broken-no-authority",
            vec!["Approver.approve"],
            "a1",
            Some("Permission あり、Authority 未接続"),
        ),
        (
            "sov-self-candidate",
            vec!["Approver.approve", "Ledger.post"],
            "l1",
            None,
        ),
        (
            "sov-effective-single.broken-single-result",
            vec!["Approver.approve"],
            "l1",
            Some("executor_not_connected"),
        ),
        (
            "sov-kernel.broken-zero-held",
            vec!["Approver.approve"],
            "l1",
            Some("executor_not_connected"),
        ),
    ] {
        let a = check(&hand::source(n));
        let people = &a["contest_paths"][0]["checks"]["reversal_authority"]["people"];
        let p = people
            .as_array()
            .expect("static effect certificate")
            .iter()
            .find(|p| p["person"] == person)
            .unwrap();
        let caps: Vec<_> = p["decisions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["capability"].as_str().unwrap())
            .collect();
        assert_eq!(caps, candidates, "{n}");
        assert_eq!(p["reason"], json!(reason), "{n}");
    }
}

#[test]
fn silent_cut_uses_exact_labels_not_prefixes_reason_or_reentry() {
    for (name, count) in [
        ("sov-warning", 0),
        ("sov-warning.broken-silent", 1),
        ("sov-cut-exact", 0),
        ("sov-cut-exact.broken-prefix", 1),
    ] {
        let a = check(&hand::source(name));
        let ds: Vec<_> = a["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|d| d["code"] == "W-CONTEST-0")
            .collect();
        assert_eq!(ds.len(), count, "{name}");
        for d in ds {
            assert_eq!(
                d["message"],
                "Vendor/Ledger.post.commit has no contest route or explicit Cut"
            );
        }
    }
    let src = hand::source("sov-warning.broken-silent");
    for (old, new) in [
        (
            "include [Approver, Ledger, Clerk, Vendor]",
            "include [Approver, Ledger, Clerk]; exclude Vendor reason \"r\" loses \"l\"",
        ),
        (
            "include [Approver, Ledger, Clerk, Vendor]",
            "include [Approver, Ledger, Clerk, Vendor]; defer Vendor reason \"r\" loses \"l\"",
        ),
    ] {
        let a = check(&hand::replace(&src, old, new));
        let count = a["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|d| d["code"] == "W-CONTEST-0")
            .count();
        assert_eq!(count, usize::from(new.contains("defer Vendor reason")));
    }
}

#[test]
fn existing_responsibility_and_unreachable_warnings_keep_their_meanings() {
    let src = hand::source("sov-basic");
    let m = model(&src);
    let warnings: Vec<_> = m
        .diagnostics
        .iter()
        .filter(|d| d.code == "W-RESP-0")
        .map(|d| d.message.as_str())
        .collect();
    assert_eq!(warnings, ["Ledger has no declared responsibility"]);
    let v = check(&hand::source("sov-vacuous"));
    let warnings: std::collections::BTreeSet<_> = v["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["code"].as_str().unwrap().starts_with("W-UNREACH-"))
        .map(|d| (d["code"].as_str().unwrap(), d["message"].as_str().unwrap()))
        .collect();
    assert_eq!(
        warnings,
        std::collections::BTreeSet::from([
            ("W-UNREACH-0", "process Flow: Ledger.post never_started"),
            ("W-UNREACH-0", "process Flow: Clerk.recheck never_started"),
            ("W-UNREACH-1", "process Flow: if else arm is never taken"),
            ("W-UNREACH-2", "process Flow: end.posted is never reached"),
            ("W-UNREACH-3", "process Flow: executor c1 is never assigned"),
        ])
    );
    let v = check(&hand::source("sov-vacuous.broken-truncated"));
    assert!(
        !v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"].as_str().unwrap().starts_with("W-UNREACH-")),
        "cut cannot assert unreachable"
    );
}

#[test]
fn simulation_refuses_on_commit_with_structural_violated_dominance() {
    for (name, verdict, static_) in [
        ("sov-sim", "unsupported", "satisfied"),
        ("sov-sim.broken-no-authority", "violated", "violated"),
    ] {
        let a = serde_json::to_value(report::simulate(&hand::source(name))).unwrap();
        assert_eq!(a["verdict"], verdict);
        assert_eq!(property(&a, "contract Controls #1 contest"), static_);
        assert!(
            a["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["code"] == "U-SEM-0")
        );
    }
}

#[test]
fn malformed_public_ir_commits_and_route_intersection_abort_without_panic() {
    let m = model(&hand::source("sov-basic"));
    let post = m
        .capabilities
        .iter()
        .position(|c| c.name == "post")
        .unwrap();
    for kind in [0, 1] {
        let mut bad = m.clone();
        if kind == 0 {
            bad.capabilities[post].commits = false;
        } else {
            bad.responsibility.as_mut().unwrap().contests[0].route =
                relca_core::ir::CapabilityId(post);
        }
        let e = engine::explore(&bad);
        let r = reference::explore(&bad);
        assert!(e.diagnostics.iter().any(|d| d.code == "U-SEM-0"));
        assert_eq!(r.stop, reference::StopReason::SemanticAbort);
        assert!(e.terminals.is_empty());
    }
}

#[test]
fn lexical_site_route_intersection_and_duplicate_rules_also_cover_standing_only() {
    let base = hand::source("sov-basic");
    let no_effect = hand::replace(&base, "ack = Ledger.post(); end posted", "end posted");
    let q2 = hand::replace(&no_effect, "route Clerk.recheck", "route Ledger.post");
    let terminal_route = hand::replace(
        &base,
        "standing over [Ledger.post.commit]",
        "standing over [end.returned, Ledger.post.commit]",
    );
    let terminal_route = hand::replace(
        &terminal_route,
        "contest Vendor on Ledger.post.commit route Clerk.recheck",
        "contest Vendor on end.returned route Ledger.post",
    );
    let duplicate = hand::replace(
        &base,
        "basis \"posting claim\"",
        "basis \"posting claim\"; standing over [Ledger.post.commit] basis \"second\"",
    );
    let two_sites = hand::source("sov-one-site.broken-two-sites");
    let standing_only = two_sites
        .lines()
        .filter(|l| !l.trim_start().starts_with("contract Controls"))
        .collect::<Vec<_>>()
        .join("\n");
    for (s, message) in [
        (q2, "effect target used as contest route is unsupported"),
        (
            terminal_route,
            "effect target used as contest route is unsupported",
        ),
        (duplicate, "duplicate Standing effect"),
        (
            standing_only,
            "effect contest with multiple commit sites is unsupported",
        ),
    ] {
        let a = check(&s);
        assert_eq!(a["verdict"], "unsupported");
        assert!(a["diagnostics"].as_array().unwrap().iter().any(|d|d["code"]=="U-SEM-0"&&d["message"].as_str().unwrap().contains(message)),"{a}");
    }
    let a = check(&no_effect);
    assert_eq!(property(&a, "contract Controls #1 contest"), "violated");
    assert_eq!(
        property(&a, "contract Controls #1 contest_response"),
        "vacuous"
    );
    assert_eq!(a["contest_runtime"][0]["alternatives"], json!([]));
    assert!(
        !a["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "W-CONTEST-0")
    );
}

#[test]
fn restricted_self_authority_is_not_hidden_by_an_unrestricted_duplicate() {
    let bad = hand::source("sov-restricted.broken-expiry");
    let duplicate = hand::replace(
        &bad,
        "  process Flow",
        "  authority OldGrant { basis \"old\"; grants l1 to Ledger.post; issued_by cfo }\n  process Flow",
    );
    assert_eq!(check(&duplicate)["verdict"], "unsupported");
    let unrelated = hand::replace(
        &hand::source("sov-self-candidate"),
        "  process Flow",
        "  authority Unrelated { basis \"elsewhere\"; grants cfo to Clerk.recheck; expires 100min }\n  process Flow",
    );
    assert_eq!(check(&unrelated)["verdict"], "satisfied");
}

#[test]
fn both_parser_entry_points_fail_closed_and_no_commits_is_a_contract_error() {
    let base = hand::source("sov-basic");
    let contest = hand::replace(
        &base,
        "standing over [Ledger.post.commit]",
        "standing over [end.returned]",
    );
    for target in [
        "Ledger.post.start",
        "Ledger.post.finish",
        "Ledger.post.finish(committed)",
        "Ledger.post.cancel_ack",
        "registered",
        "Ledger",
        "Ledger.post",
    ] {
        for entry in ["standing", "contest"] {
            let src = if entry == "standing" {
                hand::replace(
                    &base,
                    "standing over [Ledger.post.commit]",
                    &format!("standing over [{target}]"),
                )
            } else {
                hand::replace(&contest, "on Ledger.post.commit", &format!("on {target}"))
            };
            let a = check(&src);
            assert_eq!(a["verdict"], "unsupported", "{entry} {target}");
            assert!(
                a["diagnostics"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|d| d["code"] == "U-SYN-0"),
                "{a}"
            );
            let d = a["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .find(|d| d["code"] == "U-SYN-0")
                .unwrap();
            assert!(
                d["message"]
                    .as_str()
                    .unwrap()
                    .contains("standing supports only end and commit targets")
            );
            let start = if entry == "standing" {
                src.find("standing over [").unwrap() + "standing over [".len()
            } else {
                src.find(&format!("on {target}")).unwrap() + 3
            };
            assert_eq!(
                d["span"],
                json!({"start":start,"end":start+target.split('.').next().unwrap().len()}),
                "C2 keeps the old first-word diagnostic span"
            );
        }
    }
    let approve = base.replace("Ledger.post.commit", "Approver.approve.commit");
    assert_ne!(approve, base);
    let a = check(&approve);
    assert_eq!(a["verdict"], "invalid");
    assert!(a["diagnostics"].as_array().unwrap().iter().any(|d| {
        d["code"] == "E-CON-0"
            && d["message"]
                .as_str()
                .unwrap()
                .contains("commit requires a commits Capability")
    }));
    // commit is a positional event word, never a globally reserved Actor name.
    let keyword = base.replace("Ledger", "commit");
    assert_ne!(keyword, base);
    assert_eq!(check(&keyword)["verdict"], "satisfied");
}

#[test]
fn duration_hazard_and_probability_graph_both_observe_commit_not_start() {
    let base = hand::source("sov-basic");
    let src = hand::replace(
        &base,
        "commits; duration fixed 1min",
        "commits; duration {1min:0.5,2min:0.5}",
    );
    for arrivals in [false, true] {
        let src = if arrivals {
            hand::replace(&src, "  bounds", "  arrivals fixed [0min]\n  bounds")
        } else {
            src.clone()
        };
        let m = model(&src);
        let e = engine::explore(&m);
        let mut r = reference::explore(&m);
        assert_eq!(r.stop, reference::StopReason::Complete);
        r.outcome.diagnostics.drain(..m.diagnostics.len());
        assert_eq!(e, r.outcome);
        let a = check(&src);
        assert_eq!(a["verdict"], "satisfied");
        let alts = a["contest_runtime"][0]["alternatives"].as_array().unwrap();
        assert_eq!(alts.len(), 6);
        for target in [3, 4] {
            let rows: Vec<_> = alts.iter().filter(|a| a["target_at"] == target).collect();
            assert_eq!(rows.len(), 3);
            for alt in rows {
                if alt["filed"] == true {
                    assert_eq!(alt["filed_at"], target);
                    assert_eq!(alt["deadline"], target + 5);
                    assert_eq!(alt["response_at"], target + 1);
                }
            }
        }
    }
}

// ---- Boundary review M2 and surviving mutations (ADR 0040, after eba77ea) ----
// Hand oracles are written in each comment before the assertions; they were
// derived from the model text, not copied from an implementation run.

/// Engine and the independent reference agree on the whole runtime Outcome in
/// both schedulers (legacy single case and the arrivals engine).
fn both_interpreters_agree(src: &str) {
    for arrivals in [false, true] {
        let src = if arrivals {
            hand::replace(src, "  bounds", "  arrivals fixed [0min]\n  bounds")
        } else {
            src.to_owned()
        };
        let m = model(&src);
        let e = engine::explore(&m);
        let mut r = reference::explore(&m);
        assert_eq!(
            r.stop,
            reference::StopReason::Complete,
            "arrivals={arrivals}"
        );
        assert!(r.outcome.diagnostics.starts_with(&m.diagnostics));
        r.outcome.diagnostics.drain(..m.diagnostics.len());
        assert_eq!(e, r.outcome, "arrivals={arrivals}");
    }
}

/// Runtime alternatives as a closed multiset (DFS order is not semantics).
fn alternative_rows(a: &Value) -> Vec<String> {
    let rows = a["contest_runtime"].as_array().unwrap();
    assert_eq!(rows.len(), 1, "one case, one contest");
    let mut out: Vec<String> = rows[0]["alternatives"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| {
            json!({
                "filed": x["filed"], "target_at": x["target_at"], "deadline": x["deadline"],
                "status": x["status"], "response_at": x["response_at"], "closed_at": x["closed_at"],
                "result": x["result"], "executor": x["executor"],
                "violation_reason": x["violation_reason"], "route_wait": x["route_wait"],
            })
            .to_string()
        })
        .collect();
    out.sort();
    out
}

#[allow(clippy::too_many_arguments)]
fn row(
    filed: bool,
    target_at: u64,
    response_at: Option<u64>,
    result: Option<&str>,
    by: u64,
    wait: Option<u64>,
    executor: Option<&str>,
) -> Value {
    let deadline = filed.then_some(target_at + by);
    let (status, reason) = match (filed, response_at) {
        (false, _) => ("not_filed", None),
        (true, Some(t)) if t > target_at + by => ("violated", Some("late_response")),
        (true, Some(_)) => ("responded", None),
        (true, None) => unreachable!("covered by the dedicated deadlock fixture"),
    };
    json!({
        "filed": filed, "target_at": target_at, "deadline": deadline, "status": status,
        "response_at": response_at,
        "closed_at": if filed { response_at } else { Some(target_at) },
        "result": result, "executor": if filed { executor } else { None },
        "violation_reason": reason,
        "route_wait": wait.map(|w| json!({"min": w, "max": w})),
    })
}

fn expect_rows(a: &Value, expected: Vec<Value>) {
    let mut e: Vec<String> = expected.iter().map(Value::to_string).collect();
    e.sort();
    assert_eq!(alternative_rows(a), e);
}

fn terminal(a: &Value, end: &str) -> (Value, Value) {
    let t = a["terminals"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["end"] == end)
        .unwrap_or_else(|| panic!("terminal {end} in {}", a["terminals"]));
    (t["elapsed"].clone(), t["work"].clone())
}

const TARGET_MERGE: &str = r#"model StandingEffects version "0.2" {
  type Verdict = enum { pass, fail }
  type Ack = enum { committed }
  type Done = enum { ok }
  people [c1, a1, cfo]
  actor Approver { executors [a1]; capability approve() -> Verdict { duration fixed 2min; result nondet { pass, fail } } }
  actor Ledger { capacity 1; capability post() -> Ack { commits; duration fixed 1min; result nondet { committed } }; capability other() -> Done { duration fixed 1min; result nondet { ok } } }
  actor Clerk { executors [c1]; capability busy() -> Done { duration fixed 5min; result nondet { ok } }; capability recheck() -> Verdict { duration fixed 1min; result nondet { pass, fail } } }
  actor Vendor { control party; standing over [Ledger.post.commit] basis "posting claim" }
  permit a1 to Approver.approve
  authority ApprovalGrant { basis "approval table"; grants a1 to Approver.approve; issued_by cfo }
  process Flow { approval = Approver.approve(); if approval == fail { end returned } else { }; fork f { ack = Ledger.post(); o = Ledger.other(); b = Clerk.busy() }; join all f; end posted }
  cut Scope { include [Approver, Ledger, Clerk, Vendor] }
  bounds { cases 1; live_tasks 4; clock 20min }
  verify safety no_resource_overcommit
  contract Controls { contest Vendor on Ledger.post.commit route Clerk.recheck reversible_by [a1] respond_by 4min }
}
"#;

/// Mutation A10 (target_at outside RouteState Eq/Hash). Hand oracle:
/// approve 0..2; fail -> End returned 2/2. pass -> fork at 2: busy (c1) 2..7;
/// post and other share Ledger (capacity 1) in either order, so the commit is
/// at 3 (post first) or 4 (other first). A filed recheck waits for c1 until 7
/// and responds 8. join all at 7 -> End posted 7, work 2+1+1+5 = 9 (route
/// excluded). Deadline 3+4 = 7 < 8 is late; 4+4 = 8 = 8 is on time. At time 4
/// the two filed histories differ only by target_at, so they must stay two
/// states: six alternatives, contest_response violated, root violated.
#[test]
fn a10_commit_times_three_and_four_are_distinct_states_with_six_alternatives() {
    let a = check(TARGET_MERGE);
    assert_eq!(a["verdict"], "violated");
    assert_eq!(property(&a, "safety no_resource_overcommit"), "satisfied");
    assert_eq!(property(&a, "contract Controls #1 contest"), "satisfied");
    assert_eq!(
        property(&a, "contract Controls #1 contest_response"),
        "violated"
    );
    let r = |n| json!({"min": n, "max": n});
    assert_eq!(terminal(&a, "returned"), (r(2), r(2)));
    assert_eq!(terminal(&a, "posted"), (r(7), r(9)));
    let mut rows = vec![
        row(false, 3, None, None, 4, None, None),
        row(false, 4, None, None, 4, None, None),
    ];
    for v in ["pass", "fail"] {
        rows.push(row(true, 3, Some(8), Some(v), 4, Some(4), Some("c1")));
        rows.push(row(true, 4, Some(8), Some(v), 4, Some(3), Some("c1")));
    }
    expect_rows(&a, rows);
    assert_eq!(
        a["contest_summary"]["business_ended_open"],
        json!({"min":"0","max":"1","reason":null})
    );
    both_interpreters_agree(TARGET_MERGE);
}

const ROUTE_CAPACITY: &str = r#"model StandingEffects version "0.2" {
  type Verdict = enum { pass, fail }
  type Ack = enum { committed }
  people [a1, cfo]
  actor Approver { executors [a1]; capability approve() -> Verdict { duration fixed 2min; result nondet { pass, fail } } }
  actor Ledger { capacity 1; capability post() -> Ack { commits; duration fixed 1min; result nondet { committed } } }
  actor Clerk { capacity 1; capability notify() -> Verdict { duration fixed 2min; result nondet { pass } }; capability recheck() -> Verdict { duration fixed 1min; result nondet { pass, fail } } }
  actor Vendor { control party; standing over [Ledger.post.commit] basis "posting claim" }
  permit a1 to Approver.approve
  authority ApprovalGrant { basis "approval table"; grants a1 to Approver.approve; issued_by cfo }
  process Flow { approval = Approver.approve(); if approval == fail { end returned } else { }; ack = Ledger.post(); n = Clerk.notify(); end posted }
  cut Scope { include [Approver, Ledger, Clerk, Vendor] }
  metrics { window 20min; currency "JPY" }
  bounds { cases 1; live_tasks 2; clock 20min }
  verify safety no_resource_overcommit
  contract Controls { contest Vendor on Ledger.post.commit route Clerk.recheck reversible_by [a1] respond_by 5min }
}
"#;

/// Mutation O05b (route ignores Actor capacity in scheduling). Hand oracle:
/// sov-concurrent with an anonymous Clerk of capacity 1 (no person), so the
/// contention is on the Actor capacity, not on c1. pass: post 2..3, commit 3.
/// Declined: notify 3..5, End 5. Filed: recheck 3..4 then notify 4..6 (End 6,
/// response 4, wait 0) or notify 3..5 then recheck 5..6 (End 5, response 6,
/// wait 2). Deadline 8. posted elapsed 5..6, work 5 (route excluded). The route
/// never runs beside notify: no_resource_overcommit satisfied, root satisfied.
#[test]
fn o05b_route_occupies_actor_capacity_when_no_person_is_shared() {
    let a = check(ROUTE_CAPACITY);
    assert_eq!(a["verdict"], "satisfied");
    assert_eq!(property(&a, "safety no_resource_overcommit"), "satisfied");
    assert_eq!(
        property(&a, "contract Controls #1 contest_response"),
        "satisfied"
    );
    let r = |lo, hi| json!({"min": lo, "max": hi});
    assert_eq!(terminal(&a, "returned"), (r(2, 2), r(2, 2)));
    assert_eq!(terminal(&a, "posted"), (r(5, 6), r(5, 5)));
    let mut rows = vec![row(false, 3, None, None, 5, None, None)];
    for v in ["pass", "fail"] {
        rows.push(row(true, 3, Some(4), Some(v), 5, Some(0), None));
        rows.push(row(true, 3, Some(6), Some(v), 5, Some(2), None));
    }
    expect_rows(&a, rows);
    both_interpreters_agree(ROUTE_CAPACITY);
}

/// Mutation F25 (self candidate tied to the Actor name Ledger). Hand oracle:
/// sov-self-candidate with every Ledger renamed Book is P06 unchanged. Book.post
/// has two effective results, so it is a self candidate beside the Approver
/// guard; l1 qualifies through PostGrant: contest satisfied, End 2/2 and 3/3,
/// response 4 <= 8. Removing PostGrant leaves l1 without Authority: violated.
#[test]
fn f25_effect_self_candidate_is_derived_not_read_from_the_actor_name() {
    for (name, verdict) in [
        ("sov-self-candidate", "satisfied"),
        ("sov-self-candidate.broken-no-grant", "violated"),
    ] {
        let src = hand::source(name);
        let renamed = src.replace("Ledger", "Book");
        assert!(src.contains("Ledger") && !renamed.contains("Ledger"));
        let a = check(&renamed);
        assert_eq!(a["verdict"], verdict, "{name}");
        assert_eq!(
            property(&a, "contract Controls #1 contest"),
            verdict,
            "{name}"
        );
        assert_eq!(
            property(&a, "contract Controls #1 contest_response"),
            "satisfied",
            "{name}"
        );
        let r = |n| json!({"min": n, "max": n});
        assert_eq!(terminal(&a, "returned"), (r(2), r(2)));
        assert_eq!(terminal(&a, "posted"), (r(3), r(3)));
        let mut rows = vec![row(false, 3, None, None, 5, None, None)];
        for v in ["pass", "fail"] {
            rows.push(row(true, 3, Some(4), Some(v), 5, Some(0), Some("c1")));
        }
        expect_rows(&a, rows);
        assert_eq!(
            a["contest_runtime"][0]["target"], "Book.post.commit",
            "{name}"
        );
    }
}

/// Mutation F38 (public IR Commit target validity unchecked). A target index
/// past the Capability table, or a Capability without commits, is malformed
/// public IR: both interpreters abort before exploring and never panic.
#[test]
fn f38_public_ir_commit_target_out_of_range_or_noncommit_aborts_without_panic() {
    use relca_core::{contest::ContestTarget, ir::CapabilityId};
    let m = model(&hand::source("sov-basic"));
    let approve = m
        .capabilities
        .iter()
        .position(|c| c.name == "approve")
        .unwrap();
    assert!(!m.capabilities[approve].commits);
    for bad_cap in [m.capabilities.len(), approve] {
        let mut bad = m.clone();
        let meta = bad.responsibility.as_mut().unwrap();
        meta.contests[0].target = ContestTarget::Commit(CapabilityId(bad_cap));
        meta.standings[0].target = ContestTarget::Commit(CapabilityId(bad_cap));
        let e = std::panic::catch_unwind(|| engine::explore(&bad))
            .unwrap_or_else(|_| panic!("engine panicked for cap {bad_cap}"));
        assert!(
            e.diagnostics.iter().any(|d| d.code == "U-SEM-0"),
            "cap {bad_cap}"
        );
        assert!(e.terminals.is_empty(), "cap {bad_cap}");
        let r = std::panic::catch_unwind(|| reference::explore(&bad))
            .unwrap_or_else(|_| panic!("reference panicked for cap {bad_cap}"));
        assert_eq!(
            r.stop,
            reference::StopReason::SemanticAbort,
            "cap {bad_cap}"
        );
    }
}

/// Mutation F22r (reference simulation samples the filing choice). ADR 0040:
/// filing is nondeterministic, not a probability, so simulation refuses at the
/// commit. The reference reports the refusal as a diagnostic of a Complete run
/// (its StopReason is not the refusal signal) and completes no replication.
#[test]
fn f22r_reference_simulation_refuses_effect_filing_without_replications() {
    let m = model(&hand::source("sov-sim"));
    let r = reference::simulate(&m);
    assert!(
        r.outcome.diagnostics.iter().any(|d| d.code == "U-SEM-0"),
        "{:?}",
        r.outcome.diagnostics
    );
    assert_eq!(r.outcome.completed_replications, 0);
}

/// Boundary review M2, explain side. Hand oracle for
/// sov-route-start.broken-zero-capacity: the counterexample is the filed
/// branch. post finishes with its commit at 3 (the trigger), End posted at 3,
/// and the queued route can never start, so the last stage is a deadlock at 3
/// carrying the contest_response violation with deadline 3 + 5 = 8. The
/// independent replay must reproduce it in both schedulers.
#[test]
fn m2_unstartable_effect_route_deadlock_counterexample_replays() {
    let src = hand::source("sov-route-start.broken-zero-capacity");
    for arrivals in [false, true] {
        let src = if arrivals {
            hand::replace(&src, "  bounds", "  arrivals fixed [0min]\n  bounds")
        } else {
            src.clone()
        };
        let v = serde_json::to_value(relca_core::explain::explain(&src)).unwrap();
        assert_eq!(v["verdict"], "violated", "arrivals={arrivals}");
        let ce = v["explanation"]["counterexamples"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["id"] == "contract Controls #1 contest_response")
            .unwrap_or_else(|| panic!("arrivals={arrivals}: {}", v["explanation"]));
        assert_eq!(ce["status"], "verified", "arrivals={arrivals}: {ce}");
        assert_eq!(ce["replay"]["status"], "verified", "arrivals={arrivals}");
        let steps = ce["steps"].as_array().unwrap();
        let last = steps.last().unwrap();
        assert_eq!(last["kind"], "deadlock", "arrivals={arrivals}: {last}");
        assert_eq!(last["time"], 3, "arrivals={arrivals}");
        let violation = &last["violation"];
        assert_eq!(violation["id"], "contract Controls #1 contest_response");
        assert_eq!(violation["deadline"], 8, "arrivals={arrivals}: {violation}");
        let trigger = &steps[violation["trigger_step"].as_u64().unwrap() as usize];
        assert_eq!(trigger["kind"], "finish", "{trigger}");
        assert_eq!(trigger["time"], 3, "{trigger}");
        assert!(
            trigger["events"]
                .as_array()
                .unwrap()
                .iter()
                .any(|e| e["kind"] == "commit"
                    && e["actor"] == "Ledger"
                    && e["capability"] == "post"),
            "{trigger}"
        );
    }
}
