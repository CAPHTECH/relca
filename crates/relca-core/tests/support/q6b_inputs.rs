//! ADR0064 S4 models, authored from the contract, not captured output.
#![allow(dead_code)]

pub const ROUTE: &str = "degrade Manual when status == fail";
pub const RUNTIME: &[&str] = &[
    "a7",
    "other",
    "a8",
    "must",
    "guarded",
    "guarded-open",
    "suffix",
    "known-none",
    "latent-ok",
    "requires",
    "state-guard",
    "calendar",
    "priority",
    "qualified",
    "clock",
    "defer",
    "effect",
    "explain",
    "keep-route",
    "keep-other",
];
pub const STATIC: &[(&str, &str, &str)] = &[
    ("latent", "E-POL-LATENT", ""),
    ("unknown-binding", "E-NAME-0", ""),
    ("state-condition", "U-SYN-0", ""),
    ("unknown", "U-SEM-0", "q6_feature_combination"),
    ("retry", "U-SEM-0", "q6_feature_combination"),
    ("branch-wait", "U-SEM-0", "q6_feature_combination"),
    ("branch-fork", "U-SEM-0", "q6_feature_combination"),
    ("nested", "U-SEM-0", "q6_feature_combination"),
    ("two-routes", "U-SEM-0", "q6_capacity_limit"),
];

pub fn edit(s: &str, from: &str, to: &str) -> String {
    assert_ne!(from, to);
    assert_eq!(s.matches(from).count(), 1, "single edit: {from}");
    s.replacen(from, to, 1)
}

pub fn base() -> String {
    r#"model Degradation version "0.2" {
  type Status = enum { ok, fail }
  type Choice = enum { pass, fail }
  people [p,c,f,o,r,z]
  actor Doc { source document }
  actor Summary { source record }
  actor Probe { executors [p]; capability check() -> Status { duration fixed 1min; result nondet { ok, fail } } }
  actor Clerk { executors [c]; capability enter() -> Status { reads [Doc]; duration fixed 3min; result nondet { ok } } }
  actor Fast { executors [f]; capability run() -> Status { reads [Summary]; duration fixed 1min; result nondet { ok } } }
  actor Other { executors [o]; capability enter() -> Status { duration fixed 2min; result nondet { ok } } }
  actor Review { executors [r]; capability choice() -> Choice { duration fixed 1min; result nondet { pass, fail } } }
  actor Finish { executors [z]; capability close() -> Status { duration fixed 2min; result nondet { ok } } }
  process Flow {
    status = Probe.check();
    degrade Manual when status == fail { c = Clerk.enter(); end done }
    else { f = Fast.run(); end done }
  }
  cut Scope { include [Doc,Summary,Probe,Clerk,Fast,Other,Review,Finish] }
  bounds { cases 1; live_tasks 4; clock 10min }
  metrics { window 10min; currency "JPY" }
  verify reachable end.done
  simulation replications 4 seed 1 confidence 0.95
}"#.into()
}

pub fn source(name: &str) -> String {
    match name {
        "a7" => base(),
        "other" => edit(&base(), "c = Clerk.enter()", "c = Other.enter()"),
        "a8" => edit(
            &base(),
            "c = Clerk.enter(); end done",
            "v = Review.choice(); if v == pass { c = Clerk.enter(); end done } else { x = Other.enter(); end done }",
        ),
        "must" => edit(&source("a8"), "x = Other.enter()", "x = Clerk.enter()"),
        "guarded" => edit(
            &base(),
            "c = Clerk.enter(); end done",
            "v = Review.choice(); if v == pass { if v == fail { x = Other.enter(); end impossible } else { c = Clerk.enter(); end done } } else { y = Other.enter(); end alternate }",
        ),
        "guarded-open" => edit(&source("guarded"), "if v == fail", "if v == pass"),
        "suffix" => edit(
            &edit(&base(), "c = Clerk.enter(); end done", "c = Clerk.enter()"),
            "f = Fast.run(); end done }",
            "f = Fast.run() }; z = Finish.close(); end done",
        ),
        "known-none" => edit(
            &base(),
            "status = Probe.check()",
            "status = Probe.check() response_loss none",
        ),
        "unknown" => edit(
            &source("known-none"),
            "response_loss none",
            "response_loss sequence [unknown]",
        ),
        "retry" => edit(
            &source("known-none"),
            "response_loss none",
            "response_loss none retry max_attempts 2 when result [fail] backoff fixed 0min exhausted end done",
        ),
        "latent-ok" => {
            let s = edit(
                &base(),
                "  people",
                "  latent case { secret: bool = false }\n  kernel K given [secret] { false -> { 1:ok }; true -> { 1:fail } }\n  people",
            );
            edit(
                &s,
                "result nondet { ok, fail }",
                "kernel_reads latent [secret]; outcomes kernel K",
            )
        }
        "latent" => edit(
            &source("latent-ok"),
            "when status == fail",
            "when secret == true",
        ),
        "unknown-binding" => edit(&base(), "when status == fail", "when missing == fail"),
        "state-condition" => edit(
            &edit(
                &base(),
                "  people",
                "  observed case { ready: bool = false }\n  people",
            ),
            "when status == fail",
            "when ready == true",
        ),
        "requires" => edit(
            &edit(
                &base(),
                "  people",
                "  observed case { ready: bool = false }\n  people",
            ),
            "capability enter() -> Status { reads [Doc]",
            "capability enter() -> Status { requires ready == true else ok; reads [Doc]",
        ),
        "state-guard" => edit(
            &edit(
                &base(),
                "actor Review {",
                "actor Review { state ready: bool = false;",
            ),
            "c = Clerk.enter(); end done",
            "if Review.ready == true { c = Clerk.enter() } else { }; end done",
        ),
        "calendar" => {
            let s = edit(
                &base(),
                "  people",
                "  calendar Day through 11min on_close continue { [0min,3min) capacity 0; [3min,11min) capacity 1 }\n  calendar Always through 11min on_close continue { [0min,11min) capacity 1 }\n  person_calendar c Day\n  person_calendar p Always\n  person_calendar f Always\n  person_calendar o Always\n  person_calendar r Always\n  person_calendar z Always\n  people",
            );
            let mut s = edit(
                &s,
                "executors [c]; capability",
                "executors [c]; calendar Day; capability",
            );
            for actor in ["Probe", "Fast", "Other", "Review", "Finish"] {
                s = edit(
                    &s,
                    &format!("actor {actor} {{"),
                    &format!("actor {actor} {{ calendar Always;"),
                );
            }
            s
        }
        "priority" => edit(
            &base(),
            "  bounds",
            "  arrivals fixed [0min] case_priority fixed [9]\n  policy observable priority descending tie fifo tie_by case_id then call_order assign first_idle\n  bounds",
        ),
        "qualified" => edit(
            &edit(
                &base(),
                "executors [c]; capability enter() -> Status {",
                "executors [c,z]; capability enter() -> Status { qualified_executors [z];",
            ),
            "  process Flow",
            "  permit c to Clerk.enter\n  process Flow",
        ),
        "clock" => edit(&base(), "clock 10min", "clock 3min"),
        "defer" => edit(
            &base(),
            "include [Doc,Summary,Probe,Clerk,Fast,Other,Review,Finish]",
            "include [Doc,Summary,Probe,Clerk,Fast,Other,Review,Finish]; defer Clerk.enter reason \"fixture missing rates\" loses \"time and quality\"",
        ),
        "effect" | "explain" => {
            let s = edit(
                &base(),
                "  people",
                "  observed case { marked: bool = false; manual: bool = false }\n  people",
            );
            let s = edit(
                &s,
                "duration fixed 1min; result nondet { ok, fail }",
                "writes [marked]; duration fixed 1min; result nondet { ok, fail }; finish { marked = true }",
            );
            let s = edit(
                &s,
                "reads [Doc]; duration fixed 3min; result nondet { ok }",
                "reads [Doc]; writes [manual]; duration fixed 3min; result nondet { ok }; finish { manual = true }",
            );
            let s = edit(
                &s,
                "  verify reachable end.done",
                "  contract C { invariant manual == true -> marked == true }\n  verify reachable end.done",
            );
            if name == "explain" {
                edit(
                    &s,
                    "invariant manual == true -> marked == true",
                    "invariant manual == true -> marked == false",
                )
            } else {
                s
            }
        }
        "keep" | "no-keep" => {
            let s = edit(
                &base(),
                "  process Flow {\n    status = Probe.check();\n    degrade Manual when status == fail { c = Clerk.enter(); end done }\n    else { f = Fast.run(); end done }\n  }",
                "  process FromFlow { c = Clerk.enter(); end done }\n  process Flow { f = Fast.run(); end done }\n  configuration From { cut Scope; process FromFlow }\n  configuration To { cut Scope; process Flow; intent { retire Clerk.enter from normal_path keep fallback; add Fast.run reason \"replacement\" } }",
            );
            if name == "no-keep" {
                edit(
                    &s,
                    "retire Clerk.enter from normal_path keep fallback",
                    "retire Clerk.enter reason \"not retained\"",
                )
            } else {
                s
            }
        }
        "keep-route" | "keep-other" => {
            let s = edit(
                &base(),
                "  process Flow",
                "  process FromFlow { c = Clerk.enter(); end done }\n  configuration From { cut Scope; process FromFlow }\n  configuration To { cut Scope; process Flow; intent { retire Clerk.enter from normal_path keep fallback } }\n  process Flow",
            );
            // Intent is an old lexical claim: use Other for true retirement, not a false retire assertion.
            let s = edit(
                &s,
                "c = Clerk.enter(); end done }\n    else",
                "c = Other.enter(); end done }\n    else",
            );
            if name == "keep-other" {
                edit(
                    &s,
                    "c = Other.enter(); end done }\n    else",
                    "c = Fast.run(); end done }\n    else",
                )
            } else {
                s
            }
        }
        "branch-wait" => edit(
            &base(),
            "c = Clerk.enter()",
            "wait fixed 1min; c = Clerk.enter()",
        ),
        "branch-fork" => edit(
            &base(),
            "c = Clerk.enter()",
            "fork tasks { c = Clerk.enter() } join all tasks",
        ),
        "nested" => edit(
            &base(),
            "c = Clerk.enter(); end done",
            "degrade Nested when status == fail { c = Clerk.enter(); end done } else { end done }",
        ),
        "two-routes" => edit(
            &source("suffix"),
            "z = Finish.close()",
            "degrade Again when status == fail { } else { }; z = Finish.close()",
        ),
        _ => panic!("unknown Q6b input {name}"),
    }
}

pub fn configuration(name: &str) -> Option<&'static str> {
    matches!(name, "keep" | "no-keep" | "keep-route" | "keep-other").then_some("To")
}

/// Only a syntax projection for checking old-language substrate and dynamic equivalence.
/// It is never used to derive dependency expectations.
pub fn substrate(name: &str) -> String {
    source(name).replace(ROUTE, "if status == fail")
}
