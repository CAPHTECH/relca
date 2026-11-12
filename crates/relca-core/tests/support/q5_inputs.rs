//! ADR0062 hand-written finite models; no reports or binary captures are read.
#![allow(dead_code)]

pub const PRIORITY: &str = "policy observable priority descending";
pub const FIFO_TIE: &str = " tie fifo tie_by case_id";
pub const RESOLVE: &str = " resolve priority descending";

pub const RUNTIME: &[&str] = &[
    "start",
    "start-tie",
    "equal",
    "equal-explicit",
    "equal-fifo",
    "equal-sim",
    "call-order",
    "call-order-reverse",
    "people",
    "people-environment",
    "calendar",
    "calendar-later",
    "external",
    "external-fifo",
    "external-equal",
    "external-equal-fifo",
    "external-equal-sim",
    "initial",
    "conflict",
    "conflict-no-resolve",
    "conflict-equal",
    "conflict-id",
    "future",
    "future-tied",
    "stale",
    "unrelated",
    "ack",
    "clock",
    "horizon",
    "actor-hol",
    "resource-hol",
    "independent-hol",
    "unattributed",
    "physical",
    "retry",
    "retry-unknown",
    "unused",
    "max-rank",
    "label",
    "scenario",
    "explain-start",
    "explain-external",
    "explain-conflict",
];

pub const STATIC: &[(&str, &str, &str)] = &[
    ("short-table", "E-ARR-0", "priority_case_count_mismatch"),
    ("duplicate-table", "U-SEM-0", "duplicate_case_priority"),
    ("missing-table", "U-SEM-0", "priority_not_configured"),
    ("ascending", "U-SYN-0", ""),
    ("dynamic-latent", "U-SYN-0", ""),
    ("nondet-call-order", "U-SEM-0", "priority_tie_incompatible"),
    ("two-ties", "U-SEM-0", ""),
    ("zero-external", "E-CON-0", ""),
    (
        "disagree-limits",
        "U-SEM-0",
        "queue and concurrent limits disagree on overflow",
    ),
    (
        "environment-resolve",
        "U-SEM-0",
        "priority_resolution_scope_unsupported",
    ),
    (
        "one-sided-resolve",
        "U-SEM-0",
        "priority_resolution_mismatch",
    ),
    (
        "unselected-count",
        "E-ARR-0",
        "priority_case_count_mismatch",
    ),
    ("version-retry", "U-SEM-0", "retry_feature_combination"),
];

pub const PAIRS: &[(&str, &str, &str, &str)] = &[
    ("start", "start-tie", "[0, 10, 20]", "[0, 10, 0]"),
    (
        "equal",
        "equal-explicit",
        PRIORITY,
        "policy observable priority descending tie nondet",
    ),
    (
        "equal",
        "equal-fifo",
        PRIORITY,
        "policy observable priority descending tie fifo tie_by case_id",
    ),
    (
        "calendar",
        "calendar-later",
        "[0min, 4min) capacity 0; [4min, 21min)",
        "[0min, 5min) capacity 0; [5min, 21min)",
    ),
    (
        "people",
        "people-environment",
        "actor B { control policy",
        "actor B { control environment",
    ),
    (
        "external",
        "external-fifo",
        "wait_external priority descending",
        "wait_external fifo",
    ),
    (
        "external-equal",
        "external-equal-fifo",
        "wait_external priority descending",
        "wait_external priority descending tie fifo tie_by case_id",
    ),
    ("conflict", "conflict-no-resolve", RESOLVE, ""),
    (
        "conflict-equal",
        "conflict-id",
        RESOLVE,
        " resolve priority descending tie_by case_id",
    ),
    (
        "future",
        "future-tied",
        "duration fixed 3min",
        "duration fixed 2min",
    ),
    (
        "unattributed",
        "physical",
        "capability small() -> R {",
        "capability small() -> R { uses R: 1;",
    ),
    (
        "retry",
        "retry-unknown",
        "[unknown, delivered]",
        "[unknown, unknown]",
    ),
];

pub fn edit(s: &str, from: &str, to: &str) -> String {
    assert_ne!(from, to);
    assert_eq!(s.matches(from).count(), 1, "single-edit witness {from}");
    s.replacen(from, to, 1)
}
pub fn append(s: &str, item: &str) -> String {
    let i = s.rfind('}').unwrap();
    format!("{}\n{item}\n}}", &s[..i])
}
fn simple(n: usize, arrivals: &str, ranks: &str, policy: &str) -> String {
    format!(
        r#"model Priority version "0.2" {{
      type R = enum {{ ok }}
      actor A {{ capacity 1; capability run() -> R {{ duration fixed 2min; result nondet {{ ok }} }} }}
      process Flow {{ r = A.run(); end done }}
      cut C {{ include [A] }}
      bounds {{ cases {n}; live_tasks 2; clock 20min }}
      arrivals fixed [{arrivals}]
      case_priority fixed [{ranks}]
      {policy}
      metrics {{ window 20min; currency "JPY"; breakdown waits }}
      verify safety no_resource_overcommit
      verify probability reached end.done within 2min >= 0
    }}"#
    )
}
fn external() -> String {
    let s = simple(
        3,
        "0min, 1min, 2min",
        "0, 1, 9",
        "policy fifo tie_by case_id",
    );
    let s = edit(&s, "duration fixed 2min", "duration fixed 4min");
    append(
        &s,
        "concurrent cases 1 on_overflow wait_external priority descending",
    )
}
fn conflict() -> String {
    format!(
        r#"model Conflict version "0.2" {{
      type R = enum {{ committed, conflict }} type H = enum {{ free, taken }}
      actor A {{ capacity 2; state holder: H = free
        capability claim() -> R {{ duration fixed 2min; result nondet {{ committed }};
          version_check [holder] else conflict{RESOLVE}; commits;
          writes holder; finish {{ holder = taken }} }} }}
      process Flow {{ r = A.claim(); if r == committed {{ end won }} else {{ end lost }} }}
      cut C {{ include [A] }} bounds {{ cases 2; live_tasks 2; clock 20min }}
      arrivals fixed [0min, 0min] case_priority fixed [0, 9]
      policy fifo tie_by case_id
      metrics {{ window 20min; currency "JPY"; breakdown waits }}
      verify safety no_resource_overcommit
      verify probability reached end.won >= 0
    }}"#
    )
}

// Arrival0 picks first; subsequent arrivals receive second without starting.
// This is observed state + requires, never a new case.id ordering expression.
const SELECTOR: &str = r#"actor Gate { control environment; capacity 1; state used: 0..1 = 0
    capability pick() -> R { requires used == 0 else second; start { used += 1 };
      duration fixed 1min; result nondet { first } } }"#;

fn people() -> String {
    format!(r#"model People version "0.2" {{ type R = enum {{ first, second, ok }} people [p]
      calendar Always through 21min on_close continue {{ [0min, 21min) capacity 1 }}
      calendar Day through 21min on_close continue {{ [0min, 4min) capacity 0; [4min, 21min) capacity 1 }}
      {SELECTOR}
      actor A {{ control policy; executors [p]; calendar Day
        capability low() -> R {{ duration fixed 2min; result nondet {{ ok }} }} }}
      actor B {{ control policy; executors [p]; calendar Day
        capability high() -> R {{ duration fixed 3min; result nondet {{ ok }} }} }}
      person_calendar p Day
      process Flow {{ role = Gate.pick(); if role == first {{ low = A.low(); end done }}
        else {{ high = B.high(); end done }} }}
      cut C {{ include [Gate, A, B] }} bounds {{ cases 2; live_tasks 2; clock 20min }}
      arrivals fixed [0min, 2min] case_priority fixed [0, 9]
      policy observable priority descending assign first_idle
      metrics {{ window 20min; currency "JPY"; breakdown waits }}
      verify safety no_resource_overcommit
    }}"#).replace("capacity 1; state used:", "capacity 1; calendar Always; state used:")
}
fn future() -> String {
    format!(
        r#"model Future version "0.2" {{ type R = enum {{ first, second, committed, conflict }}
      type H = enum {{ free, taken }} {SELECTOR}
      actor A {{ capacity 2; state holder: H = free
        capability low() -> R {{ duration fixed 2min; result nondet {{ committed }};
          version_check [holder] else conflict{RESOLVE}; commits; writes holder; finish {{ holder = taken }} }}
        capability high() -> R {{ duration fixed 3min; result nondet {{ committed }};
          version_check [holder] else conflict{RESOLVE}; commits; writes holder; finish {{ holder = taken }} }} }}
      process Flow {{ role = Gate.pick(); if role == first {{ low = A.low();
          if low == committed {{ end won }} else {{ end lost }} }}
        else {{ high = A.high(); if high == committed {{ end won }} else {{ end lost }} }} }}
      cut C {{ include [Gate, A] }} bounds {{ cases 2; live_tasks 2; clock 20min }}
      arrivals fixed [0min, 0min] case_priority fixed [0, 9] policy fifo tie_by case_id
      metrics {{ window 20min; currency "JPY"; breakdown waits }}
      verify safety no_resource_overcommit verify probability reached end.won >= 0
    }}"#
    )
}
fn stale() -> String {
    format!(
        r#"model Stale version "0.2" {{ type R = enum {{ first, second, third, committed, conflict }}
      type H = enum {{ free, taken }}
      actor Gate {{ capacity 1; state used: 0..1 = 0; state next: 0..1 = 0
        capability pick() -> R {{ requires used == 0 else second; start {{ used += 1 }};
          duration fixed 1min; result nondet {{ first }} }}
        capability next() -> R {{ requires next == 0 else third; start {{ next += 1 }};
          duration fixed 1min; result nondet {{ second }} }} }}
      actor A {{ capacity 3; state holder: H = free
        capability high() -> R {{ duration fixed 4min; result nondet {{ committed }};
          version_check [holder] else conflict{RESOLVE}; commits; writes holder; finish {{ holder = taken }} }}
        capability middle() -> R {{ duration fixed 1min; result nondet {{ committed }};
          version_check [holder] else conflict{RESOLVE}; commits; writes holder; finish {{ holder = taken }} }}
        capability low() -> R {{ duration fixed 1min; result nondet {{ committed }};
          version_check [holder] else conflict{RESOLVE}; commits; writes holder; finish {{ holder = taken }} }} }}
      process Flow {{ role = Gate.pick(); if role == first {{ high = A.high();
          if high == committed {{ end won }} else {{ end lost }} }}
        else {{ next = Gate.next(); if next == second {{ middle = A.middle();
            if middle == committed {{ end won }} else {{ end lost }} }}
          else {{ low = A.low(); if low == committed {{ end won }} else {{ end lost }} }} }} }}
      cut C {{ include [Gate, A] }} bounds {{ cases 3; live_tasks 2; clock 20min }}
      arrivals fixed [0min, 1min, 4min] case_priority fixed [9, 5, 0] policy fifo tie_by case_id
      metrics {{ window 20min; currency "JPY"; breakdown waits }}
      verify safety no_resource_overcommit verify probability reached end.won >= 0
    }}"#
    )
}
fn unrelated() -> String {
    format!(
        r#"model Unrelated version "0.2" {{ type R = enum {{ first, second, committed, conflict }}
      {SELECTOR}
      actor A {{ capacity 2; state x: bool = false; state y: bool = false
        capability low() -> R {{ duration fixed 2min; result nondet {{ committed }};
          version_check [x] else conflict{RESOLVE}; writes x; finish {{ x = true }} }}
        capability high() -> R {{ duration fixed 2min; result nondet {{ committed }};
          version_check [y] else conflict{RESOLVE}; writes y; finish {{ y = true }} }} }}
      process Flow {{ role = Gate.pick(); if role == first {{ low = A.low(); end done }}
        else {{ high = A.high(); end done }} }}
      cut C {{ include [Gate, A] }} bounds {{ cases 2; live_tasks 2; clock 20min }}
      arrivals fixed [0min, 0min] case_priority fixed [0, 9] policy fifo tie_by case_id
      contract Order {{ invariant A.x == true -> A.y == true }}
      verify safety no_resource_overcommit
    }}"#
    )
}
fn ack() -> String {
    format!(
        r#"model AckRace version "0.2" {{ type R = enum {{ pass, conflict }}
      actor Fast {{ capacity 1; capability run() -> R {{ duration fixed 1min; result nondet {{ pass }};
        cancel forbidden_after_start }} }}
      actor A {{ capacity 1; state done: bool = false
        capability claim() -> R {{ duration fixed 2min; result nondet {{ pass }};
          version_check [done] else conflict{RESOLVE}; writes done; finish {{ done = true }};
          cancel cooperative ack_within 1min }} }}
      process Flow {{ fork f {{ fast = Fast.run(); slow = A.claim() }}
        join any f where result == pass tie by [fast, slow] remaining cancel_then_drain if none -> failed
        if fast == pass {{ end done }} else {{ end failed }} }}
      cut C {{ include [Fast, A] }} bounds {{ cases 1; live_tasks 2; clock 20min }}
      arrivals fixed [0min] case_priority fixed [9] policy fifo tie_by case_id then call_order
      verify probability reached A.done == true >= 0
    }}"#
    )
}
fn hol(resource: bool, independent: bool) -> String {
    let actor = if resource || independent { "B" } else { "A" };
    let uses = if resource { "uses R: 1;" } else { "" };
    format!(
        r#"model Head version "0.2" {{ type R = enum {{ ok }}
      calendar Day through 21min on_close continue {{ [0min, 4min) capacity 0; [4min, 21min) capacity 2 }}
      calendar Zero through 21min on_close continue {{ [0min, 21min) capacity 0 }}
      calendar ResourceDay through 21min on_close continue {{ [0min, 4min) capacity 0; [4min, 21min) capacity 1 }}
      resource R capacity 1 calendar ResourceDay
      actor A {{ capacity 2; calendar Day;
        capability blocked() -> R {{ uses R: 2; duration fixed 1min; result nondet {{ ok }} }}
        capability small() -> R {{ duration fixed 1min; result nondet {{ ok }} }} }}
      actor B {{ capacity 2; calendar Day;
        capability small() -> R {{ {uses} duration fixed 1min; result nondet {{ ok }} }} }}
      process Flow {{ fork f {{ first = A.blocked(); last = {actor}.small() }} join all f; end done }}
      cut C {{ include [A, B] }} bounds {{ cases 1; live_tasks 2; clock 20min }}
      arrivals fixed [0min] case_priority fixed [9]
      policy observable priority descending tie fifo tie_by case_id then call_order
      metrics {{ window 20min; currency "JPY"; breakdown waits }}
      verify reachable end.done
    }}"#
    )
}
fn unattributed() -> String {
    format!(
        r#"model WaitCause version "0.2" {{ type R = enum {{ first, second, ok }} {SELECTOR}
      resource R capacity 1
      actor Blocker {{ control environment; capacity 1;
        capability hold() -> R {{ uses R: 1; duration fixed 3min; result nondet {{ ok }} }} }}
      actor A {{ capacity 2;
        capability big() -> R {{ uses R: 1; duration fixed 1min; result nondet {{ ok }} }}
        capability small() -> R {{ duration fixed 1min; result nondet {{ ok }} }} }}
      process Flow {{ role = Gate.pick(); if role == first {{ r = Blocker.hold(); end done }}
        else {{ fork f {{ big = A.big(); small = A.small() }} join all f; end done }} }}
      cut C {{ include [Gate, Blocker, A] }} bounds {{ cases 2; live_tasks 2; clock 20min }}
      arrivals fixed [0min, 2min] case_priority fixed [0, 9]
      policy observable priority descending tie fifo tie_by case_id then call_order
      metrics {{ window 20min; currency "JPY"; breakdown waits }}
      verify safety no_resource_overcommit
    }}"#
    )
}
fn retry() -> String {
    r#"model RetryPriority version "0.2" {
      type Ack = enum { committed, rejected_duplicate }
      observed case { registered: bool = false }
      actor A { capacity 1; state keys: set<CaseId>
        capability post(key: CaseId) -> Ack { duration fixed 1min; commits;
          writes [keys, registered]; idempotency key key
          atomic { if key in keys { result rejected_duplicate }
            else { keys += key; registered = true; result committed } }
          cancel forbidden_after_start } }
      process Flow { ack = A.post(case.id) response_loss sequence [unknown, delivered]
        retry max_attempts 2 when receipt unknown backoff fixed 0min exhausted end failed
        if ack == rejected_duplicate { end duplicate } else { end posted } }
      cut C { include [A] } bounds { cases 2; live_tasks 2; clock 20min }
      arrivals fixed [0min, 0min] case_priority fixed [0, 9]
      policy observable priority descending
      contract Once { at_most_once A.post.commit per case.id }
      verify probability reached registered == true >= 1
    }"#
    .into()
}
pub fn source(name: &str) -> String {
    match name {
        "start" => simple(3, "0min, 0min, 1min", "0, 10, 20", PRIORITY),
        "equal" => simple(2, "0min, 0min", "5, 5", PRIORITY),
        "call-order" => r#"model CallOrder version "0.2" { type R = enum { ok }
          actor A { capacity 1; capability short() -> R { duration fixed 2min; result nondet { ok } }
            capability long() -> R { duration fixed 3min; result nondet { ok } } }
          process Flow { fork f { short = A.short(); long = A.long() } join all f; end done }
          cut C { include [A] } bounds { cases 1; live_tasks 2; clock 20min }
          arrivals fixed [0min] case_priority fixed [7]
          policy observable priority descending tie fifo tie_by case_id then call_order
          metrics { window 20min; currency "JPY"; breakdown waits }
          verify safety no_resource_overcommit }"#.into(),
        "call-order-reverse" => edit(&source("call-order"), "short = A.short(); long = A.long()", "long = A.long(); short = A.short()"),
        "people" => people(), "external" => external(), "conflict" => conflict(),
        "calendar" => {
            let s = simple(2, "0min, 1min", "0, 10", PRIORITY);
            let s = edit(&s, "actor A { capacity 1;", "actor A { capacity 1; calendar Day;");
            append(&s, "calendar Day through 21min on_close continue { [0min, 4min) capacity 0; [4min, 21min) capacity 1 }")
        }
        "external-equal" => edit(&external(), "[0, 1, 9]", "[0, 1, 1]"),
        "initial" => append(&simple(2, "0min, 0min", "0, 9", "policy fifo tie_by case_id"),
            "concurrent cases 1 on_overflow wait_external priority descending"),
        "conflict-equal" => edit(&conflict(), "[0, 9]", "[5, 5]"),
        "future" => future(), "stale" => stale(), "unrelated" => unrelated(), "ack" => ack(),
        "clock" => append(&edit(&external(), "clock 20min", "clock 6min"), "verify reachable end.done"),
        "horizon" => append(&edit(&external(), "arrivals fixed [0min, 1min, 2min]", "scenario Window { arrivals fixed [0min, 1min, 2min]; horizon 6min }"), "verify reachable end.done"),
        "actor-hol" => hol(false, false), "resource-hol" => hol(true, false), "independent-hol" => hol(false, true),
        "unattributed" => unattributed(), "retry" => retry(),
        "equal-sim" | "external-equal-sim" => {
            let base = if name == "equal-sim" { "equal" } else { "external-equal" };
            append(&source(base), "simulation replications 2 seed 1 confidence 0.5")
        }
        "unused" => simple(2, "0min, 0min", "0, 9", "policy fifo tie_by case_id"),
        "max-rank" => simple(2, "0min, 0min", "0, 18446744073709551615", PRIORITY),
        "label" => source("start").replace("actor A {", "actor Desk { grain document;").replace("A.run", "Desk.run").replace("include [A]", "include [Desk]"),
        "scenario" => edit(&source("equal-fifo"), "arrivals fixed [0min, 0min]", "scenario Window { arrivals fixed [0min, 0min] }"),
        "explain-start" => append(&source("start"), "contract Deadline { response case.received -> one_of(end.done) by 4min after case.received otherwise unresolved }"),
        "explain-external" => append(&external(), "contract Deadline { response case.received -> one_of(end.done) by 5min after case.received otherwise unresolved }"),
        "explain-conflict" => append(&conflict(), "contract Guard { invariant A.holder == free }"),
        "short-table" => edit(&source("start"), "[0, 10, 20]", "[0, 10]"),
        "duplicate-table" => append(&source("start"), "case_priority fixed [0, 10, 20]"),
        "missing-table" => edit(&source("start"), "case_priority fixed [0, 10, 20]", ""),
        "ascending" => edit(&source("start"), "priority descending", "priority ascending"),
        "dynamic-latent" => edit(&source("start"), "case_priority fixed [0, 10, 20]", "latent case { secret: bool = false } case_priority by secret"),
        "nondet-call-order" => edit(&source("equal"), PRIORITY, "policy observable priority descending then call_order"),
        "two-ties" => edit(&source("equal"), PRIORITY, "policy observable priority descending tie nondet tie fifo tie_by case_id"),
        "zero-external" => edit(&external(), "concurrent cases 1", "concurrent cases 0"),
        "disagree-limits" => append(&external(), "queue capacity 1 on_overflow wait_external fifo"),
        "environment-resolve" => edit(&conflict(), "actor A {", "actor A { control environment;"),
        "one-sided-resolve" => edit(&future(), &format!("duration fixed 3min; result nondet {{ committed }};\n          version_check [holder] else conflict{RESOLVE}"), "duration fixed 3min; result nondet { committed };\n          version_check [holder] else conflict"),
        "unselected-count" => append(&source("scenario"), "scenario Bad { bounds { cases 3; live_tasks 2; clock 20min } arrivals fixed [0min, 0min, 0min] }"),
        "version-retry" => {
            let s = conflict();
            edit(&s, "r = A.claim();", "r = A.claim() response_loss none retry max_attempts 2 when result [conflict] backoff fixed 0min exhausted end lost;")
        }
        _ => {
            let &(base, _, from, to) = PAIRS.iter().find(|p| p.1 == name).unwrap_or_else(|| panic!("unknown Q5 input {name}"));
            edit(&source(base), from, to)
        }
    }
}

/// Explicit counterfactual substrate, not a projection of production output.
/// It removes only the three ADR0062 opt-ins, retaining other semantic features.
pub fn substrate(name: &str) -> String {
    let s = source(name);
    let mut out = String::new();
    for line in s.lines() {
        let mut line = line.to_string();
        if let Some(start) = line.find("case_priority fixed [") {
            let end = start + line[start..].find(']').unwrap() + 1;
            line.replace_range(start..end, "");
        }
        out.push_str(&line);
        out.push('\n');
    }
    for suffix in [
        " tie fifo tie_by case_id then call_order",
        " tie fifo tie_by case_id",
        " tie nondet",
        "",
    ] {
        out = out.replace(
            &format!("policy observable priority descending{suffix}"),
            if suffix.ends_with("then call_order") {
                "policy observable fifo tie_by case_id then call_order"
            } else {
                "policy observable fifo tie_by case_id"
            },
        );
    }
    out = out
        .replace(
            "wait_external priority descending tie fifo tie_by case_id",
            "wait_external fifo",
        )
        .replace("wait_external priority descending", "wait_external fifo")
        .replace(" resolve priority descending tie_by case_id", "")
        .replace(RESOLVE, "");
    out
}
pub fn command(name: &str) -> &'static str {
    if name.ends_with("-sim") {
        "simulate"
    } else if name.starts_with("explain-") {
        "explain"
    } else {
        "check"
    }
}
pub fn scenario(name: &str) -> Option<&'static str> {
    if matches!(name, "horizon" | "scenario" | "unselected-count") {
        Some("Window")
    } else {
        None
    }
}
