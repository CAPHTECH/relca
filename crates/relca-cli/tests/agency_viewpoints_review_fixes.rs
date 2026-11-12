//! ADR0034 review fix K3: the key order of agency.viewpoints is part of the
//! wire contract (current golden snapshots also compare raw stdout bytes). The
//! expected bytes below are written by hand in struct declaration order, with
//! each tagged enum's "status" first.
use std::{path::PathBuf, process::Command};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn viewpoints_bytes_follow_declared_key_order_for_every_row_kind() {
    let source = r#"model Wire version "0.2" {
  type V = enum { pass,fail }
  observed case { flag: optional<V> }
  people [p]
  actor Doc { source document }
  actor A { executors [p]; capability draw() -> V { reads [Doc]; duration fixed 1min; result nondet { pass,fail } } }
  actor B { capacity 1; capability draw() -> V { duration fixed 1min; result nondet { pass,fail } } }
  process Flow { a = A.draw(); if a == pass { end done } else { if flag == pass { b = B.draw() } else {}; end returned } }
  cut K { include [Doc,A,B] }
  bounds { cases 1; live_tasks 2; clock 10min }
  verify safety no_resource_overcommit
}
"#;
    let dir = root().join("target/agency-viewpoints-inputs");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("review-fix-wire-{}.relca", std::process::id()));
    std::fs::write(&path, source).unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_relca"))
        .args(["check", path.to_str().unwrap()])
        .current_dir(root())
        .output()
        .unwrap();
    let stdout = String::from_utf8(o.stdout).unwrap();
    // ADR 0043: rules p1-2 (plan C01) and the effects block appended after
    // retention as the last viewpoints key (plan C02); its own key order is
    // fixed by the C5 acceptance, so only its leading tag is pinned here.
    let want = concat!(
        r#""viewpoints":{"rules":"agency-viewpoints-p1-2","#,
        r#""scope":"selected_process_lexical_calls_and_declared_contest_routes","#,
        r#""observation":{"status":"derived","sources":[{"actor":"Doc","#,
        r#""normal":{"actors":["A"],"people":["p"],"unattributed_actors":[]},"#,
        r#""contest":{"actors":[],"people":[],"unattributed_actors":[],"routes":[]}}],"#,
        r#""people":[{"person":"p","normal":{"direct":["Doc"],"indirect":[]},"contest":{"direct":[]}}],"#,
        r#""fallback":"unchecked"},"#,
        r#""framing":{"status":"derived","ends":["#,
        r#"{"status":"derived","end":"done","reachable":true,"#,
        r#""selectors":[{"call":"a","capability":"A.draw","selects":"all_contexts"}],"read_only":[]},"#,
        r#"{"status":"not_derived","end":"returned","reason":"state_guards"}],"#,
        r#""actors":["A"],"complete":false,"unchecked":["type_domain_authority","practical_framing"]},"#,
        r#""proposal":{"status":"not_derived","reason":"no_proposal_contract"},"#,
        r#""retention":{"status":"derived","capabilities":["#,
        r#"{"status":"derived","capability":"A.draw","normal":{"min":1,"max":1},"#,
        r#""contest_route":false,"basis":"unknown","fallback":"unchecked","join_any_cancellation":false},"#,
        r#"{"status":"not_derived","capability":"B.draw","reason":"state_guards","#,
        r#""contest_route":false,"basis":"unknown","fallback":"unchecked","join_any_cancellation":false}],"#,
        r#""notice":"行使回数から不要とは判定しない","#,
        r#""unchecked":["fallback_dependency","retention_basis","actual_skill"]},"#,
        r#""effects":{"status":""#,
    );
    assert_eq!(stdout.matches(r#""viewpoints":"#).count(), 1, "{stdout}");
    assert!(stdout.contains(want), "wire bytes differ:\n{stdout}");
}

// resource_infeasible: a call whose `uses`
// amount exceeds that resource's capacity (capacity 0 included) can never
// start (ADR 0033: the engine starts a task only if every use fits). F must
// not report an end reached through it as reachable, and L must not count it
// as exercised. Within capacity (waiting only) the bytes do not change.
fn viewpoints_bytes(args: &[&str]) -> String {
    let o = Command::new(env!("CARGO_BIN_EXE_relca"))
        .args(args)
        .current_dir(root())
        .output()
        .unwrap();
    let stdout = String::from_utf8(o.stdout).unwrap();
    let start = stdout.find(r#""viewpoints":"#).expect("viewpoints");
    let tail = &stdout[start..];
    let marker = r#""actual_skill"]}"#;
    let end = tail.find(marker).expect("retention end") + marker.len();
    (tail[..end].to_owned() + "}").replace("agency-viewpoints-p1-2", "agency-viewpoints-p1-1")
}
const EMPTY_O: &str =
    r#""observation":{"status":"derived","sources":[],"people":[],"fallback":"unchecked"},"#;
const P_TAIL: &str = r#""proposal":{"status":"not_derived","reason":"no_proposal_contract"},"#;
const L_TAIL: &str = r#""notice":"行使回数から不要とは判定しない","unchecked":["fallback_dependency","retention_basis","actual_skill"]}}"#;
fn head() -> String {
    concat!(
        r#""viewpoints":{"rules":"agency-viewpoints-p1-1","#,
        r#""scope":"selected_process_lexical_calls_and_declared_contest_routes","#,
    )
    .to_string()
        + EMPTY_O
}

#[test]
fn resource_infeasible_call_is_neither_reachable_in_f_nor_exercised_in_l() {
    // rsrc-over-demand.broken-too-much: R capacity 1, uses R: 2.
    // rsrc-unused-zero.broken-used: R capacity 0, uses R: 1.
    // Hand: A.x never starts (W-UNREACH-0), done is never reached; F/L are
    // not_derived(resource_infeasible), O and P unchanged, F incomplete.
    let want = head()
        + r#""framing":{"status":"derived","ends":[{"status":"not_derived","end":"done","reason":"resource_infeasible"}],"#
        + r#""actors":[],"complete":false,"unchecked":["type_domain_authority","practical_framing"]},"#
        + P_TAIL
        + r#""retention":{"status":"derived","capabilities":[{"status":"not_derived","capability":"A.x","reason":"resource_infeasible","#
        + r#""contest_route":false,"basis":"unknown","fallback":"unchecked","join_any_cancellation":false}],"#
        + L_TAIL;
    for name in [
        "rsrc-over-demand.broken-too-much",
        "rsrc-unused-zero.broken-used",
    ] {
        // C5 precisely derives this old input without call_order. Keep this
        // old conservative oracle on the still-unsupported head-of-line case.
        let s = std::fs::read_to_string(root().join("fixtures").join(name).join("model.relca"))
            .unwrap();
        let changed = s.replace(
            "  bounds {",
            "  arrivals fixed [0min]\n  policy fifo tie_by case_id then call_order\n  bounds {",
        );
        assert_ne!(s, changed);
        let dir = root().join("target/agency-viewpoints-inputs");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join(format!("conservative-{name}-{}.relca", std::process::id()));
        std::fs::write(&p, changed).unwrap();
        assert_eq!(
            viewpoints_bytes(&["check", p.to_str().unwrap()]),
            want,
            "{name}"
        );
    }
}

#[test]
fn resource_within_capacity_keeps_viewpoints_bytes() {
    // rsrc-competition: two cases contend for R capacity 1 (uses R: 1 each),
    // they only wait. rsrc-over-demand: the single-edit control (uses R: 1 ==
    // capacity 1). Bytes pinned from the pre-fix binary (52b05fb).
    let derived = |cap: &str, n: usize| {
        format!(
            r#"{{"status":"derived","capability":"{cap}","normal":{{"min":{n},"max":{n}}},"contest_route":false,"basis":"unknown","fallback":"unchecked","join_any_cancellation":false}}"#
        )
    };
    let f = r#""framing":{"status":"derived","ends":[{"status":"derived","end":"done","reachable":true,"selectors":[],"read_only":[]}],"actors":[],"complete":true,"unchecked":["type_domain_authority","practical_framing"]},"#;
    for (name, rows) in [
        (
            "rsrc-competition",
            [derived("A.y", 0), derived("A.x", 1), derived("B.y", 1)].join(","),
        ),
        ("rsrc-over-demand", derived("A.x", 1)),
    ] {
        let want = head()
            + f
            + P_TAIL
            + r#""retention":{"status":"derived","capabilities":["#
            + &rows
            + "],"
            + L_TAIL;
        let p = format!("fixtures/{name}/model.relca");
        assert_eq!(viewpoints_bytes(&["check", &p]), want, "{name}");
    }
}

#[test]
fn resource_infeasible_marks_the_call_and_later_calls_but_not_a_bypass_end() {
    // Hand: a is drawn first and does start (1..1). On a == pass, B.x needs
    // 2 units of R capacity 1 and never starts, so B.y after it never starts
    // either; done is not_derived. returned bypasses B.x and stays derived
    // with a selecting it in all contexts.
    let source = r#"model Inf version "0.2" {
  type V = enum { pass,fail }
  resource R capacity 1
  actor A { capacity 1; capability draw() -> V { duration fixed 1min; result nondet { pass,fail } } }
  actor B { capacity 1; capability x() -> V { uses R: 2; duration fixed 1min; result nondet { pass } } capability y() -> V { duration fixed 1min; result nondet { pass } } }
  process Flow { a = A.draw(); if a == pass { x = B.x(); y = B.y(); end done } else { end returned } }
  cut K { include [A,B] }
  arrivals fixed [0min]
  policy fifo tie_by case_id then call_order
  bounds { cases 1; live_tasks 2; clock 10min }
  verify safety no_resource_overcommit
}
"#;
    let dir = root().join("target/agency-viewpoints-inputs");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!(
        "review-fix-infeasible-{}.relca",
        std::process::id()
    ));
    std::fs::write(&path, source).unwrap();
    let not_derived = |cap: &str| {
        format!(
            r#"{{"status":"not_derived","capability":"{cap}","reason":"resource_infeasible","contest_route":false,"basis":"unknown","fallback":"unchecked","join_any_cancellation":false}}"#
        )
    };
    let want = head()
        + r#""framing":{"status":"derived","ends":["#
        + r#"{"status":"not_derived","end":"done","reason":"resource_infeasible"},"#
        + r#"{"status":"derived","end":"returned","reachable":true,"#
        + r#""selectors":[{"call":"a","capability":"A.draw","selects":"all_contexts"}],"read_only":[]}],"#
        + r#""actors":["A"],"complete":false,"unchecked":["type_domain_authority","practical_framing"]},"#
        + P_TAIL
        + r#""retention":{"status":"derived","capabilities":["#
        + r#"{"status":"derived","capability":"A.draw","normal":{"min":1,"max":1},"contest_route":false,"basis":"unknown","fallback":"unchecked","join_any_cancellation":false},"#
        + &not_derived("B.x")
        + ","
        + &not_derived("B.y")
        + "],"
        + L_TAIL;
    assert_eq!(viewpoints_bytes(&["check", path.to_str().unwrap()]), want);
}
