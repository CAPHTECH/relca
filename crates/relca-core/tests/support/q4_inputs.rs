//! Hand-authored ADR0059 models. The single-edit variants never read a report.
#![allow(dead_code)]

pub const RUNTIME: &[&str] = &[
    "open",
    "open-later",
    "continue",
    "continue-earlier-close",
    "shift",
    "shift-later",
    "resource",
    "resource-later",
    "ack",
    "ack-later",
    "horizon",
    "clock",
    "condition",
    "condition-later",
    "condition-open",
    "condition-open-later",
    "condition-true",
    "condition-deadlock",
    "condition-race",
    "condition-race-sim",
    "condition-sim",
    "drop",
    "drop-zero",
    "drop-zero-missing-rate",
    "explicit-rate",
    "free-overtime",
    "arrivals",
    "arrivals-burst",
    "arrivals-calendar",
    "fixed-burst",
    "fixed-spaced",
    "retry",
    "retry-later",
];

pub const STATIC: &[(&str, &str, &str)] = &[
    (
        "missing-close",
        "U-SEM-0",
        "calendar_close_behavior_required",
    ),
    ("suspend", "U-SEM-0", "calendar_preemption_unsupported"),
    ("gap", "E-CAP-0", "invalid_calendar_partition"),
    ("overlap", "E-CAP-0", "invalid_calendar_partition"),
    ("short-coverage", "U-SEM-0", "calendar_coverage_missing"),
    ("missing-target", "U-SEM-0", "calendar_target_unconfigured"),
    (
        "unselected-coverage",
        "U-SEM-0",
        "calendar_coverage_missing",
    ),
    ("unknown-calendar", "E-NAME-0", ""),
    ("latent-condition", "E-POL-LATENT", ""),
    ("bad-probability", "E-PROB-0", ""),
    ("missing-basis", "E-CON-0", "arrival_basis_missing"),
    ("wrong-count", "E-ARR-0", ""),
    (
        "arrivals-check",
        "U-SEM-0",
        "stochastic_arrivals_mode_unsupported",
    ),
    (
        "arrivals-explain",
        "U-SEM-0",
        "stochastic_arrivals_mode_unsupported",
    ),
];

pub fn edit(source: &str, from: &str, to: &str) -> String {
    assert_ne!(from, to);
    assert_eq!(source.matches(from).count(), 1, "one edit witness: {from}");
    source.replacen(from, to, 1)
}

pub fn source(name: &str) -> String {
    match name {
        "open" => include_str!("q4/open.relca").into(),
        "continue" => include_str!("q4/continue.relca").into(),
        "shift" => include_str!("q4/shift.relca").into(),
        "resource" => include_str!("q4/resource.relca").into(),
        "ack" => include_str!("q4/ack.relca").into(),
        "condition" => include_str!("q4/condition.relca").into(),
        "condition-true" => include_str!("q4/condition-constant.relca").into(),
        "arrivals" => include_str!("q4/arrivals.relca").into(),
        "drop" => include_str!("q4/drop.relca").into(),
        "retry" => include_str!("q4/retry.relca").into(),
        "open-later" => source("open").replace("5min", "6min"),
        "continue-earlier-close" => source("continue").replace("3min", "2min"),
        "shift-later" => edit(&source("shift"),
            "calendar Late through 21min on_close continue { [0min, 3min) capacity 0; [3min, 21min) capacity 1 }",
            "calendar Late through 21min on_close continue { [0min, 6min) capacity 0; [6min, 21min) capacity 1 }"),
        "resource-later" => source("resource").replace("4min", "5min"),
        "ack-later" => edit(&source("ack"), "ack_within 1min", "ack_within 2min"),
        "horizon" => edit(&source("open"), "arrivals fixed [0min]", "scenario Window { arrivals fixed [0min]; horizon 6min }"),
        "clock" => edit(&source("open"), "clock 20min", "clock 6min"),
        "condition-later" => edit(&source("condition"), "duration fixed 4min", "duration fixed 5min"),
        "condition-open" => edit(&source("condition"), "actor Desk { capacity 1; calendar Always",
            "calendar Day through 21min on_close continue { [0min, 6min) capacity 0; [6min, 21min) capacity 1 }\n  actor Desk { capacity 1; calendar Day"),
        "condition-open-later" => source("condition-open").replace("6min", "7min"),
        "condition-deadlock" => edit(&source("condition-true"), "ready: bool = true", "ready: bool = false"),
        "condition-race" => {
            let s = edit(&source("condition"),
                "capability set() -> R { requires requests == 0 else waiter; duration fixed 4min; result nondet { owner }; writes ready; start { requests += 1 }; finish { ready = true } }",
                "capability set() -> R { requires requests == 0 else waiter; duration fixed 4min; result nondet { owner }; writes ready; start { requests += 1 }; finish { ready = true } }\n    capability clear() -> R { duration fixed 1min; result nondet { ok }; start { ready = false } }");
            let s = edit(&s, "if s == owner { end done }", "if s == owner { c = Board.clear(); end done }");
            edit(&s, "duration fixed 2min", "duration fixed 1min")
        }
        "condition-race-sim" => edit(&source("condition-race"), "verify reachable end.done", "simulation replications 4 seed 1 confidence 0.5\n  verify probability reached end.done >= 0"),
        "condition-sim" => edit(&source("condition"), "verify reachable end.done", "simulation replications 4 seed 1 confidence 0.5\n  verify probability reached end.done >= 0"),
        "drop-zero-missing-rate" => edit(&source("drop"), "[1min, 21min) capacity 1", "[1min, 21min) capacity 0"),
        "drop-zero" => edit(&source("drop-zero-missing-rate"), "rate Desk per_minute 2;", "rate Desk per_minute 2; rate Desk overtime_per_minute 3;"),
        "explicit-rate" => edit(&source("continue"), "rate Desk per_minute 2;", "rate Desk per_minute 2; rate Desk overtime_per_minute 3;"),
        "free-overtime" => edit(&source("continue"), "rate Desk per_minute 2;", "rate Desk per_minute 2; rate Desk overtime_per_minute 0;"),
        "arrivals-burst" => edit(&source("arrivals"), "0.5: fixed [0min, 4min]", "0.5: fixed [0min, 1min]"),
        "arrivals-calendar" => edit(&source("arrivals"), "actor Desk { capacity 1",
            "calendar Day through 21min on_close continue { [0min, 5min) capacity 0; [5min, 21min) capacity 1 }\n  actor Desk { capacity 1; calendar Day"),
        "fixed-burst" | "fixed-spaced" => {
            let s = edit(&source("arrivals"),
                "arrivals distribution basis \"fixture assumption; not measured\" {\n    0.5: fixed [0min, 0min]\n    0.5: fixed [0min, 4min]\n  }",
                if name == "fixed-burst" { "arrivals fixed [0min, 0min]" } else { "arrivals fixed [0min, 4min]" });
            edit(&s, "verify probability reached end.done within 4min >= 0", "verify reachable end.done")
        }
        "retry-later" => source("retry").replace("4min", "5min"),
        "missing-close" => edit(&source("open"), " on_close continue", ""),
        "suspend" => edit(&source("open"), "on_close continue", "on_close suspend"),
        "gap" => edit(&source("open"), "    [0min, 5min) capacity 0\n", ""),
        "overlap" => edit(&source("open"), "[0min, 5min)", "[0min, 6min)"),
        "short-coverage" => source("open").replace("21min", "20min"),
        "missing-target" => edit(&source("open"), "; calendar Day", ""),
        "unknown-calendar" => edit(&source("open"), "; calendar Day", "; calendar Missing"),
        "unselected-coverage" => edit(&source("open"), "arrivals fixed [0min]",
            "scenario Window { arrivals fixed [0min]; bounds { cases 1; live_tasks 2; clock 20min } }\n  scenario Spare { bounds { cases 1; live_tasks 2; clock 30min } }")
            .replace("  bounds { cases 1; live_tasks 2; clock 20min }", "  ")
            .replace("window 20min", "window 30min"),
        "latent-condition" => edit(&source("condition-true"), "observed case", "latent case"),
        "bad-probability" => edit(&source("arrivals"), "0.5: fixed [0min, 0min]", "0.6: fixed [0min, 0min]"),
        "missing-basis" => edit(&source("arrivals"), " basis \"fixture assumption; not measured\"", ""),
        "wrong-count" => edit(&source("arrivals"), "0.5: fixed [0min, 4min]", "0.5: fixed [0min]"),
        "arrivals-check" | "arrivals-explain" => source("arrivals"),
        _ => panic!("unclassified Q4 input {name}"),
    }
}

pub fn command(name: &str) -> &'static str {
    match name {
        "arrivals" | "arrivals-burst" | "arrivals-calendar" | "condition-sim"
        | "condition-race-sim" | "bad-probability" | "missing-basis" | "wrong-count" => "simulate",
        "arrivals-explain" => "explain",
        _ => "check",
    }
}

pub fn scenario(name: &str) -> Option<&'static str> {
    matches!(name, "horizon" | "unselected-coverage").then_some("Window")
}

/// Old-language substrate control, NOT an oracle for the new semantics.
pub fn substrate(source: &str) -> String {
    let mut result = String::new();
    let mut calendar = false;
    for line in source.lines() {
        let t = line.trim();
        if t.starts_with("calendar ") && t.contains(" through ") {
            calendar = !t.contains('}');
            continue;
        }
        if calendar {
            if t.contains('}') {
                calendar = false;
            }
            continue;
        }
        if t.starts_with("person_calendar ") {
            continue;
        }
        result.push_str(line);
        result.push('\n');
    }
    for calendar in ["Day", "Always", "Early", "Late", "Two"] {
        result = result.replace(&format!("; calendar {calendar}"), "");
        result = result.replace(&format!(" calendar {calendar}"), "");
    }
    for target in ["Desk", "B"] {
        for rate in [0, 3] {
            result = result.replace(&format!("rate {target} overtime_per_minute {rate}; "), "");
        }
    }
    result = result.replace("wait until Board.ready == true", "wait 4min");
    result.replace("wait until ready == true; ", "")
}

pub const PAIRS: &[(&str, &str, &str, &str)] = &[
    ("open", "gap", "    [0min, 5min) capacity 0\n", ""),
    ("open", "missing-close", " on_close continue", ""),
    ("open", "suspend", "on_close continue", "on_close suspend"),
    ("open", "missing-target", "; calendar Day", ""),
    (
        "continue",
        "explicit-rate",
        "rate Desk per_minute 2;",
        "rate Desk per_minute 2; rate Desk overtime_per_minute 3;",
    ),
    (
        "continue",
        "free-overtime",
        "rate Desk per_minute 2;",
        "rate Desk per_minute 2; rate Desk overtime_per_minute 0;",
    ),
    (
        "drop",
        "drop-zero-missing-rate",
        "[1min, 21min) capacity 1",
        "[1min, 21min) capacity 0",
    ),
    (
        "arrivals",
        "arrivals-burst",
        "0.5: fixed [0min, 4min]",
        "0.5: fixed [0min, 1min]",
    ),
    (
        "condition",
        "condition-later",
        "duration fixed 4min",
        "duration fixed 5min",
    ),
];
