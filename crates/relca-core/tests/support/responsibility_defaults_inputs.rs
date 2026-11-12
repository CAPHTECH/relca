#![allow(dead_code)]
pub fn binding() -> String {
    r#"model Defaults version "0.2" {
type V = enum { pass, fail }
actor Input { capacity 1; capability seed() -> V { duration fixed 1min; result nondet { pass } } }
actor Judge { capacity 1; capability check(e: V) -> V { duration fixed 1min; result nondet { pass, fail } } }
actor Poster { capacity 1; capability post() -> V { commits; duration fixed 1min; result nondet { pass } } }
process Flow { e = Input.seed(); v = Judge.check(e); if v == pass { p = Poster.post(); end done } else { end returned } }
cut Scope { include [Input, Judge, Poster] }
bounds { cases 1; live_tasks 2; clock 10min }
verify safety no_resource_overcommit
}"#.into()
}
pub fn state() -> String {
    r#"model StateDefault version "0.2" {
type V = enum { pass, fail }
actor Poster { capacity 1; state open: bool = true
 capability post() -> V { requires Poster.open == true else fail; commits; duration fixed 1min; result nondet { pass } }
}
process Flow { p = Poster.post(); end done }
cut Scope { include [Poster] }
bounds { cases 1; live_tasks 2; clock 10min }
simulation replications 4 seed 41020 confidence 0.5
verify safety no_resource_overcommit
}"#.into()
}
pub fn plain() -> String {
    state().replace("commits; ", "")
}
pub fn unreachable() -> String {
    binding()
        .replace("result nondet { pass, fail }", "result nondet { pass }")
        .replace("if v == pass", "if v == fail")
}
pub fn repeat() -> String {
    state()
        .replace("requires Poster.open == true else fail; ", "")
        .replace(
            "p = Poster.post(); end done",
            "p = Poster.post(); q = Poster.post(); end done",
        )
}
pub fn rule() -> String {
    binding()
        .replace(
            "actor Input {",
            "actor R { version \"v1\"; decides open: bool = true }\nactor Input {",
        )
        .replace("if v == pass", "if v == pass && R.open == true")
        .replace(
            "include [Input, Judge, Poster]",
            "include [R, Input, Judge, Poster]",
        )
}
pub fn named() -> String {
    binding().replace(
        "actor Judge { capacity 1;",
        "people [p, q]\nactor Judge { executors [p];",
    )
}
pub fn people_only() -> String {
    binding().replace(
        "type V = enum { pass, fail }",
        "type V = enum { pass, fail }\npeople [p, q]",
    )
}
pub fn shared_person(with_duty: bool) -> String {
    let source = if with_duty { duty() } else { named() };
    source.replace(
        "actor Poster { capacity 1;",
        "actor Poster { executors [p];",
    )
}
/// Each effects carrier scans all 11,000 declared people, even though this
/// Actor is anonymous. 100 carriers alone exceed 1,000,000 work units.
/// People do not change capacity/executors and no call has arguments.
/// Clock 1 bounds runtime separately; the static derivation is the subject.
pub fn effects_limit(calls: usize) -> String {
    state()
        .replace("requires Poster.open == true else fail; ", "")
        .replace(
            "actor Poster {",
            &format!(
                "people [{}]\nactor Poster {{",
                (0..11_000)
                    .map(|i| format!("p{i}"))
                    .collect::<Vec<_>>()
                    .join(",")
            ),
        )
        .replace(
            "p = Poster.post();",
            &(0..calls)
                .map(|i| format!("p{i} = Poster.post(); "))
                .collect::<String>(),
        )
        .replace("clock 10min", "clock 1min")
}
pub const UNDERIVED_NOTICE: &str =
    "responsibility_default_candidates: process Flow: U/E not_derived (derivation_limit)";
pub fn mixed_diagnostics() -> String {
    r#"model Order version "0.2" {
type V = enum { pass }
actor Poster { capacity 1; capability post() -> V { commits; duration fixed 1min; result nondet { pass } } }
process Flow { p = Poster.post(); end done }
bounds { cases 1; live_tasks 2; clock 10min }
verify safety no_resource_overcommit
}"#.into()
}
/// Hand-authored complete vector: Cut is appended before Agency/responsibility.
/// Poster starts after lines of 28 + 23 bytes and occupies 108 bytes.
pub fn mixed_expected() -> serde_json::Value {
    serde_json::json!([
        {"code":"W-CUT-0","severity":"warning","message":"Cut is undeclared: all declared Actors are included; exclusions are unknown","span":null},
        {"code":"W-RESP-0","severity":"warning","message":"Poster has no declared responsibility","span":{"start":51,"end":159}}
    ])
}
pub const MIXED_STDERR: &str = "W-CUT-0: Cut is undeclared: all declared Actors are included; exclusions are unknown\nW-RESP-0: Poster has no declared responsibility\n";
pub fn duty() -> String {
    named().replace(
        "process Flow",
        "responsible p for Judge.check\nprocess Flow",
    )
}
pub fn cases() -> Vec<(&'static str, String, Vec<&'static str>)> {
    vec![
        ("binding", binding(), vec!["Judge", "Poster"]),
        (
            "non-stopping",
            binding().replace("if v == pass", "if v == pass || v == fail"),
            vec!["Poster"],
        ),
        ("people-only", people_only(), vec!["Judge", "Poster"]),
        ("named", named(), vec!["Judge", "Poster"]),
        ("duty", duty(), vec!["Poster"]),
        (
            "shared-person-no-duty",
            shared_person(false),
            vec!["Judge", "Poster"],
        ),
        ("shared-person-one-duty", shared_person(true), vec![]),
        (
            "unrelated-duty",
            named().replace(
                "process Flow",
                "responsible q for Judge.check\nprocess Flow",
            ),
            vec!["Judge", "Poster"],
        ),
        (
            "one-of-two",
            duty().replace("executors [p]", "executors [p, q]"),
            vec!["Poster"],
        ),
        ("state", state(), vec!["Poster"]),
        ("plain", plain(), vec![]),
        ("unreachable", unreachable(), vec!["Poster"]),
        (
            "uncalled",
            unreachable().replace("p = Poster.post(); ", ""),
            vec![],
        ),
        ("repeat", repeat(), vec!["Poster"]),
        ("rule", rule(), vec!["R", "Judge", "Poster"]),
    ]
}
