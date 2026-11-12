//! ADR0026 Stage B: arithmetic and semantic oracles fixed before implementation.
#[path = "support/requires_acceptance.rs"]
mod hand;
use relca_core::{lower, report, syntax};
use serde_json::{Value, json};
#[path = "../../relca-cli/tests/support/ledger_shape.rs"]
mod ledger_shape;
fn check(s: &str) -> Value {
    serde_json::to_value(report::check(s)).unwrap()
}
fn runtime(s: &str) -> Value {
    let legacy = hand::old_skeleton(s);
    lower::lower(
        &syntax::parse(&legacy)
            .unwrap_or_else(|e| panic!("inline old-syntax preflight: {e:?}\n{legacy}")),
    )
    .unwrap_or_else(|e| panic!("inline old-semantics preflight: {e:?}\n{legacy}"));
    check(s)
}
#[test]
fn all32_old_language_skeletons_have_legal_names_types_bounds_and_calls() {
    for name in hand::NAMES {
        let s = hand::old_skeleton(&hand::source(name));
        let ast =
            syntax::parse(&s).unwrap_or_else(|e| panic!("old syntax preflight {name}: {e:?}\n{s}"));
        lower::lower(&ast).unwrap_or_else(|e| panic!("old semantic preflight {name}: {e:?}\n{s}"));
    }
}
#[test]
fn all32_public_reports_match_hand_oracles() {
    for name in hand::NAMES {
        hand::subset(&hand::expected(name), &check(&hand::source(name)));
    }
}
#[test]
fn all16_pairs_are_exactly_one_explicit_source_edit() {
    for (good, bad, from, to) in [
        (
            "req-start",
            "false",
            "flag: bool = true",
            "flag: bool = false",
        ),
        (
            "req-after-queue",
            "close",
            "finish { flag = true }",
            "finish { flag = false }",
        ),
        ("req-observed", "latent", "observed case", "latent case"),
        ("req-explicit-else", "no-else", " else denied", ""),
        (
            "req-start-kept",
            "forbidden",
            "cancel immediate",
            "cancel forbidden_after_start",
        ),
        (
            "req-start-siblings",
            "same-target",
            "start { b = true }",
            "start { a = true }",
        ),
        (
            "req-cancel-immediate",
            "cooperative",
            "cancel immediate",
            "cancel cooperative ack_within 1min",
        ),
        ("req-wait-gap", "zero", "wait 2min", "wait 0min"),
        ("req-wait-clock", "short-clock", "clock 12min", "clock 6min"),
        (
            "req-agency",
            "remove-guard",
            "requires approval == pass else denied; ",
            "",
        ),
        (
            "req-bypass",
            "conjunction",
            "approval == pass || review == pass",
            "approval == pass && review == pass",
        ),
        (
            "req-kernel",
            "enable",
            "flag: bool = false",
            "flag: bool = true",
        ),
        (
            "req-same-time",
            "no-close",
            "finish { flag = false }",
            "finish { flag = true }",
        ),
        ("req-wait-arrivals", "no-wait", "wait 2min", "wait 0min"),
        (
            "req-wait-admission",
            "small-queue",
            "queue capacity 2",
            "queue capacity 1",
        ),
        ("req-no-capacity", "zero", "capacity 1", "capacity 0"),
    ] {
        let s = hand::source(good);
        assert_eq!(s.matches(from).count(), 1);
        assert_eq!(
            s.replacen(from, to, 1),
            hand::source(&format!("{good}.broken-{bad}"))
        );
    }
}
#[test]
fn f1_refusal_is_not_a_start_finish_commit_or_ack_and_keeps_observation() {
    let s=hand::source("req-start.broken-false").replace("  bounds", "  contract C { invariant A.run.start -> begun == true; at_most_once A.run.commit per case.id; response case.received -> one_of(A.run.finish(denied)) by 5min after case.received otherwise unresolved }\n  bounds");
    let a = runtime(&s);
    let ps = a["properties"].as_array().unwrap();
    for (id, verdict) in [
        ("contract C #1 invariant", "vacuous"),
        ("contract C #2 at_most_once", "vacuous"),
        ("contract C #3 response", "violated"),
    ] {
        assert!(
            ps.iter().any(|p| p["id"] == id && p["verdict"] == verdict),
            "{id}: {a}"
        );
    }
    let t = &a["metrics"]["cases"][0]["tasks"][0];
    hand::subset(
        &json!({"wait":{"min":null,"max":null,"reason":"refused"},"refusal_wait":{"min":"0","max":"0","reason":null}}),
        t,
    );
    let codes: Vec<_> = a["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["code"].as_str().unwrap())
        .collect();
    assert!(
        codes.contains(&"W-UNREACH-0") && codes.contains(&"W-UNREACH-3"),
        "never_started and unassigned: {a}"
    );
}
#[test]
fn f1_start_and_finish_writes_are_atomic_and_absent_on_refusal() {
    for name in ["req-start", "req-start.broken-false"] {
        let src=hand::source(name).replace("if r == ok { end accepted }","if r == ok && begun == true && finished == true { end accepted }").replace("else { end refused }","else { if begun == false && finished == false { end refused } else { end wrong } }");
        let a = runtime(&src);
        hand::subset(&hand::expected(name)["terminals"], &a["terminals"]);
    }
}
#[test]
fn f7_immediate_releases_named_person_for_next_work_at_same_clock() {
    for (name, time, work) in [
        ("req-cancel-immediate", 4, 7),
        ("req-cancel-immediate.broken-cooperative", 5, 8),
    ] {
        let src=hand::source(name).replace("  actor Fast { capacity 1;","  people [p,q]\n  actor Fast { executors [q];").replace("actor Slow { capacity 1;","actor Slow { executors [p];").replace("  process Flow", "  actor Next { executors [p]; capability run() -> T { duration fixed 1min; result nondet { ok } } }\n  process Flow").replace("; end done; end rejected","; n = Next.run(); end done; end rejected");
        let a = runtime(&src);
        assert_eq!(a["verdict"], "satisfied", "{a}");
        hand::subset(
            &json!([{"end":"done","elapsed":{"min":time,"max":time},"work":{"min":work,"max":work},"result_at":{"min":3,"max":3}}]),
            &a["terminals"],
        );
    }
}
#[test]
fn f13_gate_first_retains_both_reader_start_and_writer_finish_orders() {
    // Reader-first now REFUSES at t0 because gate_started=false. Thus the
    // accepted end can only witness Gate-first, followed at t2 by Reader-start
    // before Writer-finish. A bulk-due-finish implementation loses that end.
    let focused = hand::source("req-same-time")
        .replace(
            "flag: bool = true",
            "flag: bool = true; gate_started: bool = false",
        )
        .replacen(
            "duration fixed 2min; result nondet { ok } }",
            "duration fixed 2min; result nondet { ok }; start { gate_started = true } }",
            1,
        )
        .replace(
            "requires flag == true else denied",
            "requires flag == true && gate_started == true else denied",
        );
    let a = runtime(&focused);
    hand::subset(
        &json!([{"end":"accepted","elapsed":{"min":3,"max":3},"work":{"min":5,"max":5}},{"end":"refused","elapsed":{"min":2,"max":2},"work":{"min":4,"max":4}}]),
        &a["terminals"],
    );
}
#[test]
fn f3_f4_static_name_type_latent_and_missing_else_matrix() {
    let s = hand::source("req-observed");
    for (from, to, code, verdict) in [
        ("else denied", "else true", "E-TYPE-0", "invalid"),
        ("flag == true", "missing == true", "E-NAME-0", "invalid"),
        ("flag == true", "flag == ok", "E-TYPE-0", "invalid"),
        ("observed case", "latent case", "E-POL-LATENT", "invalid"),
        (" else denied", "", "U-SYN-0", "unsupported"),
        ("flag == true", "executor.id == p", "U-SYN-0", "unsupported"),
    ] {
        assert!(s.contains(from));
        let a = check(&s.replacen(from, to, 1));
        assert_eq!(a["verdict"], verdict, "{code}: {a}");
        assert!(
            a["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["code"] == code),
            "{code}: {a}"
        );
    }
    for ending in [";", "\n", " }"] {
        let src = format!(
            "model M version \"0.2\" {{ type T = enum {{ ok }}; observed case {{ flag: bool = true }}; actor A {{ capacity 1; capability c() -> T {{ duration fixed 1min; result nondet {{ ok }}; requires flag == true{ending} }} }}; process P {{ end done }}; bounds {{ cases 1; live_tasks 1; clock 2min }}; verify safety no_resource_overcommit }}"
        );
        let a = check(&src);
        assert_eq!(
            a["verdict"], "unsupported",
            "missing else boundary {ending}: {a}"
        );
        assert!(
            a["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["code"] == "U-SYN-0")
        );
    }
}
#[test]
fn f6_start_finish_and_start_kernel_cross_writers_are_structurally_rejected() {
    let s = hand::source("req-start-siblings.broken-same-target");
    let kernel = s
        .replace(
            "duration fixed 3min; result nondet { ok }; start { a = true }",
            "duration fixed 3min; outcomes kernel K; writes a",
        )
        .replace(
            "  process",
            "  kernel K { * -> { 1: ok with a = true } }\n  process",
        );
    for s in [
        s.clone(),
        s.replace(
            "start { a = true } } }\n  process",
            "writes a; finish { a = true } } }\n  process",
        ),
        s.replace("capacity 1", "capacity 0"),
        kernel,
    ] {
        let a = runtime(&s);
        assert_eq!(a["verdict"], "unsupported");
        assert!(
            a["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["code"] == "U-SEM-0"),
            "{a}"
        );
    }
}
#[test]
fn wait_only_leading_trailing_zero_and_exact_deadline() {
    for (body, time) in [
        ("wait 2min; wait 3min; end done", 5),
        ("wait 0min; end done", 0),
    ] {
        let s = format!(
            "model M version \"0.2\" {{ process P {{ {body} }}; bounds {{ cases 1; live_tasks 1; clock {time}min }}; verify reachable end.done; verify safety no_resource_overcommit }}"
        );
        let a = check(&s);
        assert_eq!(a["verdict"], "satisfied", "{a}");
        hand::subset(
            &json!([{"end":"done","elapsed":{"min":time,"max":time},"work":{"min":0,"max":0}}]),
            &a["terminals"],
        );
    }
    let base=hand::source("req-wait-gap").replace("a = A.short(); wait 2min; b = A.long()","wait 2min; a = A.short()").replace("  bounds","  contract C { response case.received -> one_of(end.done) by 5min after case.received otherwise unresolved }\n  bounds");
    assert_eq!(runtime(&base)["verdict"], "satisfied");
    assert_eq!(
        runtime(&base.replace("by 5min", "by 4min"))["verdict"],
        "violated"
    );
}
#[test]
fn f9_clock_cut_keeps_deadline_violation_and_completion_inconclusive() {
    let a = check(&hand::source("req-wait-clock.broken-short-clock"));
    assert_eq!(a["bounds"]["truncated"], true);
    assert_eq!(a["terminals"], json!([]));
    for id in ["recoverable end", "terminating end", "almost_sure end"] {
        assert!(
            a["properties"]
                .as_array()
                .unwrap()
                .iter()
                .any(|p| p["id"] == id && p["verdict"] == "inconclusive"),
            "{id}: {a}"
        );
    }
    assert!(
        a["properties"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["id"] == "contract Controls #1 response" && p["verdict"] == "violated")
    );
}

#[test]
fn wait_clock_cut_nulls_new_intentional_wait_extrema_without_completing_sleep() {
    let source=hand::source("req-wait-clock.broken-short-clock").replace("  bounds","  metrics { window 30min; currency \"JPY\"; rate A per_minute 2; rate A.short per_start 3; rate A.long per_start 3 }\n  bounds");
    let a = runtime(&source);
    assert_eq!(a["metrics"]["status"], "truncated", "{a}");
    assert_eq!(a["terminals"], json!([]));
    for p in [
        "/metrics/overall/intentional_wait",
        "/metrics/cases/0/intentional_wait",
    ] {
        assert_eq!(
            a.pointer(p),
            Some(&json!({"min":null,"max":null,"reason":"truncated"}))
        );
    }
    assert!(
        a.get("capability_refusals").is_none(),
        "wait alone must not add a refusal table"
    );
}

#[test]
fn wait_only_clock_cut_never_clamps_wake_to_horizon() {
    // Unlike work3/wait4/work5, this input exposes a rounded early wake as a
    // fabricated end: there is no later long Task that could hide the mistake.
    let source = "model M version \"0.2\" { process P { wait 5min; end done }; bounds { cases 1; live_tasks 0; clock 4min }; verify reachable end.done; verify recoverable end; verify terminating end; verify almost_sure end }";
    let a = runtime(source);
    assert_eq!(a["verdict"], "inconclusive", "{a}");
    assert_eq!(a["bounds"]["truncated"], true);
    assert_eq!(a["terminals"], json!([]));
    assert_eq!(a["properties"].as_array().unwrap().len(), 4);
    assert!(
        a["properties"]
            .as_array()
            .unwrap()
            .iter()
            .all(|p| p["verdict"] == "inconclusive")
    );
}

#[test]
fn wait_only_uses_zero_live_tasks_zero_work_no_fake_actor_and_five_intentional_minutes() {
    let source = "model M version \"0.2\" { process P { wait 2min; wait 3min; end done }; bounds { cases 1; live_tasks 0; clock 5min }; metrics { window 10min; currency \"JPY\" }; verify reachable end.done; verify safety no_resource_overcommit }";
    let a = runtime(source);
    assert_eq!(a["verdict"], "satisfied", "{a}");
    hand::subset(
        &json!({"actors":[],"people":[],"capabilities":[],"overall":{"work":{"min":"0","max":"0","reason":null},"intentional_wait":{"min":"5","max":"5","reason":null},"observed_cost_total":{"min":"0","max":"0","reason":null}},"cases":[{"case_id":1,"tasks":[],"first_start_wait":{"min":null,"max":null,"reason":"no_task_started"},"intentional_wait":{"min":"5","max":"5","reason":null}}]}),
        &a["metrics"],
    );
}
#[test]
fn f10_f11_requires_is_checked_without_state_guard_omission_or_new_read_edge() {
    for name in ["req-agency", "req-bypass", "req-bypass.broken-conjunction"] {
        let a = check(&hand::source(name));
        hand::subset(&hand::expected(name)["agency"], &a["agency"]);
        assert_eq!(a["agency"]["rules"], "agency-p1-requires-1");
        let ws = a["agency"]["unchecked"].as_array().unwrap();
        assert!(!ws.contains(&json!("requires")));
        assert!(!ws.contains(&json!("state_guards")));
    }
    let a = check(&hand::source("req-agency.broken-remove-guard"));
    assert!(
        a["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "W-AGY-0")
    );
}

#[test]
fn explanation_replays_refusal_wait_and_separate_same_tick_ack_without_changing_root() {
    for (name, clause, kind, time) in [
        (
            "req-start.broken-false",
            "response case.received -> one_of(A.run.finish(denied)) by 5min after case.received otherwise unresolved",
            "refuse",
            0,
        ),
        (
            "req-wait-clock",
            "response case.received -> one_of(end.done) by 6min after case.received otherwise unresolved",
            "wait",
            3,
        ),
        (
            "req-cancel-immediate",
            "response Slow.work.start -> one_of(Slow.work.finish) by 10min after Slow.work.start otherwise unresolved",
            "cancel_ack",
            3,
        ),
    ] {
        let s = if name == "req-wait-clock" {
            hand::source(name)
        } else {
            hand::source(name).replace(
                "  bounds",
                &format!("  contract Trace {{ {clause} }}\n  bounds"),
            )
        };
        let a = serde_json::to_value(relca_core::explain::explain(&s)).unwrap();
        let mut root = a.clone();
        root.as_object_mut().unwrap().remove("explanation");
        assert_eq!(
            ledger_shape::original_report(&root),
            ledger_shape::original_report(&check(&s)),
            "explain instrumentation changes root {name}"
        );
        let cs = a["explanation"]["counterexamples"].as_array().unwrap();
        let c = cs
            .iter()
            .find(|c| c["id"].as_str().unwrap().ends_with(" response"))
            .unwrap();
        assert_eq!(c["status"], "verified", "{name}: {c}");
        assert_eq!(c["replay"]["status"], "verified");
        let steps = c["steps"].as_array().unwrap();
        let matching: Vec<_> = steps.iter().filter(|s| s["kind"] == kind).collect();
        assert_eq!(matching.len(), 1, "{name}: {c}");
        assert_eq!(matching[0]["time"], time);
        if kind == "refuse" {
            let r = matching[0];
            assert_eq!(r["result"], "denied");
            assert!(r["executor"].is_null());
            assert!(!steps.iter().any(|s| {
                ["start", "finish", "commit", "cancel_ack"]
                    .iter()
                    .any(|k| s["kind"] == *k)
            }));
            for field in ["begun", "finished"] {
                assert!(
                    r["values"]["observed"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|v| v["name"] == field && v["value"] == "false")
                );
            }
        }
        if kind == "cancel_ack" {
            let requests: Vec<_> = steps
                .iter()
                .filter(|s| {
                    s["events"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|e| e["kind"] == "cancel_request" && e["ack_at"] == 3)
                })
                .collect();
            assert_eq!(requests.len(), 1);
            assert_eq!(requests[0]["time"], 3);
            assert!(
                requests[0]["index"].as_u64() < matching[0]["index"].as_u64(),
                "request and ack must be distinct transitions"
            );
        }
    }
}

#[test]
fn zero_time_refusal_does_not_prove_other_cancelled_binding_available() {
    let source=hand::source("req-start-kept").replace("begun: bool = false; finished: bool = false","begun: bool = false; finished: bool = false; flag: bool = false")
        .replace("result nondet { pass }; cancel forbidden_after_start","result nondet { pass }; requires flag == true else denied; cancel forbidden_after_start")
        .replace("where result == pass","where result == denied")
        .replace("if begun == true && finished == false { end cancelled } else { end finished }","if slow == ok { end cancelled } else { end finished }");
    let a = check(&source);
    assert_eq!(
        a["verdict"], "unsupported",
        "queued Slow may be cancelled before result: {a}"
    );
    assert!(
        a["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "U-SEM-0")
    );
}

#[test]
fn f2_refusal_wait_is_two_not_zero_and_not_queue_to_start_wait() {
    let source=hand::source("req-after-queue.broken-close").replace("  bounds", "  metrics { window 30min; currency \"JPY\"; rate A per_minute 0; rate A.close per_start 0; rate A.run per_start 0 }\n  bounds");
    let a = runtime(&source);
    let t = a["metrics"]["cases"][0]["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["binding"] == "r")
        .unwrap();
    hand::subset(
        &json!({"refused_count":{"min":"0","max":"1","reason":null},"refusal_wait":{"min":"2","max":"2","reason":null},"wait":{"min":"0","max":"0","reason":null},"cancelled_before_start_count":{"stats":{"min":"0","max":"0","reason":null}}}),
        t,
    );
}

#[test]
fn contextual_names_wait_binding_start_capability_immediate_type_stay_legacy() {
    let source = "model M version \"0.2\" { type immediate = enum { ok }; actor A { capacity 1; capability start() -> immediate { duration fixed 1min; result nondet { ok } } }; process P { wait = A.start(); end done }; bounds { cases 1; live_tasks 1; clock 2min }; verify reachable end.done }";
    let a = check(source);
    assert_eq!(a["verdict"], "satisfied");
    hand::subset(
        &json!([{"end":"done","elapsed":{"min":1,"max":1},"work":{"min":1,"max":1}}]),
        &a["terminals"],
    );
    assert!(a.get("capability_refusals").is_none());
}

macro_rules! static_start {
    ($($test:ident: $from:literal => $to:literal, $code:literal, $verdict:literal;)+)=>{$(
        #[test] fn $test() {
            let source=hand::source("req-start");
            let source=if stringify!($test).starts_with("start_") {source.replace("requires flag == true else denied; ", "")} else {source};
            assert_eq!(source.matches($from).count(),1);
            let a=check(&source.replacen($from,$to,1));
            assert_eq!(a["verdict"],$verdict,"{a}");
            assert!(a["diagnostics"].as_array().unwrap().iter().any(|d|d["code"]==$code),"{a}");
        }
    )+};
}
static_start! {
    requires_duplicate: "requires flag == true else denied" => "requires flag == true else denied; requires flag == true else denied", "U-SEM-0", "unsupported";
    start_duplicate_block: "start { begun = true }" => "start { begun = true }; start {}", "U-SEM-0", "unsupported";
    start_duplicate_target: "start { begun = true }" => "start { begun = true; begun = true }", "U-SEM-0", "unsupported";
    start_unknown_target: "start { begun = true }" => "start { missing = true }", "E-NAME-0", "invalid";
    start_wrong_constant_type: "start { begun = true }" => "start { begun = ok }", "E-TYPE-0", "invalid";
    start_result_expression: "start { begun = true }" => "start { begun = result }", "U-SYN-0", "unsupported";
    // ADR 0035 で移した形: start の代入の源の executor.id は受理する構文になった(ADR 0035 の
    // 「入れる」の executor.id の行と U10)。書込み先は観測の PersonId / optional<PersonId> に
    // 限るので、bool の begun へは E-TYPE-0。A は executors [p] を持ち、匿名の実行者の U-SEM-0 は
    // 混ざらない。受理の正例は fixtures/aset-person-start。
    start_executor_expression: "start { begun = true }" => "start { begun = executor.id }", "E-TYPE-0", "invalid";
}

#[test]
fn start_and_finish_may_overwrite_same_target_guard_reads_pre_start_value() {
    let source=hand::source("req-observed")
        .replace("requires flag == true else denied", "requires flag == true else denied; start { flag = false }; writes flag; finish { flag = true }")
        .replace("r = A.run(); end done", "r = A.run(); if flag == true { end done } else { end wrong }")
        .replace("  bounds", "  contract C { invariant A.run.start -> flag == false }\n  bounds");
    let a = runtime(&source);
    assert_eq!(a["verdict"], "satisfied", "{a}");
    hand::subset(
        &json!([{"end":"done","elapsed":{"min":4,"max":4},"work":{"min":4,"max":4}}]),
        &a["terminals"],
    );
    assert_eq!(a["capability_refusals"], json!([]));
}

#[test]
fn same_tick_start_writer_and_requires_reader_keep_both_orders() {
    let source = "model M version \"0.2\" { type T = enum { ok, denied }; observed case { flag: bool = true }; actor Writer { capacity 1; capability run() -> T { duration fixed 1min; result nondet { ok }; start { flag = false } } }; actor Reader { capacity 1; capability run() -> T { duration fixed 1min; result nondet { ok }; requires flag == true else denied } }; process P { fork f { w = Writer.run(); r = Reader.run() }; join all f; if r == ok { end accepted } else { end refused } }; bounds { cases 1; live_tasks 2; clock 3min }; verify safety no_resource_overcommit }";
    let a = runtime(source);
    assert_eq!(a["verdict"], "satisfied", "{a}");
    hand::subset(
        &json!([{"end":"accepted","elapsed":{"min":1,"max":1},"work":{"min":2,"max":2}},{"end":"refused","elapsed":{"min":1,"max":1},"work":{"min":1,"max":1}}]),
        &a["terminals"],
    );
}

#[test]
fn latent_requires_is_rejected_even_on_unused_capability_and_actor_local_latent() {
    for (decl, condition) in [
        ("latent case { truth: bool = true }", "truth == true"),
        ("", "Hidden.local == true"),
    ] {
        let src=hand::source("req-observed").replace("  process Flow",&format!("  {decl}\n  actor Hidden {{ capacity 1; latent local: bool = true; capability unused() -> T {{ duration fixed 1min; result nondet {{ ok }}; requires {condition} else denied }} }}\n  process Flow"));
        let a = check(&src);
        assert_eq!(a["verdict"], "invalid", "{a}");
        let at = src.find(condition).unwrap();
        assert!(
            a["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["code"] == "E-POL-LATENT"
                    && d["span"]["start"]
                        .as_u64()
                        .is_some_and(|s| (s as usize) < at + condition.len())
                    && d["span"]["end"].as_u64().is_some_and(|e| e as usize > at)),
            "latent read location: {a}"
        );
    }
}

// Boundary review M1 (ADR 0026 裁定 U1): a requires that reads a variable
// written by a fork sibling has no unique serial writer, so Agency must omit it
// and report state_guards, whatever the textual order of the branches.
#[test]
fn requires_reading_a_fork_sibling_write_is_omitted_in_either_branch_order() {
    let model = |branches: &str, cap: &str| {
        format!(
            "model R version \"0.2\" {{\n  type T = enum {{ ok, pass, fail, denied, posted }}\n  observed case {{ approval: optional<T>; flag: bool = false }}\n  actor Judge {{ capacity 1; capability check() -> T {{ {cap} duration fixed 2min; result nondet {{ pass, fail }} }} }}\n  actor Post {{ capacity 1; capability post() -> T {{ duration fixed 1min; result nondet {{ posted }}; requires {guard} else denied; commits }} }}\n  process Flow {{ fork f {{ {branches} }}; join all f; if p == posted {{ end posted }} else {{ end returned }} }}\n  bounds {{ cases 1; live_tasks 4; clock 30min }}\n  verify safety no_resource_overcommit\n}}\n",
            guard = if cap.starts_with("start") {
                "flag == true"
            } else {
                "approval == pass"
            }
        )
    };
    for cap in ["writes approval;", "start { flag = true };"] {
        let mut seen = Vec::new();
        for branches in [
            "j = Judge.check(); p = Post.post()",
            "p = Post.post(); j = Judge.check()",
        ] {
            let a = check(&model(branches, cap));
            assert_eq!(a["agency"]["rules"], "agency-p1-requires-1", "{a}");
            let ws = a["agency"]["unchecked"].as_array().unwrap();
            assert!(ws.contains(&json!("state_guards")), "{cap} {branches}: {a}");
            seen.push(a["agency"]["refusal"].clone());
        }
        assert_eq!(seen[0], seen[1], "{cap}: refusal depends on branch order");
    }
}

// Boundary review m1: a call refused by requires is still a nondeterministic
// choice site in the simulation's U-SEM-0 diagnostic.
#[test]
fn simulation_nondeterminism_sites_include_the_refused_call() {
    let a = serde_json::to_value(relca_core::report::simulate("model R version \"0.2\" {\n  type T = enum { ok, denied }\n  observed case { flag: bool = false }\n  actor W { capacity 1; capability open() -> T { duration fixed 1min; result nondet { ok }; start { flag = true } } }\n  actor Rd { capacity 1; capability read() -> T { duration fixed 1min; result nondet { ok }; requires flag == true else denied; commits } }\n  process Flow { fork f { w = W.open(); r = Rd.read() }; join all f; if r == ok { end accepted } else { end refused } }\n  bounds { cases 1; live_tasks 4; clock 30min }\n  simulation replications 4 seed 0 confidence 0.5\n  verify safety no_resource_overcommit\n}\n")).unwrap();
    let d = a["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["code"] == "U-SEM-0")
        .unwrap_or_else(|| panic!("{a}"));
    assert!(
        d["message"]
            .as_str()
            .unwrap()
            .contains("sites [Flow.r,Flow.w]"),
        "{d}"
    );
}

// Own mutation K14c (ADR 0026): an immediate cancel releases the person at the
// ack instant, so the next case's Slow starts at 3. Hand: case 1 work
// 3+3+{4,5} = 10..11; case 2 Fast and Slow start 3 and stop 6, Coop starts at
// the case-1 ack {4,5} and acks {7,8}: 3+3+{2,3,4} = 8..10.
#[test]
fn immediate_ack_frees_the_person_for_the_next_case_at_the_ack_time() {
    let a = check(
        "model K14Probe version \"0.2\" {\n  type T = enum { ok, pass }\n  people [p,q,r]\n  actor Fast { executors [q]; capability win() -> T { duration fixed 3min; result nondet { pass }; cancel forbidden_after_start } }\n  actor Slow { executors [p]; capability work() -> T { duration fixed 10min; result nondet { ok }; cancel immediate } }\n  actor Coop { executors [r]; capability mop() -> T { duration fixed 10min; result nondet { ok }; cancel cooperative ack_within 2min } }\n  process Flow { fork f { fast = Fast.win(); slow = Slow.work(); coop = Coop.mop() }; join any f where result == pass tie by [fast,slow,coop] remaining cancel_then_drain if none -> rejected; end done; end rejected }\n  bounds { cases 2; live_tasks 8; clock 60min }\n  arrivals fixed [0min,2min]\n  policy fifo tie_by case_id\n  verify safety no_resource_overcommit\n}\n",
    );
    assert_eq!(a["verdict"], "satisfied", "{a}");
    assert_eq!(
        a["case_outcomes"][0]["terminals"][0]["work"],
        json!({"min":10,"max":11})
    );
    assert_eq!(
        a["case_outcomes"][1]["terminals"][0]["work"],
        json!({"min":8,"max":10})
    );
}
