//! P3a hand inputs. Existing Q1 inputs are read-only substrates, never oracles.
#![allow(dead_code)]
use crate::q1;
use serde_json::json;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
const OFFSET_PREFIX: &str =
    "if registered == true { skipped = Ledger.post(case.id); end failed } else {};";
pub fn dir() -> PathBuf {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../.tmp")
        .join(format!(
            "p3acmpt-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
    fs::create_dir_all(&p).unwrap();
    p
}
pub fn edit(s: &str, a: &str, z: &str) -> String {
    q1::replace(s, a, z)
}
pub fn append(s: &str, z: &str) -> String {
    q1::append(s, z)
}
pub fn metrics(s: &str, breakdown: bool) -> String {
    append(
        s,
        &format!(
            r#"metrics {{ window 40min; currency "JPY"; rate Ledger per_minute 1; rate Ledger.post per_start 2 {} }}"#,
            if breakdown { "; breakdown waits" } else { "" }
        ),
    )
}
/// Distinct process names and the same lexical End inventory. Do not match
/// commands across these selections by their internal BindingId.
pub fn pair(name: &str) -> String {
    let s = q1::source(name);
    let start = s.find("process Flow {").unwrap();
    let mut depth = 0;
    let open = start + "process Flow ".len();
    let close = s[open..]
        .char_indices()
        .find_map(|(i, c)| {
            match c {
                '{' => depth += 1,
                '}' => depth -= 1,
                _ => (),
            }
            (depth == 0).then_some(open + i)
        })
        .unwrap();
    let body = &s[open + 1..close];
    let a = edit(&s, "process Flow {", "process Before {");
    let cut = if name.starts_with("k8-") { "G" } else { "C" };
    append(
        &a,
        &format!(
            "process After {{ {OFFSET_PREFIX} {body} }}\nconfiguration A {{ cut {cut}; process Before }}\nconfiguration B {{ cut {cut}; process After }}"
        ),
    )
}
pub fn one_sided() -> String {
    let s = pair("g-unknown");
    let at = s.find("process After").unwrap();
    format!(
        "{}{}",
        &s[..at],
        edit(&edit(&s[at..], q1::UU, ""), OFFSET_PREFIX, "")
    )
}
pub fn one_sided_matching_ends() -> String {
    let s = pair("g-unknown");
    let at = s.find("process After").unwrap();
    format!("{}{}", &s[..at], edit(&s[at..], q1::UU, ""))
}
pub fn wait(path: &str) -> String {
    let name = match path {
        "D" => "g-first-delivered",
        "UD" => "g-resend",
        "UU" => "g-unknown",
        _ => panic!(),
    };
    let s = edit(
        &q1::source(name),
        "backoff fixed 0min",
        "backoff fixed 3min",
    );
    append(
        &metrics(&s, true),
        "simulation replications 4 seed 7 confidence 0.5",
    )
}
pub const BASIS: &str = "assumed independent receipt loss, not measured";
pub fn point(p: &str, duration: u64) -> String {
    let s = if p == "0.1" {
        q1::source("g-probability")
    } else {
        edit(
            &q1::source("g-probability"),
            "probability 0.1 basis",
            &format!("probability {p} basis"),
        )
    };
    let s = edit(
        &s,
        "test assumption: independent receipt loss conditional on each normal finish",
        BASIS,
    );
    let s = if duration == 1 {
        s
    } else {
        edit(
            &s,
            "duration fixed 1min",
            &format!("duration fixed {duration}min"),
        )
    };
    // One fixed business question, no safety proof demanded of a sample.
    let s = edit(&s, "verify safety no_resource_overcommit", "");
    let s = edit(
        &s,
        "if ack == rejected_duplicate { end duplicate } else { end posted }",
        "end done",
    );
    let s = edit(
        &s,
        "verify probability reached registered == true >= 1",
        "verify probability reached end.done within 2min >= 0.99",
    );
    let s = edit(&s, "verify probability reached count == 2 <= 0", "");
    append(&s, "simulation replications 8 seed 7 confidence 0.5")
}
pub const DURATION: &str = r#"model FiniteTime version "0.2" {
    type R = enum { ok }
    actor A { capacity 1; capability run() -> R { duration fixed 1min; result nondet { ok } } }
    process Flow { r = A.run(); end done }
    cut C { include [A] }
    bounds { cases 1; live_tasks 2; clock 10min }
    verify probability reached end.done within 2min >= 1
}"#;
pub fn responsibility() -> String {
    append(
        DURATION,
        r#"people [p,q]
      authority G { basis "assumed grant"; grants p to A.run; issued_by q }
      responsible p for A.run"#,
    )
    .replace("actor A { capacity 1;", "actor A { executors [p];")
}
pub fn suite(points: &[(&str, String)]) -> PathBuf {
    let d = dir();
    let mut rows = Vec::new();
    for (n, s) in points {
        fs::write(d.join(format!("{n}.relca")), s).unwrap();
        rows.push(
            json!({"id":n,"model":format!("{n}.relca"),"configuration":null,"scenario":null}),
        );
    }
    let p = d.join("manifest.json");
    fs::write(&p,json!({"schema":"relca-sensitivity/v1","scope":{"kind":"enumerated_models","rationale":"explicit P3a finite assumptions, not a continuous guarantee"},"points":rows,
        "targets":[{"kind":"probability_query","id":"probability reached #1"}],"confidence":null}).to_string()).unwrap();
    p
}
pub fn agency(response: bool) -> String {
    let s = if response {
        q1::source("correlated")
    } else {
        q1::substrate("correlated")
    };
    let s = edit(
        &s,
        "type V = enum",
        "type Entry = enum { entry }\n type Ack = enum { committed }\n type V = enum",
    );
    let s = edit(&s, "capability run()", "capability run(doc: Entry)");
    let s = edit(
        &s,
        "duration fixed 1min;",
        "reads [Evidence]; duration fixed 1min;",
    );
    let s = edit(
        &s,
        "process Flow { v = Check.run()",
        "process Flow { doc = Reader.read(); v = Check.run(doc)",
    );
    let s = edit(
        &s,
        "if v == pass { end ok }",
        "if v == pass { k = Poster.post(doc); end ok }",
    );
    let s = edit(
        &s,
        "include [Check]",
        "include [Check,Original,Evidence,Reader,Poster]",
    );
    append(
        &s,
        r#"actor Original { source document }
      actor Evidence { source record }
      actor Reader { capacity 1; capability read()->Entry {reads [Original];duration fixed 1min;result nondet {entry}} }
      actor Poster { capacity 1; capability post(doc:Entry)->Ack {commits;duration fixed 1min;result nondet {committed}} }
      people [p]
      responsible p for Poster.post"#,
    )
}
pub fn control_change(kind: &str) -> String {
    let s = pair("g-resend");
    let at = s.find("process After").unwrap();
    let (prefix, suffix) = s.split_at(at);
    let changed = match kind {
        "loss" => edit(suffix, "[unknown, delivered]", "[unknown, unknown]"),
        "backoff" => edit(suffix, "backoff fixed 0min", "backoff fixed 3min"),
        "exhausted" => edit(suffix, "exhausted end failed", "exhausted end posted"),
        "attempts" => edit(
            suffix,
            "[unknown, delivered] retry max_attempts 2",
            "[unknown] retry max_attempts 1",
        ),
        "condition" => edit(
            suffix,
            "when receipt unknown",
            "when result [rejected_duplicate]",
        ),
        _ => panic!(),
    };
    format!("{prefix}{changed}")
}
