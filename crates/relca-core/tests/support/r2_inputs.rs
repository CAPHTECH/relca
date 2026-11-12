//! ADR 0065 independent inputs. No binary output is an input to construction.
#![allow(dead_code)]
use serde_json::json;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
pub const LOSS: &str = "response_loss sequence [unknown]";
pub const RETRY: &str = "response_loss sequence [unknown,unknown] retry max_attempts 2 when receipt unknown backoff fixed 0min exhausted end failed";
pub const PROCEED: &str = r#"status = Psp.query_status(case.id) for command pay
 if status == committed_once {
   decision = CS.decide(status)
   if decision == proceed {
     resolve effect pay using status
     sh = Shop.instruct_ship(decision)
     end shipped
   } else { stop receipt_unknown }
 } else { stop receipt_unknown }"#;
pub const REFUND: &str = r#"status = Psp.query_status(case.id) for command pay
 if status == committed_once {
   decision = CS.decide(status)
   if decision == withdraw {
     d = CS.draft_refund(decision)
     v = Finance.approve_refund(d)
     if v == approve {
       r = Psp.refund(v)
       resolve effect pay using status
       end refunded
     } else { stop receipt_unknown }
   } else { stop receipt_unknown }
 } else { stop receipt_unknown }"#;
pub fn edit(s: &str, old: &str, new: &str) -> String {
    assert_ne!(old, new);
    assert_eq!(s.matches(old).count(), 1, "single edit {old}");
    s.replacen(old, new, 1)
}
pub fn append(s: &str, text: &str) -> String {
    let i = s.rfind('}').unwrap();
    format!("{}\n{text}\n{}", &s[..i], &s[i..])
}
pub fn body(handler: Option<&str>, loss: &str) -> String {
    let h = handler
        .map(|s| format!("on receipt_unknown {{ {s} }}"))
        .unwrap_or_default();
    let source = format!(
        r#"model Recovery version "0.2" {{
 type Pay = enum {{ charged, rejected_duplicate }}
 type PaymentStatus = enum {{ committed_once, not_committed, still_unknown }}
 type Decision = enum {{ proceed, withdraw }}
 type Draft = enum {{ drafted }}
 type Verdict = enum {{ approve, reject }}
 type Ack = enum {{ recorded }}
 people [c,f,q,issuer]
 latent case {{ hidden_truth: bool = false }}
 observed case {{ charged_truth: bool = false; charges: 0..2 = 0; refunded: bool = false; approval: optional<Verdict> }}
 actor Psp {{ capacity 1; control policy; state keys: set<CaseId>
   capability charge_keyed(key: CaseId) -> Pay {{
     idempotency key key; commits; writes [keys,charged_truth,charges]; duration fixed 1min
     atomic {{ if key in keys {{ result rejected_duplicate }} else {{ keys += key; charged_truth = true; charges += 1; result charged }} }}
     cancel forbidden_after_start
   }}
   capability query_status(key: CaseId) -> PaymentStatus {{
     duration fixed 2min; receipt_query charge_keyed key key availability available
     cancel forbidden_after_start
   }}
   capability refund(v: Verdict) -> Ack {{ commits; duration fixed 1min; result nondet {{recorded}}; writes refunded; finish {{refunded = true}} }}
 }}
 actor CS {{ executors [c]; control policy
   capability decide(s: PaymentStatus) -> Decision {{ duration fixed 3min; result nondet {{proceed}} }}
   capability draft_refund(d: Decision) -> Draft {{ duration fixed 1min; result nondet {{drafted}} }}
   capability record_disposition(r: Ack) -> Ack {{ duration fixed 1min; result nondet {{recorded}} }}
   capability take_over(s: PaymentStatus) -> Ack {{ duration fixed 1min; result nondet {{recorded}} }}
 }}
 actor Finance {{ executors [f]; control policy
   capability approve_refund(d: Draft) -> Verdict {{ duration fixed 1min; result nondet {{approve}}; writes approval }}
 }}
 actor Shop {{ capacity 1; control policy
   capability instruct_ship(d: Decision) -> Ack {{ duration fixed 1min; result nondet {{recorded}} }}
   capability ship_known(p: Pay) -> Ack {{ duration fixed 1min; result nondet {{recorded}} }}
 }}
 authority RefundGrant {{ basis "assumed refund approval table"; grants f to Finance.approve_refund; issued_by issuer }}
 contract Controls {{
   at_most_once Psp.charge_keyed.commit per case.id
   invariant Psp.refund.commit -> approval == approve
   separation CS.draft_refund.finish, Finance.approve_refund.finish
   authority_basis Finance.approve_refund RefundGrant
 }}
 process Flow {{ pay = Psp.charge_keyed(case.id) {loss} {h}
   normal = Shop.ship_known(pay)
   end shipped
 }}
 cut C {{ include [Psp,CS,Finance,Shop] }}
 arrivals fixed [0min]
 policy fifo tie_by case_id
 bounds {{ cases 1; live_tasks 2; clock 40min }}
 metrics {{ window 40min; currency "JPY"; breakdown waits }}
 simulation replications 4 seed 7 confidence 0.5
 property case.Charged = reached(charged_truth == true)
 metric cohort.ChargedPerArrival = expected(count(case.Charged)) / arrivals
 verify probability reached charged_truth == true >= 1
 }}"#
    );
    if handler.is_none() {
        edit(
            &source,
            "receipt_query charge_keyed key key availability available",
            "result nondet {still_unknown}",
        )
    } else {
        source
    }
}
pub fn source(id: &str) -> String {
    match id {
        "R01" => body(Some(PROCEED), LOSS),
        "R02" => edit(
            &source("R01"),
            "availability available",
            "availability unavailable",
        ),
        "R03" => body(None, LOSS),
        "R04" => edit(
            &body(Some(REFUND), LOSS),
            "result nondet {proceed}",
            "result nondet {withdraw}",
        ),
        "R05" => edit(
            &source("R04"),
            "r = Psp.refund(v)",
            "r = Psp.refund(v)\nnote = CS.record_disposition(r)",
        ),
        "R06" => unresolved_take_over(),
        "R07" => edit(
            &source("R01"),
            "status = Psp.query_status",
            "wait 2min\nstatus = Psp.query_status",
        ),
        "R08" => body(Some(PROCEED), RETRY),
        "R09" => edit(
            &source("R04"),
            "result nondet {approve}",
            "result nondet {reject}",
        ),
        "T01" => deadline(&source("R01"), "end.shipped", 7),
        "T02" => deadline(&source("R01"), "CS.decide.finish", 6),
        "T03" => deadline(&source("R05"), "end.refunded", 9),
        "T04" => edit(&source("R01"), "clock 40min", "clock 2min"),
        "T05" => {
            let s = edit(&source("R01"), "arrivals fixed [0min]", "");
            let s = edit(&s, "bounds { cases 1; live_tasks 2; clock 40min }", "");
            append(
                &s,
                "scenario Day { arrivals fixed [0min]; horizon 2min; bounds { cases 1; live_tasks 2; clock 20min } }",
            )
        }
        "T06" => edit(&source("R01"), "clock 40min", "clock 4min"),
        "T07" => edit(
            &source("T02"),
            "arrivals fixed [0min]",
            "arrivals fixed [4min]",
        ),
        _ => panic!("unknown oracle {id}"),
    }
}
pub fn unresolved_take_over() -> String {
    body(Some("status = Psp.query_status(case.id) for command pay\nnote = CS.take_over(status)\nstop receipt_unknown"),LOSS)
        .replace("availability available","availability unavailable")
}
pub fn deadline(s: &str, event: &str, by: u64) -> String {
    append(
        s,
        &format!(
            "contract Deadline {{ response case.received -> one_of({event}) by {by}min after case.received otherwise unresolved }}"
        ),
    )
}
pub fn probability(available: bool, p: &str) -> String {
    let loss = format!(
        "response_loss probability {p} basis \"independent modeled receipt loss, not measured\" retry max_attempts 2 when receipt unknown backoff fixed 0min exhausted end failed"
    );
    let s = body(Some(PROCEED), &loss);
    let s = if available {
        s
    } else {
        edit(&s, "availability available", "availability unavailable")
    };
    // D/UD bypass CS. The ordinary suffix takes one minute.
    s
}
pub fn desk() -> String {
    let s = edit(
        &source("R01"),
        "capacity 1; control policy; state keys",
        "capacity 2; control policy; state keys",
    );
    let s = edit(
        &s,
        "duration fixed 2min; receipt_query",
        "uses QueryDesk: 1; duration fixed 2min; receipt_query",
    );
    let s = edit(
        &s,
        "executors [c]; control policy",
        "executors [c,q]; capacity 2; control policy",
    );
    let s = edit(&s, "actor Shop { capacity 1;", "actor Shop { capacity 2;");
    let s = edit(&s, "cases 1; live_tasks 2;", "cases 2; live_tasks 2;");
    append(
        &edit(&s, "arrivals fixed [0min]", "arrivals fixed [0min,0min]"),
        "resource QueryDesk capacity 1",
    )
}
pub fn paired(a: &str, b: &str) -> String {
    let s = source(a);
    let other = source(b);
    let open = other.find("process Flow {").unwrap();
    let close = other[open..].find("\n cut C").unwrap() + open;
    let second = other[open..close].replace("process Flow {", "process Other {");
    append(
        &s,
        &format!(
            "{second}\nconfiguration A {{cut C;process Flow}}\nconfiguration B {{cut C;process Other}}"
        ),
    )
}
pub fn dir() -> PathBuf {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../.tmp")
        .join(format!(
            "r2-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
    fs::create_dir_all(&p).unwrap();
    p
}
pub fn suite(points: &[(&str, String)]) -> PathBuf {
    let d = dir();
    let mut rows = Vec::new();
    for (name, s) in points {
        fs::write(d.join(format!("{name}.relca")), s).unwrap();
        rows.push(
            json!({"id":name,"model":format!("{name}.relca"),"configuration":null,"scenario":null}),
        );
    }
    let path = d.join("manifest.json");
    fs::write(&path,json!({"schema":"relca-sensitivity/v1","scope":{"kind":"enumerated_models","rationale":"explicit finite R2 assumptions"},"points":rows,"targets":[{"kind":"probability_query","id":"probability reached #1"}],"confidence":null}).to_string()).unwrap();
    path
}
pub const WORDS: &[&str] = &[
    "on",
    "receipt_unknown",
    "for",
    "command",
    "receipt_query",
    "availability",
    "available",
    "unavailable",
    "resolve",
    "effect",
    "using",
    "stop",
    "committed_once",
    "not_committed",
    "still_unknown",
];
pub fn old_binding(word: &str) -> String {
    format!(
        r#"model Old version "0.2" {{ type T = enum {{ok}} actor A {{capacity 1; capability run()->T {{duration fixed 1min;result nondet {{ok}}}}}} process Flow {{ {word} = A.run(); if {word} == ok {{end done}} else {{end no}} }} bounds {{cases 1;live_tasks 1;clock 5min}} verify reachable end.done }}"#
    )
}
