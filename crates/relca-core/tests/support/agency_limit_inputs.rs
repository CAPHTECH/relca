//! Finite models and hand oracles; see agency_limit_calculations.md.
#![allow(dead_code)]

pub fn model(body: &str, extra: &str, clock: usize) -> String {
    format!(
        r#"model AgencyLimit version "0.2" {{
  type V = enum {{ pass, fail }}
  type Ack = enum {{ done }}
  actor Doc {{ source document }}
  actor Intake {{ capacity 1; capability start() -> V {{ reads [Doc]; duration fixed 1min; result nondet {{ pass }} }} }}
  actor Judge {{ capacity 1; capability check(x:V) -> V {{ duration fixed 1min; result nondet {{ pass, fail }} }} }}
  actor Ledger {{ capacity 1; capability post(x:V) -> Ack {{ commits; duration fixed 1min; result nondet {{ done }} }} }}
  {extra}
  process Flow {{
{body}
  }}
  cut Scope {{ include [Doc,Intake,Judge,Ledger] }}
  bounds {{ cases 1; live_tasks 2; clock {clock}min }}
  verify reachable end.done
  verify safety no_resource_overcommit
}}
"#
    )
}

pub fn gate(k: usize) -> String {
    let mut b = "    s = Intake.start()\n".to_owned();
    for i in 1..=k {
        b += &format!(
            "    j{i} = Judge.check(s)\n    if j{i} == pass {{ }} else {{ end rejected }}\n"
        );
    }
    b += &format!("    p = Ledger.post(j{k})\n    end done");
    model(&b, "", 8 * k + 44)
}

pub fn joined(k: usize, op: &str) -> String {
    let mut b = "    s = Intake.start()\n".to_owned();
    for i in 1..=k {
        b += &format!("    j{i} = Judge.check(s)\n");
    }
    let c = (1..=k)
        .map(|i| format!("j{i} == pass"))
        .collect::<Vec<_>>()
        .join(op);
    b += &format!(
        "    if {c} {{ }} else {{ end rejected }}\n    p = Ledger.post(j{k})\n    end done"
    );
    model(&b, "", 4 * k + 48)
}

pub fn work() -> String {
    joined(11, " || ")
}

pub fn violation(contract: bool) -> String {
    let s = work().replace("    end done", "    p2 = Ledger.post(j11)\n    end done");
    if contract {
        s.replace(
            "  verify reachable",
            "  contract Once { at_most_once Ledger.post.commit per case.id }\n  verify reachable",
        )
    } else {
        s
    }
}

pub fn continuing(k: usize) -> String {
    gate(k).replace("else { end rejected }", "else { }")
}

pub fn requires(k: usize) -> String {
    let mut b = "    s = Intake.start()\n".to_owned();
    for i in 1..=k {
        b += &format!("    g{i} = Gate.check(s)\n");
    }
    b += &format!("    p = Ledger.post(g{k})\n    end done");
    model(&b, "actor Gate { capacity 1; state permit: V = pass; capability check(x:V) -> V { requires permit == pass else fail; duration fixed 1min; result nondet { pass } } }", 4 * k + 44)
        .replace("[Doc,Intake,Judge,Ledger]", "[Doc,Intake,Judge,Ledger,Gate]")
}

pub fn chain(k: usize) -> String {
    let mut b = "    s = Intake.start()\n    j0 = Judge.check(s)\n".to_owned();
    for i in 1..=k {
        b += &format!("    j{i} = Judge.check(j{})\n", i - 1);
    }
    b += &format!("    p = Ledger.post(j{k})\n    end done");
    model(&b, "", 4 * k + 48).replace("result nondet { pass, fail }", "result nondet { pass }")
}

pub fn configs() -> String {
    let small = "process SmallFlow { s = Intake.start(); j1 = Judge.check(s); if j1 == pass {} else {end rejected}; j2 = Judge.check(s); if j2 == pass {} else {end rejected}; p = Ledger.post(j2); end done }";
    work().replace("  cut Scope", &format!("  {small}\n  configuration Small {{ cut Scope; process SmallFlow }}\n  configuration Big {{ cut Scope; process Flow }}\n  cut Scope"))
}

pub fn simulation() -> String {
    work().replace("  actor Doc", "  latent case { coin: bool = false }\n  kernel Coin given [coin] { false -> { 0.5:pass; 0.5:fail }; true -> { 0.5:pass; 0.5:fail } }\n  actor Doc")
        .replace("result nondet { pass, fail }", "kernel_reads latent [coin]; outcomes kernel Coin")
        .replace("  verify reachable", "  simulation replications 4 seed 1 confidence 0.95\n  verify reachable")
}

pub fn degradation() -> String {
    work()
        .replace("    end done", "    degrade Manual when j11 == fail { slow = Slow.run(); end done } else { fast = Fast.run(); end done }")
        .replace("  process Flow", "  actor Slow { capacity 1; capability run() -> Ack { duration fixed 3min; result nondet { done } } }\n  actor Fast { capacity 1; capability run() -> Ack { duration fixed 1min; result nondet { done } } }\n  process Flow")
        .replace("[Doc,Intake,Judge,Ledger]", "[Doc,Intake,Judge,Ledger,Slow,Fast]")
        .replace("clock 92min", "clock 15min")
        .replace("  verify safety no_resource_overcommit\n", "")
}

pub fn contest() -> String {
    work()
        .replace("  actor Doc", "  people [j,r]\n  actor Doc")
        .replace("actor Judge { capacity 1", "actor Judge { executors [j]")
        .replace("  process Flow", "  actor Party { control party; standing over [end.rejected] basis \"claim\" }\n  actor Route { executors [r]; capability respond() -> V { duration fixed 2min; result nondet { pass } } }\n  permit j to Judge.check\n  permit r to Route.respond\n  authority Grant { basis \"decision\"; grants j to Judge.check }\n  contract Controls { contest Party on end.rejected route Route.respond reversible_by [j] respond_by 2min }\n  process Flow")
        .replace("[Doc,Intake,Judge,Ledger]", "[Doc,Intake,Judge,Ledger,Party,Route]")
}

pub fn all() -> Vec<(String, String, Option<&'static str>)> {
    let mut v = vec![
        ("agy-limit-control".into(), gate(13), None),
        ("agy-limit-work".into(), work(), None),
        ("agy-limit-paths".into(), requires(13), None),
        ("agy-limit-paths-control".into(), requires(12), None),
        ("agy-limit-violation".into(), violation(true), None),
        (
            "agy-limit-violation.broken-no-contract".into(),
            violation(false),
            None,
        ),
        ("agy-limit-chain".into(), chain(986), None),
        ("agy-limit-cont".into(), continuing(3), None),
        ("agy-limit-degradation".into(), degradation(), None),
        ("agy-limit-contest".into(), contest(), None),
        ("agy-limit-simulation".into(), simulation(), None),
        ("agy-limit-configs-small".into(), configs(), Some("Small")),
        ("agy-limit-configs-big".into(), configs(), Some("Big")),
    ];
    for k in [14, 40, 200] {
        v.push((format!("agy-limit-c1-{k}"), gate(k), None));
    }
    v.push(("agy-limit-c1-and".into(), joined(20, " && "), None));
    for k in [3, 10] {
        v.push((format!("agy-limit-any-{k}"), joined(k, " || "), None));
    }
    v
}
