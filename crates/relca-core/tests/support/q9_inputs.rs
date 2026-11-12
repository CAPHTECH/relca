//! ADR0063 inputs, authored without executing the implementation.
#![allow(dead_code)]

pub const OCCURRENCE: &str = "synthetic occurrence fixture; not measured";
pub const SERVICE: &str = "synthetic service fixture; not measured";
pub const RATES: &str = r#"rates {
      occurrence probability 0.25 basis "synthetic occurrence fixture; not measured"
      service_time basis "synthetic service fixture; not measured"
      independence per_case_per_clause
    }"#;
pub const CLAUSE: &str =
    "contest Party on end.returned route Reviewer.reply reversible_by [j] respond_by 3min";
pub const GRANT: &str = "authority G { basis \"decision\"; grants j to Judge.decide }";
pub const METRICS: &str = "metrics { window 20min; currency \"JPY\"; rate Judge per_minute 0; rate Reviewer per_minute 10; rate Judge.decide per_start 0; rate Reviewer.reply per_start 2 }";

pub fn edit(source: &str, from: &str, to: &str) -> String {
    assert_ne!(from, to);
    assert_eq!(source.matches(from).count(), 1, "single edit {from}");
    source.replacen(from, to, 1)
}

pub fn base() -> String {
    format!(
        r#"model Q9 version "0.2" {{
  type T = enum {{ pass, fail }}
  people [j,r]
  actor Judge {{ executors [j]; capability decide() -> T {{ duration fixed 1min; result nondet {{ fail }} }} }}
  actor Reviewer {{ executors [r]; capability reply() -> T {{ duration {{ 2min:0.75, 4min:0.25 }}; result nondet {{ pass }} }} }}
  actor Party {{ control party; standing over [end.returned,end.posted] basis "synthetic claim" }}
  permit j to Judge.decide
  permit r to Reviewer.reply
  {GRANT}
  process Flow {{ d = Judge.decide(); if d == fail {{ end returned }} else {{ end posted }} }}
  cut Scope {{ include [Judge,Reviewer,Party] }}
  bounds {{ cases 1; live_tasks 4; clock 20min }}
  {METRICS}
  verify reachable end.returned
  simulation replications 16 seed 1 confidence 0.95
  contract Controls {{ {CLAUSE} {RATES} }}
}}"#
    )
}

pub fn source(name: &str) -> String {
    match name {
        "c" => base(),
        "half" => edit(&base(), "probability 0.25", "probability 0.5"),
        "tiny" => edit(&base(), "probability 0.25", "probability 0.000001"),
        "tiny-no-authority" => edit(&source("tiny"), GRANT, ""),
        "zero" => edit(&base(), "probability 0.25", "probability 0"),
        "above-one" => edit(&base(), "probability 0.25", "probability 1.01"),
        "certain" => edit(&base(), "probability 0.25", "probability 1"),
        "fixed" => edit(
            &source("certain"),
            "duration { 2min:0.75, 4min:0.25 }",
            "duration fixed 2min",
        ),
        "deadline" => edit(&source("fixed"), "respond_by 3min", "respond_by 2min"),
        "late" => edit(&source("deadline"), "respond_by 2min", "respond_by 1min"),
        "clock" => edit(&source("deadline"), "clock 20min", "clock 2min"),
        "defer" => edit(
            &base(),
            "include [Judge,Reviewer,Party]",
            "include [Judge,Reviewer,Party]; defer Party.contest_rate reason \"unmeasured\" loses \"frequency and effort\"",
        ),
        "defer-fixed" => edit(
            &source("fixed"),
            "include [Judge,Reviewer,Party]",
            "include [Judge,Reviewer,Party]; defer Party.contest_rate reason \"unmeasured\" loses \"frequency and effort\"",
        ),
        "no-metrics" => edit(&base(), METRICS, ""),
        "legacy" => edit(&base(), RATES, ""),
        "legacy-fixed" => edit(
            &source("fixed"),
            &RATES.replace("probability 0.25", "probability 1"),
            "",
        ),
        "legacy-unreached" => edit(
            &source("legacy"),
            "contest Party on end.returned",
            "contest Party on end.posted",
        ),
        "deadlock" => {
            let s = edit(
                &source("fixed"),
                "actor Reviewer { executors [r]",
                "actor Reviewer { capacity 0",
            );
            edit(&s, "  permit r to Reviewer.reply\n", "")
        }
        "two-spaced" => {
            let s = edit(&source("fixed"), "cases 1", "cases 2");
            let s = edit(
                &s,
                "actor Reviewer { executors [r]; capability",
                "actor Reviewer { executors [r]; control policy; capability",
            );
            edit(
                &s,
                "  bounds {",
                "  arrivals fixed [0min,4min]\n  policy fifo tie_by case_id then call_order assign first_idle\n  bounds {",
            )
        }
        "two-burst" => edit(&source("two-spaced"), "[0min,4min]", "[0min,1min]"),
        "calendar" | "calendar-later" => {
            let opening = if name == "calendar" { 6 } else { 7 };
            let mut s = edit(
                &source("two-spaced"),
                "  type T",
                &format!(
                    "  calendar Always through 21min on_close continue {{ [0min,21min) capacity 1 }}\n  calendar Day through 21min on_close continue {{ [0min,2min) capacity 1; [2min,{opening}min) capacity 0; [{opening}min,21min) capacity 1 }}\n  person_calendar j Always\n  person_calendar r Day\n  type T"
                ),
            );
            s = edit(
                &s,
                "actor Judge { executors [j]",
                "actor Judge { executors [j]; calendar Always",
            );
            s = edit(
                &s,
                "actor Reviewer { executors [r]",
                "actor Reviewer { executors [r]; calendar Day",
            );
            edit(
                &s,
                "rate Reviewer per_minute 10;",
                "rate Reviewer per_minute 10; rate Reviewer overtime_per_minute 10;",
            )
        }
        "conditional-target" => {
            let s = edit(
                &base(),
                "  type T",
                "  kernel K { * -> { 0.5:fail; 0.5:pass } }\n  type T",
            );
            edit(
                &s,
                "duration fixed 1min; result nondet { fail }",
                "duration fixed 1min; outcomes kernel K",
            )
        }
        "ordinal" => edit(
            &base(),
            &format!("contract Controls {{ {CLAUSE}"),
            &format!("contract Controls {{ at_most_once Judge.decide.commit per case.id; {CLAUSE}"),
        )
        .replace(
            "capability decide() -> T { duration",
            "capability decide() -> T { commits; duration",
        ),
        "mixed" => edit(
            &base(),
            &format!("{RATES} }}"),
            &format!("{RATES}; {CLAUSE} }}"),
        ),
        "mixed-unreached" => edit(
            &base(),
            &format!("{RATES} }}"),
            &format!(
                "{RATES}; {} }}",
                CLAUSE.replace("end.returned", "end.posted")
            ),
        ),
        "two-clauses" => edit(
            &source("fixed"),
            &format!("{} }}", RATES.replace("probability 0.25", "probability 1")),
            &format!(
                "{0}; {CLAUSE} {0} }}",
                RATES.replace("probability 0.25", "probability 1")
            ),
        ),
        "same-tick" => {
            let s = edit(&source("fixed"), "people [j,r]", "people [j,j2,r]");
            let s = edit(&s, "executors [j]", "executors [j,j2]");
            let s = edit(
                &s,
                "permit j to Judge.decide",
                "permit [j,j2] to Judge.decide",
            );
            let s = edit(&s, "cases 1", "cases 2");
            edit(
                &s,
                "  bounds {",
                "  arrivals fixed [0min,0min]\n  policy fifo tie_by case_id then call_order assign first_idle\n  bounds {",
            )
        }
        "route-nondet" => edit(
            &source("fixed"),
            "duration fixed 2min; result nondet { pass }",
            "duration fixed 2min; result nondet { pass,fail }",
        ),
        "normal-call" => edit(
            &source("fixed"),
            "process Flow { d = Judge.decide()",
            "process Flow { pre = Reviewer.reply(); d = Judge.decide()",
        ),
        "missing-occurrence" => edit(
            &base(),
            "occurrence probability 0.25 basis \"synthetic occurrence fixture; not measured\"",
            "",
        ),
        "missing-occurrence-basis" => edit(
            &base(),
            "basis \"synthetic occurrence fixture; not measured\"",
            "",
        ),
        "empty-occurrence-basis" => edit(
            &base(),
            "basis \"synthetic occurrence fixture; not measured\"",
            "basis \"   \"",
        ),
        "missing-service-basis" => edit(
            &base(),
            "service_time basis \"synthetic service fixture; not measured\"",
            "",
        ),
        "empty-service-basis" => edit(
            &base(),
            "basis \"synthetic service fixture; not measured\"",
            "basis \"\"",
        ),
        "missing-independence" => edit(&base(), "independence per_case_per_clause", ""),
        "unknown-independence" => edit(
            &base(),
            "independence per_case_per_clause",
            "independence actor_name",
        ),
        "duplicate-rates" => edit(&base(), RATES, &format!("{RATES} {RATES}")),
        "empty-rates" => edit(&base(), RATES, "rates { }"),
        _ => panic!("unknown Q9 input {name}"),
    }
}

pub const STATIC: &[(&str, &str, &str)] = &[
    (
        "zero",
        "E-PROB-0",
        "contest_probability_zero_use_cut_or_excludes",
    ),
    ("above-one", "E-PROB-0", ""),
    ("missing-occurrence", "E-SYN-0", ""),
    ("missing-occurrence-basis", "E-SYN-0", ""),
    ("empty-occurrence-basis", "E-CON-0", ""),
    ("missing-service-basis", "E-SYN-0", ""),
    ("empty-service-basis", "E-CON-0", ""),
    ("missing-independence", "E-SYN-0", ""),
    ("unknown-independence", "U-SEM-0", ""),
    ("duplicate-rates", "U-SEM-0", ""),
    ("empty-rates", "E-SYN-0", ""),
];

pub const SIM: &[&str] = &[
    "c",
    "half",
    "fixed",
    "deadline",
    "late",
    "clock",
    "defer",
    "defer-fixed",
    "two-spaced",
    "two-burst",
    "calendar",
    "calendar-later",
    "conditional-target",
    "ordinal",
    "no-metrics",
    "mixed",
    "mixed-unreached",
    "route-nondet",
    "same-tick",
    "two-clauses",
    "deadlock",
    "normal-call",
];

pub fn substrate(name: &str) -> String {
    let s = source(name);
    let mut out = String::new();
    let mut rest = s.as_str();
    while let Some(begin) = rest.find("rates {") {
        out.push_str(&rest[..begin]);
        let end = rest[begin..].find('}').unwrap() + begin + 1;
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}
