//! Review acceptance inputs. Expectations are derived in degradation_review_calculations.md.
#![allow(dead_code)]
use crate::input::{base, edit, source};

pub fn coverage(arrival: u64, long: bool) -> String {
    let s = edit(
        &source("calendar"),
        "[0min,3min) capacity 0; [3min,11min) capacity 1",
        "[0min,11min) capacity 1",
    );
    let s = edit(&s, "c = Clerk.enter(); end done", "c = Clerk.enter()");
    let s = edit(
        &s,
        "f = Fast.run(); end done }",
        "f = Fast.run() }; z = Finish.close(); end done",
    );
    let s = edit(
        &s,
        "  bounds",
        &format!("  arrivals fixed [{arrival}min]\n  bounds"),
    );
    if long {
        edit(&s, "duration fixed 3min", "duration fixed 20min")
    } else {
        s
    }
}

pub fn domain(declaration: &str) -> String {
    edit(&base(), "result nondet { ok, fail }", declaration)
}

pub fn probability(reverse: bool) -> String {
    let weights = if reverse {
        "0:ok; 1:fail"
    } else {
        "1:ok; 0:fail"
    };
    let s = edit(
        &base(),
        "  people",
        &format!("  kernel K {{ * -> {{ {weights} }} }}\n  people"),
    );
    edit(&s, "result nondet { ok, fail }", "outcomes kernel K")
}

pub fn multiple(two: bool) -> String {
    let s = edit(
        &base(),
        "  bounds",
        if two {
            "  arrivals fixed [0min,0min]\n  policy fifo tie_by case_id then call_order assign first_idle\n  bounds"
        } else {
            "  arrivals fixed [0min]\n  policy fifo tie_by case_id then call_order assign first_idle\n  bounds"
        },
    );
    if two {
        edit(&s, "cases 1", "cases 2")
    } else {
        s
    }
}

pub fn resource(impossible: bool) -> String {
    let s = edit(&base(), "  people", "  resource Seat capacity 1\n  people");
    edit(
        &s,
        "reads [Doc]; duration",
        if impossible {
            "reads [Doc]; uses Seat:2; duration"
        } else {
            "reads [Doc]; uses Seat:1; duration"
        },
    )
}

pub fn history(atomic: bool) -> String {
    let s = edit(
        &base(),
        "actor Clerk {",
        "actor Clerk { state marked: bool = false;",
    );
    edit(
        &s,
        "reads [Doc]; duration fixed 3min; result nondet { ok }",
        if atomic {
            "reads [Doc]; writes marked; duration fixed 3min; atomic { if marked == false { marked = true; result ok } else { result fail } }"
        } else {
            "reads [Doc]; duration fixed 3min; result nondet { ok, fail }"
        },
    )
}

/// Same observable process and finite probability question; conform's harness
/// removes only the probability query from both sides of its pair.
pub fn modes(degrade: bool) -> String {
    let s = domain("result nondet { ok }");
    let s = edit(
        &s,
        "  bounds",
        "  contract Controls { requires_event Fast.run.start after Probe.check.finish(ok) }\n  bounds",
    );
    let s = edit(
        &s,
        "verify reachable end.done",
        "verify probability reached end.done >= 1",
    );
    if degrade {
        s
    } else {
        edit(&s, crate::input::ROUTE, "if status == fail")
    }
}
