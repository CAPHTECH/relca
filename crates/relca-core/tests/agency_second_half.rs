//! ADR0043: hand-derived acceptance, deliberately RED before implementation.
#[path = "support/agency_second_half_acceptance.rs"]
mod hand;
#[path = "support/agency_second_half_budget.rs"]
mod private_budget_adapter;
use relca_core::{engine, lower, reference, report, syntax};
use serde_json::{Value, json};

fn family(prefix: &str) {
    let rows: Vec<_> = hand::population()
        .into_iter()
        .filter(|r| {
            r["name"]
                .as_str()
                .unwrap()
                .split(".broken-")
                .next()
                .unwrap()
                == format!("agy2-{prefix}")
        })
        .collect();
    assert!(!rows.is_empty(), "closed family {prefix}");
    // Keep independent failures visible together in the workspace --no-fail-fast run.
    let mut failures = vec![];
    for r in rows {
        if let Err(e) = std::panic::catch_unwind(|| hand::assert_report(&r, &hand::check(&r))) {
            failures.push((r["name"].clone(), e));
        }
    }
    assert!(
        failures.is_empty(),
        "family {prefix}: {} independent RED inputs (see assertions above)",
        failures.len()
    );
}
macro_rules! families {($($test:ident: $name:literal;)+) => {$ (#[test]fn $test(){ family($name); })+};}
families! {
 t01_any_blocked: "rsrc-any";
 t02_seq_blocked_selector: "rsrc-seq";
 t03_join_all_counts_feasible_sibling: "rsrc-join-all";
 t04_call_order_is_conservative: "rsrc-callorder";
 t05_jue_and_no_guard: "jue";
 t06_single_actor_person: "concentration";
 t07_atomic_first_and_second: "atomic-u";
 t08_selected_fallback: "keep-fallback";
 t09_observed_reads_roots: "reads-observed";
 t10_exact_diff_items_unknown: "diff-items";
 t11_resp0_without_metadata: "resp0-undeclared";
 t13a_actor: "static-actor";
 t13b_path: "static-path";
 t13c_actor_state: "static-actor-state";
 t13d_action: "static-action";
 t13e_missing_token: "static-missing-fallback";
 t13f_still_called: "static-still-called";
 t13g_writer_first: "static-fork-writer-first";
 t13g_reader_first: "static-fork-reader-first";
 t13h_param_reads: "static-param";
 t14_single_actor_many_people_or: "single-actor-many-people";
 t15_single_person_many_actors_or: "single-person-many-actors";
 t16_known_two_before_anonymous: "known-two-plus-anonymous";
 t17_selected_policy_substrate: "scenario-callorder-substrate";
 t18_drain_is_not_cancellation: "rsrc-drain";
 t19_self_start_insertion_unknown: "atomic-self-start";
 t20_version_check_owner_is_state_refuser: "version-check";
 t21_permission_does_not_filter_people: "permission-not-filter";
 t22_future_writer_not_parent: "later-writer";
 t23_initial_only_read_not_judgement: "initial-read";
 t24_no_people_explicitly_unchecked: "no-people";
 t25_noncommits_actor_write_unchecked: "state-write-only";
 t26_responsibility_derived_values: "responsibility-diff";
}

#[test]
fn t12_each_label_dimension_has_a_separate_control_and_same_semantics() {
    for suffix in ["effect", "diff"] {
        for label in ["name", "grain", "boundary", "via"] {
            family(&format!("label-{suffix}-{label}"));
        }
    }
}

#[test]
fn every_new_input_has_no_comments_and_legal_legacy_substrate() {
    // Only erase the newly introduced reads and new keep intent, never a
    // neighbouring syntax error. Static fault inputs have their own legal base.
    for row in hand::population() {
        let name = row["name"].as_str().unwrap();
        let source = hand::source(name);
        assert!(!source.contains("//") && !source.contains('#'), "{name}");
        if hand::expected(name)["agency"].is_null() || name.contains("responsibility-diff") {
            continue;
        }
        let mut legacy = source;
        if name.starts_with("agy2-version-check") {
            if legacy.contains("; version_check [n] else denied") {
                legacy = hand::replace(&legacy, "; version_check [n] else denied", "");
            }
            legacy = hand::replace(&legacy, "cases 2", "cases 1");
            legacy = hand::replace(
                &legacy,
                "arrivals fixed [0min,0min]",
                "arrivals fixed [0min]",
            );
        }
        for text in [
            "reads [e,PO];",
            "reads [entry,po];",
            "reads [x];",
            "reads [e];",
        ] {
            if legacy.contains(text) {
                legacy = hand::replace(&legacy, text, "");
            }
        }
        if legacy.contains("from normal_path keep fallback") {
            legacy = hand::replace(&legacy, "from normal_path keep fallback", "reason \"kept\"");
        }
        let ast =
            syntax::parse(&legacy).unwrap_or_else(|e| panic!("substrate syntax {name}: {e:?}"));
        let lowered = if row["invocation"]["command"] == "diff"
            || row["invocation"]["configuration"] == "To"
        {
            lower::lower_configuration(&ast, "To").map(|l| l.model)
        } else {
            lower::lower(&ast)
        };
        assert!(
            lowered.is_ok(),
            "substrate lower {name}: {:?}",
            lowered.err()
        );
    }
}

#[test]
fn blocked_and_atomic_runtime_engine_reference_and_hand_times() {
    for (name, done, elapsed) in [
        ("agy2-rsrc-any", true, 2),
        ("agy2-rsrc-any.broken-no-feasible-match", false, 0),
        ("agy2-rsrc-drain.broken-drain", false, 0),
        ("agy2-rsrc-join-all", false, 0),
        ("agy2-rsrc-join-all.broken-fits", true, 5),
        ("agy2-jue", true, 3),
        ("agy2-atomic-u", true, 3),
        ("agy2-atomic-u.broken-second-post", true, 4),
    ] {
        let m = lower::lower(&syntax::parse(&hand::source(name)).unwrap()).unwrap();
        let r = reference::explore(&m);
        assert_eq!(r.stop, reference::StopReason::Complete, "{name}");
        let e = engine::explore(&m);
        assert_eq!(
            &r.outcome.diagnostics[..m.diagnostics.len()],
            m.diagnostics.as_slice()
        );
        assert_eq!(
            &r.outcome.diagnostics[m.diagnostics.len()..],
            e.diagnostics.as_slice()
        );
        assert_eq!(r.outcome.properties, e.properties, "{name}");
        assert_eq!(r.outcome.terminals, e.terminals, "{name}");
        let report = serde_json::to_value(report::check(&hand::source(name))).unwrap();
        let terminal = report["terminals"].as_array().unwrap().iter().find(|r| {
            r["end"]
                == if name.contains("jue") || name.contains("atomic-u") {
                    "posted"
                } else {
                    "done"
                }
        });
        assert_eq!(terminal.is_some(), done, "{name}");
        if let Some(t) = terminal {
            assert_eq!(t["elapsed"]["max"], elapsed, "{name}: hand minutes");
        }
    }
}

#[test]
fn param_reads_do_not_narrow_actual_argument_lineage_or_change_legacy_rules() {
    let a = serde_json::to_value(report::check(&hand::source("agy2-static-param"))).unwrap();
    let b = serde_json::to_value(report::check(&hand::source(
        "agy2-static-param.broken-explicit-param",
    )))
    .unwrap();
    assert_eq!(
        b["verdict"], "satisfied",
        "param reads must be accepted: {b}"
    );
    for key in ["lineage", "refusal", "origin", "rules"] {
        assert_eq!(a["agency"][key], b["agency"][key], "{key}");
    }
}

#[test]
fn lexical_retire_rejects_dead_branch_and_post_end_calls_before_warning() {
    let s = hand::source("agy2-static-still-called");
    let s = hand::replace(
        &s,
        "  actor Doc {",
        "  observed case { dead: bool = false }\n  actor Doc {",
    );
    for added in [
        "if dead == true { z = Clerk.enter() } else {}; ",
        "end posted; z = Clerk.enter(); ",
    ] {
        let changed = hand::replace(
            &s,
            "process ToFlow {",
            &format!("process ToFlow {{ {added}"),
        );
        let substrate = hand::replace(
            &changed,
            "from normal_path keep fallback",
            "reason \"kept\"",
        );
        let ast = syntax::parse(&substrate)
            .expect("retire counterfactual must have legal process syntax");
        assert!(
            lower::lower_configuration(&ast, "To").is_ok(),
            "only keep intent may reject this substrate"
        );
        let a = serde_json::to_value(report::check_configuration(&changed, "To")).unwrap();
        assert_eq!(
            a["verdict"], "invalid",
            "lexical presence, not dynamic reachability: {a}"
        );
        assert!(
            a["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["code"] == "E-DIFF-0")
        );
        assert!(
            !a["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["code"] == "W-AGY-3")
        );
    }
}

#[test]
fn every_preceding_writer_including_an_overwritten_one_contributes_a_parent() {
    let a = serde_json::to_value(report::check(&hand::all_writers_source())).unwrap();
    assert_eq!(a["verdict"], "satisfied", "{a}");
    let v = a["agency"]["lineage"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["call"] == "v")
        .unwrap();
    assert_eq!(v["subject_sources"], json!(["Doc", "PO", "Extra"]));
    assert_eq!(v["independent_sources"], json!([]));
    let c = &a["agency"]["viewpoints"]["effects"]["rows"][0]["concentration"];
    assert_eq!(
        c["generation_roots"],
        json!([{"call":"old","capability":"Earlier.write"},{"call":"e","capability":"Clerk.enter"}])
    );
    assert_eq!(c["single_generation_root"], false);
}

#[test]
fn reads_ancestry_includes_all_preceding_writers_and_excludes_future_writer() {
    let row = |name: &str| {
        hand::population()
            .into_iter()
            .find(|r| r["name"] == name)
            .unwrap()
    };
    for name in [
        "agy2-reads-observed",
        "agy2-later-writer",
        "agy2-later-writer.broken-later-call",
    ] {
        let a = hand::check(&row(name));
        assert_eq!(a["verdict"], "satisfied", "{name}: {a}");
        let v = a["agency"]["lineage"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["call"] == "v")
            .unwrap();
        assert_eq!(v["subject_sources"], json!(["Doc", "PO"]));
        assert_eq!(v["independent_sources"], json!([]));
        assert_eq!(
            a["agency"]["viewpoints"]["effects"]["rows"][0]["concentration"]["generation_roots"],
            json!([{"call":"e","capability":"Clerk.enter"}])
        );
    }
}

#[test]
fn responsibility_diff_same_unknown_is_not_equal_and_direct_reach_changes() {
    for (name, changed) in [
        ("agy2-responsibility-diff", true),
        ("agy2-responsibility-diff.broken-no-inspect", false),
    ] {
        let r = hand::population()
            .into_iter()
            .find(|r| r["name"] == name)
            .unwrap();
        let a = hand::check(&r);
        assert_eq!(a["verdict"], "satisfied", "{a}");
        let rows = a["agency"]["differences"]["rows"]
            .as_array()
            .expect("new meaning differences missing");
        let rr: Vec<_> = rows
            .iter()
            .filter(|r| r["viewpoint"] == "responsibility")
            .collect();
        let obs = rr.iter().find(|r| r["item"] == "observation");
        assert_eq!(obs.is_some(), changed);
        if let Some(r) = obs {
            assert_eq!(r["status"], "changed");
            assert_eq!(r["person"], "a1");
            assert_eq!(
                r["from"],
                json!({"status":"missing","reason":null,"required_sources":["Doc"],"direct":[],"indirect":["Doc"],"missing_sources":["Doc"]})
            );
            assert_eq!(
                r["to"],
                json!({"status":"present","reason":null,"required_sources":["Doc"],"direct":["Doc"],"indirect":["Doc"],"missing_sources":[]})
            );
        }
        let t = rr
            .iter()
            .find(|r| r["item"] == "time_frame")
            .expect("same unknown remains non-comparable");
        assert_eq!(t["status"], "not_comparable");
    }
}

#[test]
fn no_people_is_one_explicit_unchecked_not_silent_responsibility() {
    for (name, absent) in [
        ("agy2-no-people", false),
        ("agy2-no-people.broken-undeclared", true),
    ] {
        let a: Value = serde_json::to_value(report::check(&hand::source(name))).unwrap();
        let tags = a["agency"]["unchecked"].as_array().unwrap();
        assert_eq!(
            tags.iter()
                .filter(|x| **x == "responsibility_people_undeclared")
                .count(),
            usize::from(absent)
        );
    }
}

/// Deferred T20 (after A5, ADR 0041): engine and reference agree on both
/// version_check variants. Hand: both cases start at 0 and finish at 3 with
/// work 3 (capacity 2); with the check the second finish is denied, so n
/// ends at 1 (an added `invariant Ledger.n <= 1` holds, `<= 0` does not);
/// without it both +1 writes commute and n reaches 2 (`<= 1` is violated).
/// Static count per case 1..1; safety and state_in_range satisfied.
#[test]
fn t20_version_check_engine_reference_complete_and_final_counter() {
    let checked = hand::source("agy2-version-check");
    let plain = hand::source("agy2-version-check.broken-no-check");
    for (name, source, le1, le0) in [
        ("checked", &checked, "satisfied", "violated"),
        ("plain", &plain, "violated", "violated"),
    ] {
        let m = lower::lower(&syntax::parse(source).unwrap()).unwrap();
        let r = reference::explore(&m);
        assert_eq!(r.stop, reference::StopReason::Complete, "{name}");
        let e = engine::explore(&m);
        assert_eq!(r.outcome.properties, e.properties, "{name}");
        assert_eq!(r.outcome.terminals, e.terminals, "{name}");
        let v = serde_json::to_value(report::check(source)).unwrap();
        assert_eq!(v["verdict"], "satisfied", "{name}");
        for p in v["properties"].as_array().unwrap() {
            assert_eq!(p["verdict"], "satisfied", "{name} {}", p["id"]);
        }
        let cases = v["case_outcomes"].as_array().unwrap();
        assert_eq!(cases.len(), 2);
        for c in cases {
            let t = &c["terminals"].as_array().unwrap()[..];
            assert_eq!(t.len(), 1, "{name}");
            assert_eq!(t[0]["end"], "done");
            assert_eq!(t[0]["completed_at"], json!({"min":3,"max":3}));
            assert_eq!(t[0]["work"], json!({"min":3,"max":3}));
        }
        let ledger = &v["agency"]["viewpoints"]["retention"]["capabilities"][0];
        assert_eq!(ledger["capability"], "Ledger.post");
        assert_eq!(ledger["normal"], json!({"min":1,"max":1}), "{name}");
        for (bound, want) in [("1", le1), ("0", le0)] {
            let s = hand::replace(
                source,
                "  verify safety no_resource_overcommit\n",
                &format!(
                    "  verify safety no_resource_overcommit\n  contract Q {{ invariant Ledger.n <= {bound} }}\n"
                ),
            );
            let v = serde_json::to_value(report::check(&s)).unwrap();
            let p = v["properties"]
                .as_array()
                .unwrap()
                .iter()
                .find(|p| p["id"] == "contract Q #1 invariant")
                .unwrap();
            assert_eq!(p["verdict"], want, "{name} n <= {bound}");
        }
    }
}

/// Deferred T20, second half: ADR 0041's cf-agency, unchanged, read with the
/// C5 rules. Hand (cf-agency calc A13c and ADR 0043 ruling M1): the owner is a
/// state refuser reading only the version-checked Ledger.holder, unknown;
/// process judgements not derived, no j/u keys; the legacy refusal still
/// omits claim and state_guards appears exactly once.
#[test]
fn t20_cf_agency_owner_is_unknown_state_refuser_and_legacy_rows_unchanged() {
    let v = serde_json::to_value(report::check(&hand::source("cf-agency"))).unwrap();
    assert_eq!(v["verdict"], "satisfied");
    let a = &v["agency"];
    let rows = a["viewpoints"]["effects"]["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(row["effect"], "Ledger.claim");
    assert_eq!(
        row["state_refusers"],
        json!([{"actor":"Ledger","capability":"Ledger.claim","reads":["Ledger.holder"],"on_normal_path":"unknown"}])
    );
    assert_eq!(
        row["process_judgements"],
        json!({"status":"not_derived","reason":"state_guards"})
    );
    assert!(row.get("j").is_none() && row.get("u").is_none());
    assert_eq!(a["refusal"], json!([]));
    let unchecked = a["unchecked"].as_array().unwrap();
    assert_eq!(unchecked.iter().filter(|x| *x == "state_guards").count(), 1);
}

fn reads_record_source() -> String {
    // ADR 0038's grammar as integrated: record type, record literal initial
    // and record literal result (fixtures rec-*).
    let s = hand::source("agy2-reads-observed");
    let s = hand::replace(
        &s,
        "type Entry = enum { entry }",
        "type Entry = record { ok: bool }",
    );
    let s = hand::replace(
        &s,
        "entry: Entry = entry;",
        "entry: Entry = Entry { ok = true };",
    );
    hand::replace(
        &s,
        "result nondet { entry } }",
        "result nondet { Entry { ok = true } } }",
    )
}

/// Deferred A4 (after ADR 0038): a whole record-valued observed read behaves
/// as the enum read of T09. Hand (agy2-reads-observed calc.md): roots
/// [e/Clerk.enter]; m subject [Doc] independent [PO]; v subject [Doc, PO]
/// independent []; J m,v and U v; returned 3, posted 4; static counts 1..1.
/// `reads [entry.ok]` is U-SYN-0, never a truncated read of entry.
#[test]
fn reads_record_whole_value_adds_writer_parent() {
    let s = reads_record_source();
    assert!(s.contains("writes entry") && s.contains("reads [entry,po]"));
    let v = serde_json::to_value(report::check(&s)).unwrap();
    assert_eq!(v["verdict"], "satisfied", "{v}");
    let want = hand::expected("agy2-reads-observed");
    hand::subset(&want["agency"], &v["agency"], "record reads");
    assert_eq!(
        v["agency"]["viewpoints"]["effects"]["rows"][0]["concentration"]["generation_roots"],
        json!([{"call":"e","capability":"Clerk.enter"}])
    );
    assert_eq!(
        v["agency"]["lineage"],
        json!([
            {"call":"m","capability":"Clerk.match","subject_sources":["Doc"],"independent_sources":["PO"]},
            {"call":"v","capability":"Approver.approve","subject_sources":["Doc","PO"],"independent_sources":[]}
        ])
    );
    let ends: Vec<_> = v["terminals"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| (t["end"].clone(), t["elapsed"]["max"].clone()))
        .collect();
    assert_eq!(
        ends,
        [(json!("returned"), json!(3)), (json!("posted"), json!(4))]
    );
    for c in v["agency"]["viewpoints"]["retention"]["capabilities"]
        .as_array()
        .unwrap()
    {
        // post0..1 (the returned path skips it), the others 1..1.
        let min = if c["capability"] == "Ledger.post" {
            0
        } else {
            1
        };
        assert_eq!(
            c["normal"],
            json!({"min":min,"max":1}),
            "{}",
            c["capability"]
        );
    }
    let field = hand::replace(&s, "reads [entry,po]", "reads [entry.ok,po]");
    let f = serde_json::to_value(report::check(&field)).unwrap();
    let codes: Vec<_> = f["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["code"].as_str().unwrap())
        .collect();
    assert_eq!(codes, ["U-SYN-0"], "{f}");
    assert!(f["agency"].is_null());
}

/// Deferred B4 (after ADR 0039): the overflow End is an admission outcome,
/// never a process-selected End. Two arrivals at 0, concurrent cases 1 and
/// queue capacity 1, both `on_overflow end spill` (the limits must agree).
/// Hand: case 1 runs A.run 0..2 and ends done; case 2 overflows to spill at 0.
/// F has only done (derived, reachable); L A.run 1..1; no commits, so no
/// effect rows. Renaming only the overflow End keeps F, L and effects exactly.
const OVERFLOW_END: &str = r#"model Overflow version "0.2" {
  type T = enum { ok }
  actor A { capacity 1; capability run() -> T { duration fixed 2min; result nondet { ok } } }
  process Flow { r = A.run(); end done }
  arrivals fixed [0min, 0min]
  policy fifo tie_by case_id
  concurrent cases 1 on_overflow end spill
  queue capacity 1 on_overflow end spill
  cut K { include [A] }
  bounds { cases 2; live_tasks 1; clock 20min }
  verify safety no_resource_overcommit
}
"#;

#[test]
fn overflow_end_never_becomes_framing_classification() {
    let renamed = OVERFLOW_END.replace("end spill", "end other");
    assert_eq!(renamed.matches("end other").count(), 2);
    let mut views = vec![];
    for (source, overflow) in [(OVERFLOW_END.to_owned(), "spill"), (renamed, "other")] {
        let v = serde_json::to_value(report::check(&source)).unwrap();
        assert_eq!(v["verdict"], "satisfied", "{v}");
        let cases = v["case_outcomes"].as_array().unwrap();
        assert_eq!(cases[0]["terminals"][0]["end"], "done");
        assert_eq!(
            cases[0]["terminals"][0]["completed_at"],
            json!({"min":2,"max":2})
        );
        assert_eq!(cases[1]["terminals"][0]["kind"], "overflow");
        assert_eq!(cases[1]["terminals"][0]["end"], overflow);
        let vp = v["agency"]["viewpoints"].clone();
        let ends: Vec<_> = vp["framing"]["ends"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["end"].clone())
            .collect();
        assert_eq!(ends, [json!("done")], "{overflow} is not a framing End");
        assert_eq!(vp["framing"]["ends"][0]["reachable"], true);
        assert_eq!(
            vp["retention"]["capabilities"],
            json!([{"status":"derived","capability":"A.run","normal":{"min":1,"max":1},"contest_route":false,"basis":"unknown","fallback":"unchecked","join_any_cancellation":false}])
        );
        assert_eq!(vp["effects"]["rows"], json!([]));
        views.push((
            vp["framing"].clone(),
            vp["retention"].clone(),
            vp["effects"].clone(),
        ));
    }
    assert_eq!(views[0], views[1]);
}

fn standing_route_commits(route_reads: bool) -> String {
    // sov-basic (ADR 0040 P01) with the declared contest route made a commits
    // call, and a source Actor the route may read.
    let s = hand::source("sov-basic");
    let s = hand::replace(
        &s,
        "capability recheck() -> Verdict { duration fixed 1min;",
        if route_reads {
            "capability recheck() -> Verdict { reads [Doc]; commits; duration fixed 1min;"
        } else {
            "capability recheck() -> Verdict { commits; duration fixed 1min;"
        },
    );
    let s = hand::replace(
        &s,
        "  actor Vendor",
        "  actor Doc { source document }\n  actor Vendor",
    );
    hand::replace(
        &s,
        "include [Approver, Ledger, Clerk, Vendor]",
        "include [Approver, Ledger, Clerk, Doc, Vendor]",
    )
}

/// Deferred C4 (after ADR 0040): E rows are the lexical normal-process commits
/// only. Hand: one row, Ledger.post/ack. J and U are []: the legacy refusal
/// candidates are calls with a non-case argument (ADR 0043 context, U), and
/// approve() has none; so there are no judgement ancestors and generation
/// roots are []. (The first draft expected approve in J/U; that ignored the
/// candidate rule and was corrected before any fix.) The
/// route-only commits call Clerk.recheck is no effect row and no root; L keeps
/// it as the contest route. Changing only the route's reads changes O's contest
/// block for Doc (Clerk, c1, route Clerk.recheck) and nothing in E/J/U/roots.
/// (The first draft looked for the change in the legacy origin, which counts
/// process calls only; corrected to O's contest block before any fix.)
#[test]
fn effect_standing_route_is_not_normal_effect_row() {
    let mut effect_blocks = vec![];
    for (reads, contest) in [
        (
            false,
            json!({"actors":[],"people":[],"unattributed_actors":[],"routes":[]}),
        ),
        (
            true,
            json!({"actors":["Clerk"],"people":["c1"],"unattributed_actors":[],"routes":["Clerk.recheck"]}),
        ),
    ] {
        let v = serde_json::to_value(report::check(&standing_route_commits(reads))).unwrap();
        assert_eq!(v["verdict"], "satisfied", "{v}");
        let a = &v["agency"];
        let rows = a["viewpoints"]["effects"]["rows"].as_array().unwrap();
        assert_eq!(rows.len(), 1, "{rows:?}");
        let row = &rows[0];
        assert_eq!(row["effect"], "Ledger.post");
        assert_eq!(row["call"], "ack");
        assert_eq!(row["j"], json!([]));
        assert_eq!(row["u"], json!([]));
        assert_eq!(
            row["e"],
            json!({"actors":["Ledger"],"people":[],"unattributed_actors":["Ledger"],"permitted":[]})
        );
        assert_eq!(row["concentration"]["generation_roots"], json!([]));
        let route = a["viewpoints"]["retention"]["capabilities"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["capability"] == "Clerk.recheck")
            .unwrap()
            .clone();
        assert_eq!(route["contest_route"], true);
        // O's contest block follows the route's reads (ADR 0040, kept by C5);
        // the legacy origin counts process calls only and stays empty.
        let doc = &a["viewpoints"]["observation"]["sources"][0];
        assert_eq!(doc["actor"], "Doc");
        assert_eq!(doc["contest"], contest);
        let clerk = a["origin"]
            .as_array()
            .unwrap()
            .iter()
            .find(|o| o["actor"] == "Clerk")
            .unwrap();
        assert_eq!(clerk["direct"], json!([]));
        effect_blocks.push(a["viewpoints"]["effects"].clone());
    }
    assert_eq!(effect_blocks[0], effect_blocks[1]);
}
