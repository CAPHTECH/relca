//! Mutation testing of ADR 0043 (c5): acceptance for two non-equivalent
//! mutants that survived every test. Expectations are hand-derived from the
//! models (comments below), not copied from the binary's output.
//!
//! - M29b: `agency.rules` switched to `agency-p1-reads-1` by a declared
//!   `reads [x]` instead of by a read edge (ADR 0043 U11: only models with a
//!   read edge get the new version; a written-by-nobody x makes no edge).
//! - X09c: `via_model_paths.rules` kept at `via-model-read-paths-1` although
//!   read edges were walked (plan c5 section 2.3: with an added parent the version is
//!   `via-model-read-paths-reads-1`). No fixture had both a read edge and a
//!   `via model` Actor, so the input is kept here, not in the closed
//!   fixtures/ population.
use relca_core::report;
use serde_json::{Value, json};

fn check(src: &str) -> Value {
    serde_json::to_value(report::check(src)).unwrap()
}
fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .join(name)
            .join("model.relca"),
    )
    .unwrap()
}

#[test]
fn m29b_agency_rules_follow_read_edges_not_declared_reads() {
    // agy2-initial-read: approve reads [x]; Writer.write writes x but is never
    // called, so no call before v writes x. No edge: the old version.
    // .broken-preceding-writer: w = Writer.write() precedes v, edge w→v.
    // int-class-reads.broken-observed: A.run reads [flag], flag has no writer.
    // None of them has a state guard, requires, atomic, wait, start writes or
    // immediate cancel, so p0-1 is the only other candidate.
    for (name, want) in [
        ("agy2-initial-read", "agency-p0-1"),
        (
            "agy2-initial-read.broken-preceding-writer",
            "agency-p1-reads-1",
        ),
        ("int-class-reads.broken-observed", "agency-p0-1"),
    ] {
        let v = check(&fixture(name));
        assert_eq!(v["agency"]["rules"], want, "{name}");
    }
}

/// agy2-reads-observed with `via model` on Clerk. approve() has no argument;
/// it reads entry (written by e) and po (written by m).
const READS_VIA: &str = r#"model AgencySecondHalf version "0.2" {
  type Entry = enum { entry }
  type V = enum { pass, fail }
  type Ack = enum { committed }
  people [c1, a1]
  observed case { entry: Entry = entry; po: V = pass }
  latent case { truth: bool = false }
  actor Doc { source document }
  actor PO { source record }
  actor Clerk { via model "Parser@v1"; executors [c1]; capability enter() -> Entry { reads [Doc]; writes entry; duration fixed 1min; result nondet { entry } } capability match(e: Entry) -> V { reads [e,PO]; writes po; duration fixed 1min; result nondet { pass,fail } } }
  actor Approver { executors [a1]; capability approve() -> V { reads [entry,po]; duration fixed 1min; result nondet { pass, fail } } }
  actor Ledger { capacity 1; capability post(v: V) -> Ack { commits; duration fixed 1min; result nondet { committed } } }
  process Flow { e = Clerk.enter(); m = Clerk.match(e); v = Approver.approve(); if v == pass {} else { end returned }; k = Ledger.post(v); end posted }
  cut Scope { include [Doc, PO, Clerk, Approver, Ledger] }
  bounds { cases 1; live_tasks 3; clock 20min }
  verify safety no_resource_overcommit
}
"#;

fn via(src: &str) -> Value {
    let v = check(src);
    let p = v["via_model_paths"].clone();
    assert!(p.is_object(), "{v}");
    p
}
fn unchecked() -> Value {
    json!([
        "transformation_recognition",
        "state_guard_control_paths",
        "contest_paths",
        "fallback_paths"
    ])
}

#[test]
fn x09c_via_paths_with_read_edges_use_the_reads_version() {
    // Edges: argument e→m and v→k; read edges e→v (entry) and m→v (po).
    // Targets: k commits (E); v stops k on all paths (U, `else end returned`).
    // Walk from each Clerk call (e, m), emitting a node's targets on entry,
    // then its successors in binding order:
    //   e: [e,m,v] U, [e,m,v,k] E, [e,v] U, [e,v,k] E
    //   m: [m,v] U, [m,v,k] E
    let c = |call: &str, cap: &str| json!({"call": call, "capability": cap});
    let e = c("e", "Clerk.enter");
    let m = c("m", "Clerk.match");
    let v = c("v", "Approver.approve");
    let k = c("k", "Ledger.post");
    let u = json!({"kind": "U", "call": "v", "effect_call": "k", "stops": "all_paths"});
    let ek = json!({"kind": "E", "call": "k"});
    let path = |start: &str, calls: Vec<&Value>, target: &Value| {
        json!({"actor": "Clerk", "model": "Parser@v1", "start_call": start,
               "calls": calls, "target": target})
    };
    let p = via(READS_VIA);
    assert_eq!(p["rules"], "via-model-read-paths-reads-1");
    assert_eq!(p["truncated"], false);
    assert_eq!(p["path_count_lower_bound"], 6);
    assert_eq!(
        p["paths"],
        json!([
            path("e", vec![&e, &m, &v], &u),
            path("e", vec![&e, &m, &v, &k], &ek),
            path("e", vec![&e, &v], &u),
            path("e", vec![&e, &v, &k], &ek),
            path("m", vec![&m, &v], &u),
            path("m", vec![&m, &v, &k], &ek),
        ])
    );
    assert_eq!(p["unchecked"], unchecked());
}

#[test]
fn x09c_broken_no_reads_keeps_the_old_version_and_no_path() {
    // approve() without reads: no read edge, so the old version. The only
    // edges are e→m and v→k; nothing from a Clerk call reaches v or k, and
    // neither e nor m commits or judges. No path.
    let src = READS_VIA.replacen(" reads [entry,po];", "", 1);
    assert_ne!(src, READS_VIA);
    let p = via(&src);
    assert_eq!(p["rules"], "via-model-read-paths-1");
    assert_eq!(p["truncated"], false);
    assert_eq!(p["path_count_lower_bound"], 0);
    assert_eq!(p["paths"], json!([]));
    assert_eq!(p["unchecked"], unchecked());
}
