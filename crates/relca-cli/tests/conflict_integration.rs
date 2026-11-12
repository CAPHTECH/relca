//! Shared-update conflict integration with sets, invariants, records, limits and Standing.
//! Obligation (a) of the integration: the mutation survivors AG8・IR2 (u1) and
//! AG10・IR1 (u2) were undecided at a5. Both inputs put two `join any` adoptions
//! on one path, and the first adoption can only adopt through the
//! `version_check` reject value. The current rule is the conservative static
//! refusal at the second join: U-SEM-0 `multiple any adoptions on one path have
//! ambiguous result_at` (lower.rs, `join_any`), exit 4, no property rows.
//! Expected values are written from that rule and the source text (span = the
//! second `join` keyword through `-> miss`), not copied from output.
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn scratch(s: &str) -> PathBuf {
    static N: AtomicUsize = AtomicUsize::new(0);
    let dir = root()
        .join("target")
        .join(format!("conflict-integration-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(format!("{}.relca", N.fetch_add(1, Ordering::Relaxed)));
    std::fs::write(&p, s).unwrap();
    p
}

fn check(src: &str) -> (i32, Value) {
    let path = scratch(src);
    let o = Command::new(env!("CARGO_BIN_EXE_relca"))
        .args(["check", path.to_str().unwrap()])
        .output()
        .unwrap();
    (
        o.status.code().unwrap(),
        serde_json::from_slice(&o.stdout)
            .unwrap_or_else(|e| panic!("{e}: {}", String::from_utf8_lossy(&o.stdout))),
    )
}

/// Error diagnostics as (code, message, span start, span end).
fn errors(v: &Value) -> Vec<(String, String, usize, usize)> {
    v["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["severity"] == "error")
        .map(|d| {
            (
                d["code"].as_str().unwrap().to_owned(),
                d["message"].as_str().unwrap().to_owned(),
                d["span"]["start"].as_u64().unwrap() as usize,
                d["span"]["end"].as_u64().unwrap() as usize,
            )
        })
        .collect()
}

const AMBIGUOUS: &str = "multiple any adoptions on one path have ambiguous result_at";
const SECOND_JOIN: &str =
    "join any g where result == committed tie by [c, d] remaining drain if none -> miss";

/// The one expected error: U-SEM-0 at the second join, from `join` to `miss`.
fn refused_at_second_join(src: &str) {
    let (code, v) = check(src);
    let start = src.find(SECOND_JOIN).unwrap();
    assert_eq!(src.matches(SECOND_JOIN).count(), 1);
    assert_eq!(code, 4, "{v}");
    assert_eq!(v["verdict"], "unsupported", "{v}");
    assert_eq!(
        errors(&v),
        vec![(
            "U-SEM-0".to_owned(),
            AMBIGUOUS.to_owned(),
            start,
            start + SECOND_JOIN.len()
        )],
        "{v}"
    );
    assert_eq!(v["properties"], json!([]), "{v}");
    assert_eq!(v["terminals"], json!([]), "{v}");
}

/// u1 (AG8・IR2). The first join adopts `a` only when its result is
/// `committed`; `a == conflict` is reachable only through the version_check
/// reject value, which lower must keep in the call's possible results. On that
/// branch a second `join any` adopts again on the same path -> refused there.
/// AG8 (match domain = results + requires otherwise) and IR2
/// (possible_results without the reject value) drop the `a == conflict` branch
/// and accept the model.
const U1: &str = r#"model U version "0.2" {
  type R = enum { committed, conflict }
  actor Ledger { capacity 1; state holder: bool = false; capability claim() -> R { version_check [holder] else conflict; commits; writes holder; finish { holder = true }; duration fixed 1min; result nondet { committed } } }
  actor Fast { capacity 2; capability go() -> R { duration fixed 2min; result nondet { committed }; cancel forbidden_after_start } }
  actor Slow { capacity 2; capability go() -> R { duration fixed 3min; result nondet { committed }; cancel forbidden_after_start } }
  process P {
    fork f { a = Ledger.claim(); b = Fast.go() }
    join any f where result == committed tie by [a, b] remaining drain if none -> miss
    if a == conflict {
      fork g { c = Fast.go(); d = Slow.go() }
      join any g where result == committed tie by [c, d] remaining drain if none -> miss
      end again
    } else { if b == committed { end done } else { end miss } }
  }
  cut C { include [Ledger, Fast, Slow] }
  bounds { cases 1; live_tasks 2; clock 10min }
  verify reachable end.done
}
"#;

/// u2 (AG10・IR1). The first join adopts only `result == conflict`, which no
/// ordinary result produces: only the version_check reject value of the two
/// claims can be adopted, so an adoption on the path is possible only if
/// lower counts the reject among the finish results. The second join adopts
/// again on the same path -> refused there. AG10 (adoption_possible over the
/// ordinary results) and IR1 (finish_results without the reject value) clear
/// the history and accept the model.
const U2: &str = r#"model U version "0.2" {
  type R = enum { committed, conflict }
  actor Ledger { capacity 1; state holder: bool = false; capability claim() -> R { version_check [holder] else conflict; commits; writes holder; finish { holder = true }; duration fixed 1min; result nondet { committed } } }
  actor Other { capacity 1; state mark: bool = false; capability claim() -> R { version_check [mark] else conflict; commits; writes mark; finish { mark = true }; duration fixed 1min; result nondet { committed } } }
  actor Fast { capacity 2; capability go() -> R { duration fixed 2min; result nondet { committed }; cancel forbidden_after_start } }
  actor Slow { capacity 2; capability go() -> R { duration fixed 3min; result nondet { committed }; cancel forbidden_after_start } }
  process P {
    fork f { a = Ledger.claim(); b = Other.claim() }
    join any f where result == conflict tie by [a, b] remaining drain if none -> miss
    fork g { c = Fast.go(); d = Slow.go() }
    join any g where result == committed tie by [c, d] remaining drain if none -> miss
    if c == committed { end done } else { end miss }
  }
  cut C { include [Ledger, Other, Fast, Slow] }
  bounds { cases 1; live_tasks 2; clock 10min }
  verify reachable end.done
}
"#;

#[test]
fn ag8_ir2_second_any_adoption_behind_a_version_reject_branch_is_refused() {
    refused_at_second_join(U1);
}

#[test]
fn ag10_ir1_any_adoption_of_the_version_reject_value_counts_as_an_adoption() {
    refused_at_second_join(U2);
}
