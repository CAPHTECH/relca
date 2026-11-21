use super::{assert_supplement, blocker, check, diagnostic, invoke, location};
use serde_json::{Value, json};

const REG: &str = "registration_feature_combination";
const FINISH_DEPENDENT: &str = "registration_result_dependent";
pub(super) const REG_SEE: &str = "docs/guide/dsl.md#知識を記録へ登録する";
const UNCONNECTED: &str = "analysis_not_connected";

fn detail(reason: &str, cause: &str, kind: &str, blockers: Vec<Value>, see: &str) -> Value {
    json!({"reason":reason,"cause":cause,"kind":kind,"blockers":blockers,"see":see})
}

fn at(role: &str, name: &str, line: usize, start: usize, end: usize) -> Value {
    blocker(role, Some(name), location(line, start, line, end))
}

fn refuses(src: &str, message: &str, position: (usize, usize), want: Value) -> String {
    let (exit, report, stderr) = check(src, &[]);
    assert_eq!(exit, 4, "{report}");
    assert_eq!(report["verdict"], "unsupported");
    let row = diagnostic(&report, message);
    assert_eq!(row["code"], "U-SEM-0");
    assert_eq!(row["detail"], want, "{message}: {src}");
    assert_supplement(&stderr, message, position, &want);
    stderr
}

fn accepted(src: &str) {
    let (exit, report, stderr) = check(src, &[]);
    assert_eq!(exit, 0, "{report}\n{stderr}");
    assert_eq!(report["verdict"], "satisfied");
    assert!(
        !stderr.lines().any(|line| line.starts_with("  ")),
        "{stderr}"
    );
}

const REG_BASE: &str = r#"model Reg version "0.2" {
  type V = enum { ok, alt }
  observed case { flag: bool = false }
  actor Src { source document }
  actor Notes {}
  actor A { capacity 1
    capability save() -> V { reads [Src]; registers [Notes]; duration fixed 1min; result nondet { ok } }
  }
  cut C { include [Src, Notes, A] }
  process P {
    a = A.save()
    end done
  }
  bounds { cases 1; live_tasks 1; clock 10min }
  verify safety no_resource_overcommit
}
"#;

fn reg(reason: &str, cause: &str, blockers: Vec<Value>) -> Value {
    let kind = if reason == FINISH_DEPENDENT {
        "rewrite"
    } else {
        UNCONNECTED
    };
    detail(reason, cause, kind, blockers, REG_SEE)
}

const LOOP: &str = "loop_feature_combination";
pub(super) const LOOP_SEE: &str = "docs/guide/dsl.md#繰り返し";

fn loop_base() -> String {
    REG_BASE.replace("registers [Notes]; ", "").replace(
        "    a = A.save()\n",
        "    repeat R at_most 1 {\n      a = A.save()\n      leave\n    } on_limit {\n      end limit\n    }\n",
    )
}

#[test]
fn loop_reports_first_syntax_cause_and_outer_repeat() {
    let base = loop_base();
    let outer = blocker("repeat", None, location(11, 5, 11, 11));
    for (source, cause, kind, line, start, end) in [
        (base.replace("      a = A.save()", "      repeat X at_most 1 { a = A.save(); leave } on_limit { end inner }"), "nested_repeat", UNCONNECTED, 12, 7, 13),
        (base.replace("      leave", "      v = if a == ok { yield a } else { yield a }\n      leave"), "value_returning_if", "rewrite", 13, 7, 8),
        (base.replace("      leave", "      degrade D when a == ok {} else {}\n      leave"), "degradation_in_repeat", UNCONNECTED, 13, 7, 14),
        (base.replace("a = A.save()", "a = A.save() response_loss none"), "response_or_retry", UNCONNECTED, 12, 20, 33),
        (base.replace("      end limit", "      wait until flag == true\n      end limit"), "on_limit_statement", UNCONNECTED, 15, 18, 22),
        (base.replace("      end limit", "      fork F { b = A.save() }; join all F\n      end limit"), "on_limit_statement", UNCONNECTED, 15, 7, 11),
        (base.replace("      a = A.save()", "      fork F { a = A.save() }; join any F where result == ok tie by [a] remaining drain if none -> limit"), "join_any", UNCONNECTED, 12, 7, 11),
    ] {
        refuses(&source, "unsupported syntax: loop_feature_combination", (line,start), detail(LOOP, cause, kind, vec![
            outer.clone(), blocker("feature", None, location(line,start,line,end)),
        ], LOOP_SEE));
        accepted(&base);
    }
}

fn terminal_hand(src: &str, end: &str, minutes: (u64, u64), prob: &str) {
    let (exit, report, stderr) = check(src, &[]);
    assert_eq!(exit, 0, "{stderr}");
    let row = report["terminals"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["end"] == end)
        .expect(end);
    assert_eq!(row["elapsed"], json!({"min":minutes.0,"max":minutes.1}));
    assert_eq!(row["probability"], json!({"min":prob,"max":prob}));
}

#[test]
fn local_rewrites_preserve_repeat_and_registration() {
    let original = r#"model Repeat version "0.2" {
  type V = enum { ok, alt }
  kernel K { * -> { 0.5: ok; 0.5: alt } }
  actor A { capacity 1
    capability pick() -> V { duration fixed 1min; outcomes kernel K }
    capability use(x: V) -> V { duration fixed 1min; result nondet { ok } }
  }
  cut C { include [A] }
  process P {
    repeat R at_most 2 {
      a = A.pick()
      v = if a == ok { yield a } else { yield a }
      u = A.use(v)
      if a == ok { leave } else { again }
    } on_limit { end limit }
    end done
  }
  bounds { cases 1; live_tasks 1; clock 10min }
  verify safety no_resource_overcommit
}
"#;
    let stderr = refuses(
        original,
        "unsupported syntax: loop_feature_combination",
        (12, 7),
        detail(
            LOOP,
            "value_returning_if",
            "rewrite",
            vec![
                blocker("repeat", None, location(10, 5, 10, 11)),
                blocker("feature", None, location(12, 7, 12, 8)),
            ],
            LOOP_SEE,
        ),
    );
    assert!(
        stderr.ends_with(concat!(
            "U-SEM-0: unsupported syntax: loop_feature_combination\n",
            "  at 12:7\n  kind: rewrite\n  cause: value_returning_if\n",
            "  blocked by: repeat at 10:5\n  blocked by: feature at 12:7\n",
            "  see: docs/guide/dsl.md#繰り返し\n",
        )),
        "{stderr}"
    );
    let rewritten = original.replace(
        "      v = if a == ok { yield a } else { yield a }\n      u = A.use(v)\n      if a == ok { leave } else { again }",
        "      if a == ok { u1 = A.use(a); leave } else { u2 = A.use(a); again }",
    );
    // A round takes 1+1 minutes. Its independent draw leaves with 1/2;
    // after two draws, the remaining probability is 1/4.
    terminal_hand(&rewritten, "done", (2, 4), "3/4");
    terminal_hand(&rewritten, "limit", (4, 4), "1/4");

    let original = REG_BASE
        .replace(
            "  observed",
            "  kernel K { * -> { 0.5: ok; 0.5: alt } }\n  observed",
        )
        .replace("result nondet { ok }", "outcomes kernel K")
        .replace(
            "    end done",
            "    if a == ok { end registered } else { end skipped }",
        );
    let rewritten = original.replace("    capability save()", "    capability choose() -> V { duration fixed 0min; outcomes kernel K }\n    capability save()")
        .replace("registers [Notes]; duration fixed 1min; outcomes kernel K", "registers [Notes]; duration fixed 1min; result nondet { ok }")
        .replace("    a = A.save()\n    if a == ok { end registered } else { end skipped }", "    chosen = A.choose()\n    if chosen == ok { a = A.save(); end registered } else { wait 1min; end skipped }");
    // The zero-duration draw chooses each path with 1/2. Both paths retain
    // the original minute of elapsed time; only the registering path writes.
    let (exit, report, stderr) = check(&original, &[]);
    assert_eq!(exit, 4);
    assert_eq!(
        diagnostic(&report, FINISH_DEPENDENT)["detail"]["kind"],
        "rewrite"
    );
    assert!(
        stderr.ends_with(concat!(
            "U-SEM-0: registration_result_dependent\n",
            "  at 8:5\n  kind: rewrite\n  cause: multiple_finish_results\n",
            "  blocked by: record Notes at 6:3\n  blocked by: registrar A.save at 8:5\n",
            "  blocked by: call a at 12:5\n  see: docs/guide/dsl.md#知識を記録へ登録する\n",
        )),
        "{stderr}"
    );
    terminal_hand(&rewritten, "registered", (1, 1), "1/2");
    terminal_hand(&rewritten, "skipped", (1, 1), "1/2");
}

#[test]
fn shared_record_lists_only_the_cause_and_deduplicates_round_calls() {
    let source = REG_BASE
        .replace("  actor Notes {}", "  actor Notes {}\n  actor Notes2 { instance per_case }")
        .replace("[Notes]", "[Notes, Notes2]")
        .replace("[Src, Notes, A]", "[Src, Notes, Notes2, A]")
        .replace("    a = A.save()", "    repeat R at_most 2 {\n      a = A.save()\n      again\n    } on_limit { end done }")
        .replace("cases 1", "cases 2")
        .replace("  verify", "  arrivals fixed [0min, 0min]\n  verify");
    let mut call = at("call", "a", 13, 7, 19);
    call["rounds"] = json!([1, 2]);
    let stderr = refuses(
        &source,
        REG,
        (8, 5),
        reg(
            REG,
            "shared_record",
            vec![
                at("record", "Notes", 5, 3, 17),
                at("registrar", "A.save", 8, 5, 113),
                call,
            ],
        ),
    );
    assert!(stderr.ends_with(concat!(
        "U-SEM-0: registration_feature_combination\n",
        "  at 8:5\n  kind: analysis_not_connected\n  cause: shared_record\n",
        "  blocked by: record Notes at 5:3\n  blocked by: registrar A.save at 8:5\n",
        "  blocked by: call a (rounds 1, 2) at 13:7\n  see: docs/guide/dsl.md#知識を記録へ登録する\n",
    )), "{stderr}");
}

#[test]
fn record_lineage_marks_the_repeat_that_supplies_the_record() {
    let source = r#"model RecordPath version "0.2" {
  type V = enum { ok }
  actor A { capacity 1
    capability pick() -> V { reads [Doc]; duration fixed 1min; result nondet { ok } }
    capability reg(x: V) -> V { registers [Notes]; duration fixed 1min; result nondet { ok } }
    capability rd() -> V { reads [Notes]; duration fixed 1min; result nondet { ok } }
  }
  actor Doc { source document }
  actor Notes {}
  cut C { include [A, Doc, Notes] }
  process P {
    repeat R0 at_most 1 { z = A.pick(); leave } on_limit { end other }
    repeat R at_most 1 { a = A.pick(); leave } on_limit { end limit }
    t = A.reg(a)
    u = A.rd()
    end done
  }
  bounds { cases 1; live_tasks 1; clock 10min }
  contract Basis { evidence_basis A.rd [Doc] }
}
"#;
    refuses(
        source,
        LOOP,
        (19, 20),
        detail(
            LOOP,
            "record_evidence_path",
            UNCONNECTED,
            vec![
                blocker("repeat", None, location(13, 5, 13, 11)),
                blocker("feature", None, location(15, 5, 15, 15)),
            ],
            LOOP_SEE,
        ),
    );
    accepted(&source.replace(
        "repeat R at_most 1 { a = A.pick(); leave } on_limit { end limit }",
        "a = A.pick()",
    ));
}

#[test]
fn contest_marks_both_repeats_at_their_keywords() {
    let source = r#"model ContestLoop version "0.2" {
  type V = enum { pass, fail }
  people [p1,q1]
  actor Clerk { executors [p1]; capability judge() -> V { duration fixed 1min; result nondet { pass, fail } }
    capability redo() -> V { duration fixed 1min; result nondet { pass, fail } } }
  actor Vendor { control party; standing over [end.returned] basis "claim" }
  authority G { basis "decision"; grants p1 to Clerk.judge }
  cut S { include [Clerk,Vendor] }
  process Main {
    repeat repeat at_most 2 { d = Clerk.judge(); if d == pass { leave } else { again } } on_limit { end returned }
    repeat R1 at_most 2 { e = Clerk.judge(); if e == pass { leave } else { again } } on_limit { end returned }
    end posted
  }
  bounds { cases 1; live_tasks 1; clock 20min }
  verify safety no_resource_overcommit
  contract Appeal {
    contest Vendor on end.returned route Clerk.redo reversible_by [p1] respond_by 2min
  }
}
"#;
    refuses(
        source,
        "loop_feature_combination: repeat with contest is not connected",
        (1, 1),
        detail(
            LOOP,
            "contest",
            UNCONNECTED,
            vec![
                blocker("repeat", None, location(10, 5, 10, 11)),
                blocker("repeat", None, location(11, 5, 11, 11)),
                blocker("feature", None, location(17, 5, 17, 87)),
            ],
            LOOP_SEE,
        ),
    );
    accepted(&format!(
        "{}}}\n",
        source.split("  contract Appeal").next().unwrap()
    ));
}

#[test]
fn degradation_lists_both_the_registering_and_reading_calls() {
    let src = REG_BASE.replace("  }\n  cut", "    capability rd() -> V { reads [Notes]; duration fixed 1min; result nondet { ok } }\n  }\n  cut")
        .replace("    end done", "    degrade D when a == ok {\n      b = A.save()\n    } else {\n      t = A.rd()\n    }\n    end done");
    refuses(
        &src,
        REG,
        (11, 3),
        reg(
            REG,
            "degradation",
            vec![
                at("record", "Notes", 5, 3, 17),
                at("registrar", "A.save", 7, 5, 105),
                at("reader", "A.rd", 8, 5, 86),
                blocker("degradation", Some("D"), location(13, 5, 17, 6)),
                at("call", "b", 14, 7, 19),
                at("call", "t", 16, 7, 17),
            ],
        ),
    );
}

#[test]
fn loop_in_degradation_and_recovery_has_both_tokens() {
    let repeat = "      repeat R at_most 1 { wait 1min; leave } on_limit { end posted }";
    let degradation = REG_BASE.replace("registers [Notes]; ", "").replace(
        "    end done\n",
        &format!("    degrade D when a == ok {{\n{repeat}\n    }} else {{}}\n    end done\n"),
    );
    let header = "unsupported syntax: loop_feature_combination";
    refuses(
        &degradation,
        header,
        (13, 7),
        detail(
            LOOP,
            "repeat_in_degradation",
            UNCONNECTED,
            vec![
                blocker("feature", None, location(12, 5, 12, 12)),
                blocker("repeat", None, location(13, 7, 13, 13)),
            ],
            LOOP_SEE,
        ),
    );
    accepted(&degradation.replace(repeat, "      b = A.save()"));

    let recovery = RECOVERY.replace("      b = A.save()", repeat);
    refuses(
        &recovery,
        header,
        (18, 7),
        detail(
            LOOP,
            "repeat_in_recovery",
            UNCONNECTED,
            vec![
                blocker("feature", None, location(17, 61, 17, 76)),
                blocker("repeat", None, location(18, 7, 18, 13)),
            ],
            LOOP_SEE,
        ),
    );
    let control = recovery.replace(repeat, "");
    assert!(!check(&control, &[]).2.contains(LOOP));
}

#[test]
fn loop_mutation_without_source_span_has_cause_and_no_false_position() {
    let source = loop_base().replace(
        "      a = A.save()",
        "      fork F { a = A.save() }; join all F",
    );
    accepted(&source);
    let (exit, report, stderr) = invoke("mutate", &source, &[]);
    assert_eq!(exit, 4);
    let message = "loop_feature_combination: repeat mutation sites are not connected";
    let row = diagnostic(&report, message);
    assert_eq!(row["span"], Value::Null);
    assert_eq!(row["location"], Value::Null);
    assert_eq!(
        row["detail"],
        detail(LOOP, "mutate_repeat_fork", UNCONNECTED, vec![], LOOP_SEE)
    );
    let expected =
        format!("  kind: analysis_not_connected\n  cause: mutate_repeat_fork\n  see: {LOOP_SEE}\n");
    let (_, after) = stderr
        .split_once(&format!("mutate: U-SEM-0: {message}\n"))
        .unwrap();
    assert_eq!(after, expected);
    assert!(!invoke("mutate", &loop_base(), &[]).2.contains(LOOP));
}

#[test]
fn registration_declaration_refusals() {
    // Span of `capability save` starts after four spaces. The unmodified
    // line's text has 100 ASCII characters; end is exclusive.
    let record = at("record", "Notes", 5, 3, 17);
    let call = at("call", "a", 11, 5, 17);
    let shared = REG_BASE
        .replace("cases 1", "cases 2")
        .replace("  verify", "  arrivals fixed [0min, 0min]\n  verify");
    refuses(
        &shared,
        REG,
        (7, 5),
        reg(
            REG,
            "shared_record",
            vec![
                record.clone(),
                at("registrar", "A.save", 7, 5, 105),
                call.clone(),
            ],
        ),
    );
    accepted(&shared.replace("actor Notes {}", "actor Notes { instance per_case }"));

    let start = REG_BASE.replace(
        "registers [Notes]",
        "start { flag = true }; registers [Notes]",
    );
    refuses(
        &start,
        REG,
        (7, 5),
        reg(
            REG,
            "start_writes",
            vec![
                record.clone(),
                at("registrar", "A.save", 7, 5, 128),
                call.clone(),
            ],
        ),
    );
    accepted(&start.replace("start { flag = true }; ", ""));

    let results = REG_BASE.replace("result nondet { ok }", "result nondet { ok, alt }");
    refuses(
        &results,
        FINISH_DEPENDENT,
        (7, 5),
        reg(
            FINISH_DEPENDENT,
            "multiple_finish_results",
            vec![record, at("registrar", "A.save", 7, 5, 110), call],
        ),
    );
    accepted(&results.replace("result nondet { ok, alt }", "result nondet { ok }"));
}

#[test]
fn registration_rejects_response_and_degradation() {
    let record = at("record", "Notes", 5, 3, 17);
    let registrar = at("registrar", "A.save", 7, 5, 105);
    let response = REG_BASE.replace("a = A.save()", "a = A.save() response_loss none");
    refuses(
        &response,
        REG,
        (11, 5),
        reg(
            REG,
            "response_or_retry",
            vec![
                record.clone(),
                registrar.clone(),
                at("call", "a", 11, 5, 36),
            ],
        ),
    );
    accepted(&response.replace(" response_loss none", ""));

    let degradation = REG_BASE.replace("    end done\n", "    degrade D when a == ok {\n      b = A.save()\n    } else {\n      end done\n    }\n    end done\n");
    refuses(
        &degradation,
        REG,
        (10, 3),
        reg(
            REG,
            "degradation",
            vec![
                record,
                registrar,
                blocker("degradation", Some("D"), location(12, 5, 16, 6)),
                at("call", "b", 13, 7, 19),
            ],
        ),
    );
    accepted(&degradation.replace("    degrade D when a == ok {", "    if a == ok {"));
}

const RECOVERY: &str = r#"model Recovery version "0.2" {
  type Ack = enum { ok, duplicate }
  actor Doc { source document }
  actor Notes {}
  actor A { capacity 1
    capability save() -> Ack { reads [Doc]; registers [Notes]; duration fixed 1min; result nondet { ok } }
  }
  actor L { capacity 1; state keys: set<CaseId>
    capability post(key:CaseId) -> Ack {
      reads [Doc]; commits; duration fixed 1min; writes keys; idempotency key key
      atomic { if key in keys { result duplicate } else { keys += key; result ok } }
      cancel forbidden_after_start
    }
  }
  cut C { include [Doc, Notes, A, L] }
  process P {
    x = L.post(case.id) response_loss sequence [unknown] on receipt_unknown {
      b = A.save()
      stop receipt_unknown
    }
    end posted
  }
  bounds { cases 1; live_tasks 1; clock 10min }
  verify safety no_resource_overcommit
}
"#;

#[test]
fn registration_rejects_handler_calls() {
    refuses(
        RECOVERY,
        REG,
        (18, 7),
        reg(
            REG,
            "recovery",
            vec![
                at("record", "Notes", 4, 3, 17),
                at("registrar", "A.save", 6, 5, 107),
                at("call", "b", 18, 7, 19),
            ],
        ),
    );
    let control = RECOVERY.replace("      b = A.save()\n", "");
    assert!(
        !check(&control, &[]).1["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["message"] == REG)
    );
}

const CONTEST: &str = r#"model ContestRecord version "0.2" {
  type V = enum { pass, fail }
  type Ack = enum { ok }
  people [p1,q1]
  actor Know { source tacit "knowledge" }
  actor Notes {}
  actor Clerk { executors [p1]; capability judge() -> V { duration fixed 1min; result nondet { fail } } }
  actor Registrar { executors [q1]
    capability store() -> Ack { reads [Know]; registers [Notes]; duration fixed 1min; result nondet { ok } }
  }
  actor Vendor { control party; standing over [end.returned] basis "claim" }
  authority G { basis "decision"; grants p1 to Clerk.judge }
  cut S { include [Know,Notes,Clerk,Registrar,Vendor] }
  process Main { r=Registrar.store(); d=Clerk.judge(); if d == fail { end returned } else { end posted } }
  bounds { cases 1; live_tasks 1; clock 10min }
  verify safety no_resource_overcommit
  contract Appeal {
    contest Vendor on end.returned route Registrar.store reversible_by [p1] respond_by 2min
  }
}
"#;

#[test]
fn registration_rejects_contest_route() {
    refuses(
        CONTEST,
        REG,
        (18, 5),
        reg(
            REG,
            "contest_route",
            vec![
                at("record", "Notes", 6, 3, 17),
                at("registrar", "Registrar.store", 9, 5, 109),
                at("contest_route", "Registrar.store", 18, 5, 92),
            ],
        ),
    );
    accepted(&format!(
        "{}}}\n",
        CONTEST.split("  contract Appeal").next().unwrap()
    ));
}

const BASIS_FEATURE: &str = "evidence_basis_feature_combination";
const EB_SEE: &str = "docs/guide/dsl.md#根拠源を求める契約";

#[test]
fn evidence_basis_marks_degradation_and_recovery() {
    let degradation = REG_BASE.replace("registers [Notes]; ", "")
        .replace("    end done\n", "    degrade D when a == ok {\n      b = A.save()\n    } else {\n      end done\n    }\n    end done\n")
        .replace("  verify", "  contract Basis { evidence_basis A.save [Src] }\n  verify");
    let stderr = refuses(
        &degradation,
        BASIS_FEATURE,
        (20, 20),
        detail(
            BASIS_FEATURE,
            "degradation",
            UNCONNECTED,
            vec![blocker("degradation", Some("D"), location(12, 5, 16, 6))],
            EB_SEE,
        ),
    );
    assert!(
        stderr.ends_with(concat!(
            "U-SEM-0: evidence_basis_feature_combination\n",
            "  at 20:20\n  kind: analysis_not_connected\n  cause: degradation\n",
            "  blocked by: degradation D at 12:5\n  see: docs/guide/dsl.md#根拠源を求める契約\n",
        )),
        "{stderr}"
    );
    accepted(&degradation.replace("degrade D when", "if"));

    let recovery = RECOVERY.replace("      b = A.save()", "").replace(
        "  verify",
        "  contract Basis { evidence_basis L.post [Doc] }\n  verify",
    );
    refuses(
        &recovery,
        BASIS_FEATURE,
        (24, 20),
        detail(
            BASIS_FEATURE,
            "recovery",
            UNCONNECTED,
            vec![
                blocker("response", Some("x"), location(17, 5, 20, 6)),
                at("recovery_handler", "x", 19, 7, 27),
            ],
            EB_SEE,
        ),
    );
    accepted(&recovery.replace(" response_loss sequence [unknown] on receipt_unknown {\n\n      stop receipt_unknown\n    }", ""));
}

const MERGED: &str = r#"model Merge version "0.2" {
  type V = enum { ok }
  actor Doc { source document }
  actor A { capacity 1
    capability take() -> V { reads [Doc]; duration fixed 1min; result nondet { ok } }
    capability use(x: V) -> V { duration fixed 1min; result nondet { ok } }
  }
  cut C { include [Doc,A] }
  process P {
    d = A.take()
    v = if d == ok { yield d } else { yield d }
    u = A.use(v)
    end done
  }
  bounds { cases 1; live_tasks 1; clock 10min }
  contract Basis { evidence_basis A.use [Doc] }
}
"#;

#[test]
fn evidence_basis_names_the_merge_and_target() {
    let stderr = refuses(
        MERGED,
        "merge_control: evidence_basis target has merged inputs",
        (16, 20),
        detail(
            "merge_control",
            "merged_evidence_input",
            UNCONNECTED,
            vec![
                at("merge_binding", "v", 11, 5, 6),
                at("target_call", "u", 12, 5, 17),
            ],
            EB_SEE,
        ),
    );
    assert!(
        stderr.ends_with(concat!(
            "U-SEM-0: merge_control: evidence_basis target has merged inputs\n",
            "  at 16:20\n  kind: analysis_not_connected\n  cause: merged_evidence_input\n",
            "  blocked by: merge_binding v at 11:5\n  blocked by: target_call u at 12:5\n",
            "  see: docs/guide/dsl.md#根拠源を求める契約\n",
        )),
        "{stderr}"
    );
    accepted(
        &MERGED
            .replace("    v = if d == ok { yield d } else { yield d }\n", "")
            .replace("A.use(v)", "A.use(d)"),
    );
}

const MERGE_FEATURE: &str = "merge_feature_combination";
pub(super) const MERGE_SEE: &str = "docs/guide/dsl.md#分岐した値を先へ渡す";

#[test]
fn merge_provenance_survives_a_repeat_alias() {
    let suffix = MERGED.replace("    u = A.use(v)", "    repeat R at_most 2 {\n      x = A.use(v)\n      leave\n    } on_limit { end limit }\n    u = A.use(x)");
    let message = "merge_control: evidence_basis target has merged inputs";
    let mut inner = at("target_call", "x", 13, 7, 19);
    inner["rounds"] = json!([1, 2]);
    refuses(
        &suffix,
        message,
        (20, 20),
        detail(
            "merge_control",
            "merged_evidence_input",
            UNCONNECTED,
            vec![
                at("merge_binding", "v", 11, 5, 6),
                inner,
                at("target_call", "u", 16, 5, 17),
            ],
            EB_SEE,
        ),
    );
}

#[test]
fn merge_wait_names_definition_and_reference() {
    let plain = MERGED.replace(
        "  contract Basis { evidence_basis A.use [Doc] }",
        "  verify safety no_resource_overcommit",
    );
    let source = plain.replace(
        "    u = A.use(v)",
        "    wait until v == ok\n    u = A.use(v)",
    );
    refuses(
        &source,
        "merge_feature_combination: merge_read_position",
        (12, 16),
        detail(
            MERGE_FEATURE,
            "unconnected_reference",
            UNCONNECTED,
            vec![
                at("merge_binding", "v", 11, 5, 6),
                at("reference", "v", 12, 16, 23),
            ],
            MERGE_SEE,
        ),
    );
    accepted(&plain);
}

#[test]
fn merge_mutation_names_the_declaration_but_does_not_invent_a_root_span() {
    let plain = MERGED.replace(
        "  contract Basis { evidence_basis A.use [Doc] }",
        "  verify safety no_resource_overcommit",
    );
    let (exit, report, stderr) = invoke("mutate", &plain, &[]);
    assert_eq!(exit, 4);
    let message = "merge_feature_combination: merge mutation sites are not connected";
    let row = diagnostic(&report, message);
    assert_eq!(row["span"], Value::Null);
    assert_eq!(row["location"], Value::Null);
    assert_eq!(
        row["detail"],
        detail(
            MERGE_FEATURE,
            "mutate_merge_sites",
            UNCONNECTED,
            vec![at("merge_binding", "v", 11, 5, 6)],
            MERGE_SEE
        )
    );
    let (before, after) = stderr
        .split_once(&format!("mutate: U-SEM-0: {message}\n"))
        .unwrap();
    assert!(!before.lines().any(|line| line.starts_with("  ")));
    assert_eq!(
        after,
        format!(
            "  kind: analysis_not_connected\n  cause: mutate_merge_sites\n  blocked by: merge_binding v at 11:5\n  see: {MERGE_SEE}\n"
        )
    );
    let control = plain
        .replace("    v = if d == ok { yield d } else { yield d }\n", "")
        .replace("A.use(v)", "A.use(d)");
    assert!(!invoke("mutate", &control, &[]).2.contains(MERGE_FEATURE));
}
