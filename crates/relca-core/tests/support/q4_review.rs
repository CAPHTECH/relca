//! Independent review regressions. Arithmetic is in resource_calendars_review.md.
#![allow(dead_code)]
use serde_json::{Value, json};

pub const CONDITION: &str = r#"model ConditionResponsibility version "0.2" {
  type Entry = enum { entry }
  type Verdict = enum { pass, fail }
  type Ack = enum { committed }
  people [a1]
  actor Invoice { source document }
  actor Reader { capacity 1
    capability enter() -> Entry { reads [Invoice]; duration fixed 1min; result nondet { entry } }
  }
  actor Approver { executors [a1]
    capability approve(e: Entry) -> Verdict { reads [Invoice]; duration fixed 1min; result nondet { pass, fail } }
  }
  actor Ledger { capacity 1
    capability post() -> Ack { commits; duration fixed 1min; result nondet { committed } }
  }
  responsible a1 for Approver.approve
  responsible a1 for Ledger.post
  process Flow {
    entry = Reader.enter()
    approval = Approver.approve(entry)
    wait until approval == pass
    posted = Ledger.post()
    end done
  }
  cut C { include [Invoice, Reader, Approver, Ledger] }
  arrivals fixed [0min]
  bounds { cases 1; live_tasks 2; clock 20min }
  verify safety no_resource_overcommit
}"#;

pub fn condition_source(dataflow: bool) -> String {
    if dataflow {
        CONDITION
            .replace("post() -> Ack", "post(v: Verdict) -> Ack")
            .replace("posted = Ledger.post()", "posted = Ledger.post(approval)")
    } else {
        CONDITION.into()
    }
}

pub fn branch_control(dataflow: bool) -> String {
    condition_source(dataflow).replace(
        "wait until approval == pass",
        "if approval == pass { } else { end returned }",
    )
}

pub fn scenario_arrivals(last: u64, reverse: bool) -> String {
    let source = include_str!("q4/arrivals.relca");
    let burst = "0.5: fixed [0min, 0min]";
    let spaced = format!("0.5: fixed [0min, {last}min]");
    let rows = if reverse {
        format!("{spaced}\n    {burst}")
    } else {
        format!("{burst}\n    {spaced}")
    };
    let from = "arrivals distribution basis \"fixture assumption; not measured\" {\n    0.5: fixed [0min, 0min]\n    0.5: fixed [0min, 4min]\n  }";
    assert_eq!(source.matches(from).count(), 1);
    source.replace(from, &format!("scenario Day {{ arrivals distribution basis \"fixture assumption; not measured\" {{\n    {rows}\n  }} }}"))
}

pub fn condition_withdrawal(value: &Value) -> Result<(), String> {
    let rows = value
        .pointer("/responsibilities")
        .and_then(Value::as_array)
        .ok_or("responsibility rows required")?;
    if rows.len() != 2 {
        return Err("two declared duties required".into());
    }
    for target in ["Approver.approve", "Ledger.post"] {
        let matching: Vec<_> = rows
            .iter()
            .filter(|r| r["target"]["value"] == target && r["person"] == "a1")
            .collect();
        if matching.len() != 1 {
            return Err(format!("one duty required for {target}"));
        }
        let refusal = &matching[0]["refusal"];
        if refusal["status"] != "not_derived" {
            return Err(format!(
                "{target}: condition control must be not_derived, got {refusal}"
            ));
        }
        if !refusal["reason"]
            .as_str()
            .is_some_and(|s| s.contains("condition"))
        {
            return Err(format!(
                "{target}: explicit condition reason required, got {refusal}"
            ));
        }
        if refusal["effects"] != json!([]) {
            return Err(format!(
                "{target}: no certified decision-support effects under condition wait"
            ));
        }
    }
    Ok(())
}

pub fn drain(value: &Value, last: u64) -> Result<(), String> {
    let rows = value["assumptions"]
        .as_array()
        .ok_or("assumptions required")?;
    let rows: Vec<_> = rows
        .iter()
        .filter(|r| r["field"] == "scenario.drain")
        .collect();
    if rows.len() != 1 {
        return Err("one scenario.drain assumption required".into());
    }
    let row = rows[0];
    if row["value"] == format!("{last}min..all_terminal_or_clock") {
        return Ok(());
    }
    // The ruling permits withdrawal instead of computing a support envelope.
    // A bare null or a generic caveat beside the wrong integer is not withdrawal.
    if row["value"] == "not_derived"
        && row["affects"]
            .as_str()
            .is_some_and(|s| s.contains("arrival") && s.contains("distribution"))
    {
        return Ok(());
    }
    Err(format!(
        "scenario.drain requires support maximum {last}min or reasoned withdrawal, got {row}"
    ))
}

pub fn condition_witness() -> Value {
    json!({"responsibilities":[
        {"person":"a1","target":{"value":"Approver.approve"},"refusal":{"status":"not_derived","reason":"condition_wait_control","effects":[]}},
        {"person":"a1","target":{"value":"Ledger.post"},"refusal":{"status":"not_derived","reason":"condition_wait_control","effects":[]}}
    ]})
}

pub fn control_pairs() {
    let good = condition_witness();
    condition_withdrawal(&good).unwrap();
    for duty in 0..2 {
        let mut bad = good.clone();
        bad["responsibilities"][duty]["refusal"]["status"] = json!("present");
        assert!(condition_withdrawal(&bad).is_err());
        let mut bad = good.clone();
        bad["responsibilities"][duty]["refusal"]["reason"] = Value::Null;
        assert!(condition_withdrawal(&bad).is_err());
        let mut bad = good.clone();
        bad["responsibilities"][duty]["refusal"]["effects"] =
            json!([{"capability":"Ledger.post","status":"none"}]);
        assert!(condition_withdrawal(&bad).is_err());
    }
    let good = json!({"assumptions":[{"field":"scenario.drain","value":"4min..all_terminal_or_clock","affects":"declaration envelope"}]});
    drain(&good, 4).unwrap();
    let mut bad = good.clone();
    bad["assumptions"][0]["value"] = json!("0min..all_terminal_or_clock");
    assert!(
        drain(&bad, 4).is_err(),
        "first row must not stand for the distribution"
    );
    let mut withdrawn = good;
    withdrawn["assumptions"][0]["value"] = json!("not_derived");
    withdrawn["assumptions"][0]["affects"] = json!("arrival distribution drain not derived");
    drain(&withdrawn, 4).unwrap();
    withdrawn["assumptions"][0]["affects"] = json!("");
    assert!(drain(&withdrawn, 4).is_err());
}

// The queued long branch is cancelled before its resource opens. It must not
// survive the owner's End and block the second case's FIFO/call-order head.
pub const CANCELLED_QUEUE: &str = r#"model CancelledQueue version "0.2" {
  type R = enum { ok }
  calendar Always through 21min on_close continue { [0min, 21min) capacity 1 }
  calendar Late through 21min on_close continue { [0min, 5min) capacity 0; [5min, 21min) capacity 1 }
  resource R capacity 1 calendar Late
  actor Desk { capacity 1; calendar Always
    capability short() -> R { duration fixed 1min; result nondet { ok }; cancel forbidden_after_start }
    capability long() -> R { uses R: 1; duration fixed 2min; result nondet { ok }; cancel cooperative ack_within 1min }
  }
  process Flow {
    fork work { a = Desk.short(); b = Desk.long() }
    join any work where result == ok tie by [a, b] remaining cancel_then_drain if none -> done
    end done
  }
  cut C { include [Desk] }
  arrivals fixed [0min, 2min]
  policy fifo tie_by case_id
  bounds { cases 2; live_tasks 2; clock 20min }
  metrics { window 20min; currency "JPY"; breakdown waits }
  verify reachable end.done
  verify safety no_resource_overcommit
}"#;

pub fn queued_source(ordered: bool, cancel: bool) -> String {
    let mut source = CANCELLED_QUEUE.to_string();
    if ordered {
        source = source.replace(
            "policy fifo tie_by case_id",
            "policy observable fifo tie_by case_id then call_order assign first_idle",
        );
    }
    if !cancel {
        source = source.replace("remaining cancel_then_drain", "remaining drain");
    }
    source
}

pub fn receipt_source(delivered: bool) -> String {
    let source = include_str!("q4/retry.relca");
    let from = "response_loss sequence [unknown, delivered]\n      retry max_attempts 2 when receipt unknown backoff fixed 0min exhausted end failed";
    assert_eq!(source.matches(from).count(), 1);
    source
        .replace(
            from,
            if delivered {
                "response_loss sequence [delivered]"
            } else {
                "response_loss sequence [unknown]"
            },
        )
        .replace("arrivals fixed [0min]", "arrivals fixed [0min, 2min]")
        .replace("cases 1;", "cases 2;")
        .replace(
            "  contract Once",
            "  policy fifo tie_by case_id\n  contract Once",
        )
}
