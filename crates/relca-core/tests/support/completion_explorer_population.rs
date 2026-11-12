//! DSL and handwritten semantic models. No production output is an expectation.
use super::completion_explorer::*;
pub struct Example {
    pub name: &'static str,
    pub source: String,
    pub own: OwnModel,
    pub hand_terminals: Vec<(&'static str, u64, u64)>,
    pub deadlock: bool,
    pub cut: bool,
}
fn location(source: &str, text: &str) -> Location {
    let start = source
        .find(text)
        .unwrap_or_else(|| panic!("missing {text}"));
    assert_eq!(
        source.matches(text).count(),
        1,
        "unique source anchor {text}"
    );
    (start, start + text.len())
}
fn cap(name: &str, duration: u64, results: &str, uses: bool) -> String {
    format!(
        "capability {name}() -> V {{ duration fixed {duration}min; result nondet {{ {results} }}; cancel forbidden_after_start{} }}",
        if uses { "; uses R: 1" } else { "" }
    )
}
fn source(declarations: &str, body: &str, clock: u64, ends: &[&str]) -> String {
    format!(
        "model Independent version \"0.2\" {{\ntype V = enum {{ pass, fail }}\npeople [p,q]\n{declarations}\nprocess P {{ {body} }}\nbounds {{ cases 1; live_tasks 3; clock {clock}min }}\nverify recoverable end\nverify terminating end\nverify almost_sure end\n{}\n}}",
        ends.iter()
            .map(|e| format!("verify reachable end.{e}"))
            .collect::<Vec<_>>()
            .join("\n")
    )
}
fn actor(name: &str, capacity: u64, people: Option<&[&str]>) -> OwnActor {
    OwnActor {
        name: name.into(),
        capacity,
        people: people.map(|ps| ps.iter().map(|p| (*p).into()).collect()),
    }
}
fn own(
    source: &str,
    actors: Vec<OwnActor>,
    caps: &[(&str, usize, u64, &str, bool)],
    bindings: &[&str],
    resource: Option<u64>,
    clock: u64,
) -> OwnModel {
    let roster = source.find("people [p,q]").unwrap();
    OwnModel {
        actors,
        capabilities: caps
            .iter()
            .map(|(name, a, t, results, uses)| OwnCapability {
                name: format!("{}.{name}", if *a == 1 { "B" } else { "A" }),
                actor: *a,
                duration: *t,
                results: results.split(", ").map(str::to_owned).collect(),
                uses: u64::from(*uses),
                location: location(source, &cap(name, *t, results, *uses)),
            })
            .collect(),
        people: vec![
            ("p".into(), (roster + 8, roster + 9)),
            ("q".into(), (roster + 10, roster + 11)),
        ],
        resource: resource.map(|r| ("R".into(), r)),
        bindings: bindings.iter().map(|b| (*b).into()).collect(),
        body: vec![],
        clock,
    }
}
fn end(source: &str, name: &str) -> Op {
    Op::End(name.into(), location(source, &format!("end {name}")))
}
pub fn population() -> Vec<Example> {
    let mut examples = vec![];
    for (name, results, capacity, resource, clock) in [
        ("both", "pass, fail", 1, None, 10),
        ("executor-pool", "pass, fail", 2, None, 10),
        ("anonymous-one", "pass, fail", 1, None, 10),
        ("pass-only", "pass", 1, None, 10),
        ("capacity-zero", "pass, fail", 0, None, 10),
        ("resource-one", "pass, fail", 1, Some(1), 10),
        ("resource-zero", "pass, fail", 1, Some(0), 10),
        ("clock-zero", "pass, fail", 1, None, 0),
        ("clock-boundary", "pass, fail", 1, None, 1),
    ] {
        let branch = "if x == pass { end good } else { end bad }";
        let anonymous = name == "capacity-zero" || name == "anonymous-one";
        let pool: &[&str] = if name == "executor-pool" {
            &["p", "q"]
        } else {
            &["p"]
        };
        let slots = if anonymous {
            format!("capacity {capacity}")
        } else {
            format!("executors [{}]", pool.join(","))
        };
        let decl = format!(
            "{}\nactor A {{ {slots}; {}; {} }}\nactor Spare {{ capacity 1; executors [q] }}",
            resource.map_or(String::new(), |r| format!("resource R capacity {r}")),
            cap("x", 1, results, resource.is_some()),
            cap("unused", 1, "pass", false)
        );
        let src = source(
            &decl,
            &format!("x = A.x(); {branch}"),
            clock,
            &["good", "bad"],
        );
        let mut m = own(
            &src,
            vec![
                actor("A", capacity, if anonymous { None } else { Some(pool) }),
                actor("Spare", 1, Some(&["q"])),
            ],
            &[
                ("x", 0, 1, results, resource.is_some()),
                ("unused", 0, 1, "pass", false),
            ],
            &["x"],
            resource,
            clock,
        );
        m.body = vec![
            Op::Calls(vec![(0, 0)]),
            Op::If(
                OwnCondition::Equal(0, "pass".into()),
                vec![end(&src, "good")],
                vec![end(&src, "bad")],
                location(&src, branch),
            ),
        ];
        let deadlock = capacity == 0 || resource == Some(0);
        let cut = clock == 0;
        let mut hand_terminals = vec![];
        if !deadlock && !cut {
            hand_terminals.push(("good", 1, 1));
            if results.contains("fail") {
                hand_terminals.push(("bad", 1, 1));
            }
        }
        examples.push(Example {
            name,
            source: src,
            own: m,
            hand_terminals,
            deadlock,
            cut,
        });
    }
    for (name, parallel, capacity, resource, wait) in [
        ("serial", false, 2, None, false),
        ("parallel-two", true, 2, None, false),
        ("parallel-one", true, 1, None, false),
        ("parallel-resource", true, 2, Some(1), false),
        ("serial-wait", false, 2, None, true),
    ] {
        let body = if parallel {
            "fork f { x = A.x(); y = A.y() } join all f; end done"
        } else if wait {
            "x = A.x(); wait 2min; y = A.y(); end done"
        } else {
            "x = A.x(); y = A.y(); end done"
        };
        let decl = format!(
            "{}\nactor A {{ capacity {capacity}; {}; {} }}\nactor Spare {{ capacity 1; executors [q] }}",
            resource.map_or(String::new(), |r| format!("resource R capacity {r}")),
            cap("x", 3, "pass", resource.is_some()),
            cap("y", 5, "pass", resource.is_some())
        );
        let src = source(&decl, body, 20, &["done"]);
        let mut m = own(
            &src,
            vec![actor("A", capacity, None), actor("Spare", 1, Some(&["q"]))],
            &[
                ("x", 0, 3, "pass", resource.is_some()),
                ("y", 0, 5, "pass", resource.is_some()),
            ],
            &["x", "y"],
            resource,
            20,
        );
        m.body = if parallel {
            vec![Op::Calls(vec![(0, 0), (1, 1)])]
        } else {
            let mut ops = vec![Op::Calls(vec![(0, 0)])];
            if wait {
                ops.push(Op::Wait(2));
            }
            ops.push(Op::Calls(vec![(1, 1)]));
            ops
        };
        m.body.push(end(&src, "done"));
        examples.push(Example {
            name,
            source: src,
            own: m,
            hand_terminals: vec![(
                "done",
                if wait {
                    10
                } else if parallel && capacity == 2 && resource.is_none() {
                    5
                } else {
                    8
                },
                8,
            )],
            deadlock: false,
            cut: false,
        });
    }
    for (name, second) in [("shared-person", "p"), ("distinct-people", "q")] {
        let decl = format!(
            "actor A {{ capacity 1; executors [p]; {} }}\nactor B {{ capacity 1; executors [{second}]; {} }}\nactor Spare {{ capacity 1; executors [q] }}",
            cap("x", 3, "pass", false),
            cap("y", 5, "pass", false)
        );
        let src = source(
            &decl,
            "fork f { x = A.x(); y = B.y() } join all f; end done",
            20,
            &["done"],
        );
        let mut m = own(
            &src,
            vec![
                actor("A", 1, Some(&["p"])),
                actor("B", 1, Some(&[second])),
                actor("Spare", 1, Some(&["q"])),
            ],
            &[("x", 0, 3, "pass", false), ("y", 1, 5, "pass", false)],
            &["x", "y"],
            None,
            20,
        );
        m.body = vec![Op::Calls(vec![(0, 0), (1, 1)]), end(&src, "done")];
        examples.push(Example {
            name,
            source: src,
            own: m,
            hand_terminals: vec![("done", if second == "p" { 8 } else { 5 }, 8)],
            deadlock: false,
            cut: false,
        });
    }
    for (name, call_else, blocked) in [
        ("arm-call", true, false),
        ("arm-call-removed", false, false),
        ("mixed-deadlock", true, true),
    ] {
        let else_body = if call_else {
            "y = B.y(); end bad"
        } else {
            "end bad"
        };
        let branch = format!("if x == pass {{ end good }} else {{ {else_body} }}");
        let decl = format!(
            "actor A {{ capacity 1; executors [p]; {} }}\nactor B {{ capacity {}; {} }}",
            cap("x", 1, "pass, fail", false),
            u64::from(!blocked),
            cap("y", 3, "pass", false)
        );
        let src = source(&decl, &format!("x = A.x(); {branch}"), 10, &["good", "bad"]);
        let mut m = own(
            &src,
            vec![
                actor("A", 1, Some(&["p"])),
                actor("B", u64::from(!blocked), None),
            ],
            &[("x", 0, 1, "pass, fail", false), ("y", 1, 3, "pass", false)],
            if call_else { &["x", "y"] } else { &["x"] },
            None,
            10,
        );
        let mut no = vec![];
        if call_else {
            no.push(Op::Calls(vec![(1, 1)]));
        }
        no.push(end(&src, "bad"));
        m.body = vec![
            Op::Calls(vec![(0, 0)]),
            Op::If(
                OwnCondition::Equal(0, "pass".into()),
                vec![end(&src, "good")],
                no,
                location(&src, &branch),
            ),
        ];
        let mut hand_terminals = vec![("good", 1, 1)];
        if !blocked {
            hand_terminals.push((
                "bad",
                if call_else { 4 } else { 1 },
                if call_else { 4 } else { 1 },
            ));
        }
        examples.push(Example {
            name,
            source: src,
            own: m,
            hand_terminals,
            deadlock: blocked,
            cut: false,
        });
    }
    examples
}
