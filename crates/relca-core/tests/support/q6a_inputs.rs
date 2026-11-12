//! ADR0064 finite inputs, authored from the adopted contract, not solver output.
#![allow(dead_code)]

pub const BASIS: &str = "fixture assumption; not measured";
pub const QUAL: &str = "qualified_executors [q]";
pub const LOAD: &str = "load queued_same_actor_at_start";
pub const CONDITIONAL: &[&str] = &["given-finish", "given-later"];
pub const CAPACITY_CONTROLS: &[&str] = &["profile-boundary", "row-boundary"];
pub const RUNTIME: &[&str] = &[
    "qualified",
    "uncontrolled",
    "forbidden",
    "shift",
    "shift-unassigned",
    "empty",
    "people",
    "reverse",
    "unassigned",
    "burst",
    "lighter",
    "held",
    "later",
    "simultaneous",
    "single",
    "clock",
    "horizon",
    "priority",
    "priority-reverse",
    "refused",
    "admitted",
    "defer",
    "known-none",
    "label",
    "unused",
];

// Diagnostics are intentionally specific; a generic parse rejection is RED.
// The ADR permits E-TYPE-0 or E-CON-0 for coverage. We select E-TYPE-0,
// reserving E-CON-0 for basis and E-PROB-0 for probability arithmetic.
pub const STATIC: &[(&str, &str, &str)] = &[
    ("empty-basis", "E-CON-0", "performance_basis_missing"),
    ("missing-person", "E-TYPE-0", "performance_coverage"),
    ("extra-person", "E-TYPE-0", "performance_coverage"),
    ("duplicate-person", "E-TYPE-0", "performance_coverage"),
    ("gap", "E-TYPE-0", "performance_coverage"),
    ("overlap", "E-TYPE-0", "performance_coverage"),
    ("empty-band", "E-TYPE-0", "performance_coverage"),
    ("finite-tail", "E-TYPE-0", "performance_coverage"),
    ("unknown-kernel", "E-NAME-0", ""),
    ("unknown-profile", "E-NAME-0", ""),
    ("unknown-person", "E-NAME-0", ""),
    ("outside-actor", "E-TYPE-0", "qualification_mismatch"),
    ("duplicate-qualified", "E-TYPE-0", "qualification_mismatch"),
    ("anonymous-qualified", "U-SEM-0", "q6_feature_combination"),
    (
        "empty-with-performance",
        "U-SEM-0",
        "q6_feature_combination",
    ),
    ("distributed-duration", "U-SEM-0", "q6_feature_combination"),
    ("unknown-load", "U-SEM-0", "load_metric_unsupported"),
    (
        "duplicate-qualification",
        "U-SEM-0",
        "q6_feature_combination",
    ),
    ("duplicate-source", "U-SEM-0", "q6_feature_combination"),
    ("version-check", "U-SEM-0", "q6_feature_combination"),
    ("bad-mass", "E-PROB-0", ""),
    (
        "wrong-contract",
        "E-TYPE-0",
        "performance_contract_mismatch",
    ),
    ("zero-mass-wrong-type", "E-TYPE-0", ""),
    ("unused-gap", "E-TYPE-0", "performance_coverage"),
    ("profile-limit", "U-SEM-0", "q6_capacity_limit"),
    ("row-limit", "U-SEM-0", "q6_capacity_limit"),
    ("response-unknown", "U-SEM-0", "q6_feature_combination"),
];

pub const PAIRS: &[(&str, &str, &str, &str)] = &[
    ("qualified", "forbidden", "permit [q]", "permit [p]"),
    (
        "given-finish",
        "given-later",
        "duration fixed 1min",
        "duration fixed 3min",
    ),
    (
        "qualified",
        "uncontrolled",
        "policy observable fifo tie_by case_id then call_order assign first_idle",
        "",
    ),
    ("shift", "empty", QUAL, "qualified_executors []"),
    ("shift", "shift-unassigned", " assign first_idle", ""),
    ("people", "reverse", "executors [p,q]", "executors [q,p]"),
    ("people", "unassigned", " assign first_idle", ""),
    (
        "burst",
        "lighter",
        "[1,infinity): kernel Half",
        "[1,infinity): kernel Quarter",
    ),
    (
        "held",
        "burst",
        "concurrent cases 1 on_overflow wait_external fifo",
        "",
    ),
    ("later", "simultaneous", "[0min,1min]", "[0min,0min]"),
    (
        "priority",
        "priority-reverse",
        "case_priority fixed [0,10]",
        "case_priority fixed [10,0]",
    ),
    ("refused", "admitted", "ready == false", "ready == true"),
];

pub fn edit(src: &str, from: &str, to: &str) -> String {
    assert_ne!(from, to);
    assert_eq!(src.matches(from).count(), 1, "single edit {from}");
    src.replacen(from, to, 1)
}
pub fn append(src: &str, item: &str) -> String {
    let i = src.rfind('}').unwrap();
    format!("{}\n{item}\n}}", &src[..i])
}

fn qualification() -> String {
    r#"model Qualification version "0.2" {
  type R = enum { ok }
  people [p,q]
  actor Desk { control policy; executors [p,q]
    capability run() -> R {
      qualified_executors [q]
      duration fixed 2min; result nondet { ok }
    }
  }
  permit [q] to Desk.run
  contract Controls { permitted Desk.run }
  process Flow { r = Desk.run(); end done }
  cut C { include [Desk] }
  bounds { cases 1; live_tasks 2; clock 20min }
  arrivals fixed [0min]
  policy observable fifo tie_by case_id then call_order assign first_idle
  metrics { window 20min; currency "JPY"; breakdown waits }
  simulation replications 16 seed 1 confidence 0.5
  verify safety no_resource_overcommit
  verify reachable end.done
}"#
    .into()
}
pub const KERNELS: &str = r#"
  kernel Low { * -> { 0.9: ok with bad = false; 0.1: bad with bad = true } }
  kernel Q40 { * -> { 0.6: ok with bad = false; 0.4: bad with bad = true } }
  kernel Half { * -> { 0.5: ok with bad = false; 0.5: bad with bad = true } }
  kernel Quarter { * -> { 0.75: ok with bad = false; 0.25: bad with bad = true } }
"#;
const PEOPLE_PROFILE: &str = r#"performance P basis "fixture assumption; not measured" {
    load none
    executor p { kernel Low }
    executor q { kernel Q40 }
  }"#;
const QUEUE_PROFILE: &str = r#"performance P basis "fixture assumption; not measured" {
    load queued_same_actor_at_start
    executor anonymous { [0,1): kernel Low; [1,infinity): kernel Half }
  }"#;
fn performance(people: bool, arrivals: &str, cases: u64, duration: u64) -> String {
    let ids = if people { "people [p,q]" } else { "" };
    let executors = if people {
        "executors [p,q]"
    } else {
        "capacity 1"
    };
    let qualification = if people {
        "qualified_executors [q,p]"
    } else {
        ""
    };
    let assignment = if people { " assign first_idle" } else { "" };
    let profile = if people {
        PEOPLE_PROFILE
    } else {
        QUEUE_PROFILE
    };
    format!(
        r#"model Performance version "0.2" {{
  type R = enum {{ ok, bad, denied }}
  {ids}
  latent case {{ bad: bool = false }}
  {KERNELS}
  {profile}
  actor Desk {{ control policy; {executors}
    capability run() -> R {{
      {qualification}
      duration fixed {duration}min; outcomes performance P; writes bad
    }}
  }}
  process Flow {{ r = Desk.run(); end done }}
  cut C {{ include [Desk] }}
  bounds {{ cases {cases}; live_tasks 4; clock 20min }}
  arrivals fixed [{arrivals}]
  policy observable fifo tie_by case_id then call_order{assignment}
  metrics {{ window 20min; currency "JPY"; breakdown waits }}
  property case.Bad = reached(bad == true)
  metric cohort.BadPerArrival = expected(count(case.Bad)) / arrivals
  simulation replications 16 seed 1 confidence 0.5
  verify probability reached bad == true <= 1
}}"#
    )
}

pub fn source(name: &str) -> String {
    match name {
        "qualified" => qualification(),
        "shift" => {
            let s = edit(
                &qualification(),
                "actor Desk { control policy; executors [p,q]",
                "actor Desk { control policy; capacity 2; executors [p,q]; calendar Always",
            );
            append(
                &s,
                r#"calendar Always through 21min on_close continue { [0min,21min) capacity 2 }
  calendar PersonAlways through 21min on_close continue { [0min,21min) capacity 1 }
  calendar Late through 21min on_close continue { [0min,3min) capacity 0; [3min,21min) capacity 1 }
  person_calendar p PersonAlways
  person_calendar q Late"#,
            )
        }
        "people" => performance(true, "0min", 1, 2),
        "given-finish" => r#"model ConditionalPerformance version "0.2" {
  type T = enum { ok }
  observed case { flag: bool = false; hit: bool = false }
  kernel K given [flag] {
    false -> { 0: ok with hit = true; 1: ok with hit = false }
    true -> { 0.5: ok with hit = true; 0.5: ok with hit = false }
  }
  performance P basis "fixture assumption; not measured" { load none; executor anonymous { kernel K } }
  actor Writer { capacity 1; capability set() -> T { duration fixed 1min; result nondet { ok }; writes flag; finish { flag = true } } }
  actor Sampler { capacity 1; capability sample() -> T { duration fixed 2min; outcomes performance P; writes hit } }
  process Flow { fork f { w = Writer.set(); s = Sampler.sample() }; join all f; if hit == true { end bad } else { end good } }
  cut C { include [Writer,Sampler] }
  arrivals fixed [0min]
  policy observable fifo tie_by case_id then call_order
  bounds { cases 1; live_tasks 2; clock 20min }
  property case.Hit = reached(hit == true)
  metric cohort.HitPerArrival = expected(count(case.Hit)) / arrivals
  verify safety no_resource_overcommit
  verify probability reached hit == true >= 0.5
}"#.into(),
        "given-later" => edit(&source("given-finish"), "duration fixed 1min", "duration fixed 3min"),
        "burst" => edit(
            &source("held"),
            "concurrent cases 1 on_overflow wait_external fifo",
            "",
        ),
        "held" => append(
            &performance(false, "0min,0min,0min", 3, 2),
            "concurrent cases 1 on_overflow wait_external fifo",
        ),
        "later" => performance(false, "0min,1min", 2, 4),
        "single" => append(
            &performance(false, "0min", 1, 4),
            "verify reachable end.done",
        ),
        "clock" => edit(&source("single"), "clock 20min", "clock 3min"),
        "horizon" => edit(
            &edit(&source("single"), "clock 20min", "clock 10min"),
            "arrivals fixed [0min]",
            "scenario Window { arrivals fixed [0min]; horizon 3min }",
        ),
        "priority" => {
            let s = edit(&source("shift"), "cases 1;", "cases 2;");
            let s = edit(
                &s,
                "arrivals fixed [0min]",
                "arrivals fixed [0min,0min]\n  case_priority fixed [0,10]",
            );
            edit(
                &s,
                "policy observable fifo tie_by case_id",
                "policy observable priority descending tie fifo tie_by case_id",
            )
        }
        "refused" => {
            let s = performance(true, "0min", 1, 2);
            let s = edit(&s, "qualified_executors [q,p]", QUAL);
            let s = edit(&s, "    executor p { kernel Low }\n", "");
            let s = edit(
                &s,
                "duration fixed 2min;",
                "requires ready == false else denied; uses Slots: 1; duration fixed 2min;",
            );
            append(
                &s,
                "observed case { ready: bool = true }\n  resource Slots capacity 1",
            )
        }
        "defer" => edit(
            &source("burst"),
            "include [Desk]",
            "include [Desk]; defer Desk.run reason \"fixture unmeasured\" loses \"quality and workload\"",
        ),
        "known-none" => edit(
            &edit(
                &source("people"),
                "r = Desk.run();",
                "r = Desk.run() response_loss none;",
            ),
            "; breakdown waits",
            "",
        ),
        "label" => edit(
            &source("people"),
            "actor Desk {",
            "actor Desk { grain document;",
        ),
        "unused" => append(&legacy("ordinary"), PEOPLE_PROFILE),
        "empty-basis" => edit(&source("people"), BASIS, ""),
        "missing-person" => edit(&source("people"), "    executor q { kernel Q40 }\n", ""),
        "extra-person" => append_executor(&source("refused"), "executor p { kernel Low }"),
        "duplicate-person" => append_executor(&source("people"), "executor p { kernel Low }"),
        "gap" => edit(&source("burst"), "[1,infinity)", "[2,infinity)"),
        "overlap" => edit(&source("burst"), "[1,infinity)", "[0,infinity)"),
        "empty-band" => edit(&source("burst"), "[0,1)", "[0,0)"),
        "finite-tail" => edit(&source("burst"), "[1,infinity)", "[1,20)"),
        "unknown-kernel" => edit(
            &source("people"),
            "executor q { kernel Q40 }",
            "executor q { kernel Missing }",
        ),
        "unknown-profile" => edit(
            &source("people"),
            "outcomes performance P",
            "outcomes performance Missing",
        ),
        "unknown-person" => edit(&source("qualified"), QUAL, "qualified_executors [missing]"),
        "outside-actor" => edit(
            &edit(&source("qualified"), "people [p,q]", "people [p,q,x]"),
            QUAL,
            "qualified_executors [x]",
        ),
        "duplicate-qualified" => edit(&source("qualified"), QUAL, "qualified_executors [q,q]"),
        "anonymous-qualified" => edit(
            &source("single"),
            "duration fixed 4min;",
            "qualified_executors [q]; duration fixed 4min;",
        )
        .replace("  latent case", "  people [q]\n  latent case"),
        "empty-with-performance" => edit(
            &source("people"),
            "qualified_executors [q,p]",
            "qualified_executors []",
        ),
        "distributed-duration" => edit(
            &source("people"),
            "duration fixed 2min",
            "duration distribution { 1: 2min }",
        ),
        "unknown-load" => edit(&source("burst"), LOAD, "load utilization"),
        "duplicate-qualification" => edit(
            &source("qualified"),
            QUAL,
            "qualified_executors [q]; qualified_executors [q]",
        ),
        "duplicate-source" => edit(
            &source("people"),
            "outcomes performance P",
            "outcomes performance P; result nondet { ok }",
        ),
        "version-check" => edit(
            &edit(&source("people"), "latent case", "observed case"),
            "writes bad",
            "writes bad; version_check [bad] else denied",
        ),
        "bad-mass" => edit(&source("people"), "0.9: ok", "0.8: ok"),
        "wrong-contract" => edit(&source("people"),
            "kernel Q40 { * -> { 0.6: ok with bad = false; 0.4: bad with bad = true } }",
            "kernel Q40 { * -> { 0.6: ok; 0.4: bad } }"),
        "zero-mass-wrong-type" => edit(
            &source("people"),
            "0.4: bad with bad = true",
            "0.4: bad with bad = true; 0: missing with bad = false",
        ),
        "unused-gap" => edit(
            &append(&legacy("ordinary"), QUEUE_PROFILE),
            "[1,infinity)",
            "[2,infinity)",
        ),
        "profile-limit" => {
            let mut s = source("single");
            for i in 1..=256 {
                s = append(
                    &s,
                    &QUEUE_PROFILE
                        .replace("performance P basis", &format!("performance P{i} basis")),
                );
            }
            s
        }
        "profile-boundary" => edit(&source("profile-limit"),
            &QUEUE_PROFILE.replace("performance P basis", "performance P256 basis"), ""),
        "row-limit" => {
            let bands: String = (0..4096)
                .map(|i| format!("[{i},{}): kernel Low; ", i + 1))
                .collect();
            edit(
                &source("single"),
                "[0,1): kernel Low; [1,infinity): kernel Half",
                &format!("{bands}[4096,infinity): kernel Half"),
            )
        }
        "row-boundary" => edit(&source("row-limit"),
            "[4095,4096): kernel Low; [4096,infinity): kernel Half", "[4095,infinity): kernel Half"),
        "response-unknown" => edit(
            &source("people"),
            "r = Desk.run();",
            "r = Desk.run() response_loss sequence [unknown];",
        ),
        _ => {
            let &(base, _, from, to) = PAIRS
                .iter()
                .find(|p| p.1 == name)
                .unwrap_or_else(|| panic!("unknown Q6a input {name}"));
            edit(&source(base), from, to)
        }
    }
}
fn append_executor(src: &str, extra: &str) -> String {
    edit(src, "    load none", &format!("    load none\n    {extra}"))
}

/// Explicit old-language controls, not projections of numeric output.
pub fn legacy(name: &str) -> String {
    match name {
        "ordinary" => substrate(&source("people")),
        "calendar" => substrate(&source("shift")),
        "permit" => substrate(&source("forbidden")),
        "refused" => substrate(&source("refused")),
        "clock" => substrate(&source("clock")),
        "horizon" => substrate(&source("horizon")),
        "words" => edit(
            &substrate(&source("qualified")),
            "r = Desk.run()",
            "performance = Desk.run()",
        ),
        _ => panic!("unknown legacy {name}"),
    }
}
pub fn substrate(src: &str) -> String {
    let mut out = String::new();
    let mut depth: i32 = 0;
    for line in src.lines() {
        if depth != 0 || line.trim_start().starts_with("performance ") {
            depth += line.chars().filter(|c| *c == '{').count() as i32;
            depth -= line.chars().filter(|c| *c == '}').count() as i32;
            continue;
        }
        let mut line = line.to_string();
        while let Some(i) = line.find("qualified_executors [") {
            let end = i + line[i..].find(']').unwrap() + 1;
            line.replace_range(i..end, "");
        }
        out.push_str(&line.replace("outcomes performance P", "outcomes kernel Low"));
        out.push('\n');
    }
    out
}
pub fn scenario(name: &str) -> Option<&'static str> {
    (name == "horizon").then_some("Window")
}

pub fn quantile_source(base: &str, slow: bool) -> String {
    let src = append(
        &source(base),
        "property case.Done = reached(end.done)\nmetric quantile.DoneMedian = quantile(0.5, case.Done)\nmetric quantile.BadGiven = quantile(1, case.Bad given reached)",
    );
    if slow {
        edit(&src, "duration fixed 2min", "duration fixed 3min")
    } else {
        src
    }
}
