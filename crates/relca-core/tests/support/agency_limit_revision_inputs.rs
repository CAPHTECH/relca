//! Review reproductions, using the existing DSL only.
pub fn votes(n: usize, op: &str, response: bool) -> String {
    let calls = (1..=n)
        .map(|i| {
            format!(
                "j{i}=Judge.check(s){};",
                if response && i == 1 {
                    " response_loss sequence [unknown]"
                } else {
                    ""
                }
            )
        })
        .collect::<String>();
    let condition = (1..=n)
        .map(|i| format!("j{i}==pass"))
        .collect::<Vec<_>>()
        .join(op);
    format!(
        r#"model ViewpointLimit version "0.2" {{
type V = enum {{ pass, fail }}
type Ack = enum {{ done }}
actor Doc {{ source document }}
actor Intake {{ capacity 1; capability start()->V {{ reads [Doc]; duration fixed 1min; result nondet {{ pass }} }} }}
actor Judge {{ capacity 1; capability check(x:V)->V {{ duration fixed 1min; result nondet {{ pass, fail }} }} }}
actor Ledger {{ capacity 1; capability post(x:V)->Ack {{ commits; duration fixed 1min; result nondet {{ done }} }} }}
process Flow {{ s=Intake.start(); {calls} if {condition} {{}} else {{end rejected}}; p=Ledger.post(j{n}); end done }}
bounds {{ cases 1; live_tasks 8; clock 200min }}
verify safety no_resource_overcommit
}}"#
    )
}

pub fn merge_or(n: usize) -> String {
    votes(n," || ",false)
        .replace("type Ack", "type Doc2 = enum { draft, repaired }\ntype Ack")
        .replace("actor Ledger", "actor Office { capacity 1; capability write()->Doc2 { duration fixed 1min; result nondet { draft } } capability fix(d:Doc2)->Doc2 { duration fixed 1min; result nondet { repaired } } capability review(d:Doc2)->V { duration fixed 1min; result nondet { pass, fail } } }\nactor Ledger")
        .replace("s=Intake.start();", "s=Intake.start(); d=Office.write(); r=Office.review(d); ready = if r == fail { f = Office.fix(d); yield f } else { yield d }")
}

pub const REPEAT: &str = r#"model Rework version "0.2" {
  type Verdict = enum { pass, fail }
  actor Office { capacity 1
    capability inspect() -> Verdict { duration fixed 1min; result nondet { pass, fail } }
    capability second() -> Verdict { duration fixed 1min; result nondet { pass, fail } }
    capability correct() -> Verdict { duration fixed 2min; result nondet { pass } }
    capability escalate() -> Verdict { duration fixed 1min; result nondet { pass } }
  }
  process Main {
    repeat rework at_most 3 {
      r = Office.inspect()
      q = Office.second()
      if r == pass || q == pass { leave } else { c = Office.correct(); again }
    } on_limit { e = Office.escalate(); end escalated }
    end done
  }
  cut Scope { include [Office] }
  bounds { cases 1; live_tasks 1; clock 40min }
  verify reachable end.done
}"#;
