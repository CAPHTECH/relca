//! Hand-derived C6 crossings. No new fixture directories or census members.
#![allow(dead_code)]
pub const S: &str = include_str!("../../../../fixtures/mut2-summary/model.relca");
pub const X: &str = include_str!("../../../../fixtures/mut2-expiry/model.relca");
pub fn replace(s: &str, from: &str, to: &str) -> String {
    assert_eq!(s.matches(from).count(), 1, "edit witness: {from}");
    s.replacen(from, to, 1)
}
pub fn add(s: &str, items: &str) -> String {
    let at = s.rfind('}').unwrap();
    format!("{}\n{items}\n{}", &s[..at], &s[at..])
}
pub fn scenario(s: &str) -> String {
    replace(
        s,
        "bounds { cases 1; live_tasks 1; clock 10min }",
        "scenario Day { arrivals fixed [0min]; bounds { cases 1; live_tasks 1; clock 10min } }",
    )
}
pub fn version(writer: bool) -> String {
    let s =
        include_str!("../../../../fixtures/mut2-summary-state-finish.broken-writer/model.relca");
    let s = replace(
        s,
        "type Entry = enum { entry }",
        "type Entry = enum { unused }",
    );
    let s = s
        .replace("-> Entry", "-> Verdict")
        .replace("e:Entry", "e:Verdict");
    let s = replace(&s, "result nondet { entry }", "result nondet { pass }");
    if writer {
        replace(
            &s,
            "writes flag;",
            "version_check [flag] else fail; writes flag;",
        )
    } else {
        s
    }
}
pub fn observed(source: bool, multiple: bool, record: bool) -> String {
    let mut s = S.to_owned();
    s = replace(&s, "reads [Invoice]", "reads [snap]");
    s = replace(
        &s,
        "e = Extract.make();",
        "w = Writer.make(); e = Extract.make();",
    );
    s = replace(&s, "include [Invoice,", "include [Writer,Invoice,");
    s = add(
        &s,
        &format!(
            "observed case {{ snap:optional<Entry> }}\nactor Writer {{ capacity 1; capability make() -> Entry {{ {} writes snap; duration fixed 1min; result nondet {{ entry }} }} }}",
            if source { "reads [Invoice];" } else { "" }
        ),
    );
    if multiple {
        s = replace(
            &s,
            "w = Writer.make();",
            "first = Earlier.make(); w = Writer.make();",
        );
        s = replace(&s, "include [Writer,", "include [Earlier,Archive,Writer,");
        s = add(
            &s,
            "actor Archive { source record } actor Earlier { capacity 1; capability make() -> Entry { reads [Archive]; writes snap; duration fixed 1min; result nondet { entry } } }",
        );
    }
    if record {
        s = recordize(&s);
    }
    s
}
pub fn recordize(s: &str) -> String {
    replace(
        s,
        "type Entry = enum { entry }",
        "type Entry = record { confidence: enum { high, low } }",
    )
    .replace(
        "result nondet { entry }",
        "result nondet { Entry { confidence = high } }",
    )
}
pub fn record() -> String {
    let s = recordize(S);
    let s = replace(&s, "approve(e:Entry)", "approve(e:Entry,copy:Entry)");
    let s = replace(&s, "a = Approver.approve(e)", "a = Approver.approve(e,e)");
    replace(
        &s,
        "if p == pass && a == pass",
        "if e.confidence == high && p == pass && a == pass",
    )
}
pub fn manual(s: &str) -> String {
    let s = replace(
        s,
        "e = Extract.make();",
        "e = Extract.make(); e_dup = Extract.make();",
    );
    if s.contains("a = Approver.approve(e,e)") {
        replace(
            &s,
            "a = Approver.approve(e,e)",
            "a = Approver.approve(e_dup,e_dup)",
        )
    } else {
        replace(&s, "a = Approver.approve(e)", "a = Approver.approve(e_dup)")
    }
}
pub fn quality() -> String {
    let s = S.replace("result nondet { pass, fail }", "result nondet { pass }");
    add(
        &s,
        "property case.Timely = reached(end.done) within 4min\nmetric cohort.Rate = expected(count(case.Timely)) / arrivals",
    )
}
pub fn rule(basis: bool, quality: bool) -> String {
    let mut s = S.to_owned();
    s = replace(&s, "include [Invoice,", "include [ApprovalRule,Invoice,");
    s = replace(
        &s,
        "if p == pass && a == pass",
        "if ApprovalRule.required == true && p == pass && a == pass",
    );
    s = add(
        &s,
        "people [a1,cfo]\nactor ApprovalRule { version \"r1\"; decides required: bool = true }\nauthority RuleChange { basis \"amendment procedure\"; grants cfo to change ApprovalRule; not_by [a1] }",
    );
    if basis {
        s = replace(
            &s,
            "actor Approver { capacity 1;",
            "actor Approver { executors [cfo];",
        );
        s = replace(
            &s,
            "grants cfo to change ApprovalRule;",
            "grants cfo to change ApprovalRule; grants cfo to Approver.approve;",
        );
        s = add(
            &s,
            "contract K { authority_basis Approver.approve RuleChange }",
        );
    }
    if quality {
        s = add(
            &s,
            "property case.Done = reached(end.done)\nmetric cohort.Rate = expected(count(case.Done)) / arrivals",
        );
    }
    s
}
pub fn held_summary(horizon: Option<u64>, clock: u64) -> String {
    let s = S.replace("result nondet { pass, fail }", "result nondet { pass }");
    let s = replace(&s, "bounds { cases 1; live_tasks 2; clock 10min }", "");
    add(
        &s,
        &format!(
            "policy fifo tie_by case_id\nscenario Day {{ arrivals fixed [0min,1min]; concurrent cases 1 on_overflow wait_external fifo; {} bounds {{ cases 2; live_tasks 1; clock {clock}min }} }}",
            horizon
                .map(|h| format!("horizon {h}min;"))
                .unwrap_or_default()
        ),
    )
}

pub fn observed_wlin2() -> String {
    let s = observed(true, false, false);
    let s = replace(
        &s,
        "writes snap; duration fixed 1min; result nondet { entry }",
        "writes [snap,bad]; duration fixed 1min; outcomes kernel Seed",
    );
    let s = replace(
        &s,
        "reads [PurchaseOrder]; duration fixed 1min; result nondet { pass, fail }",
        "reads [PurchaseOrder]; duration fixed 1min; outcomes kernel KA; kernel_reads latent [bad]",
    );
    let s = replace(
        &s,
        "capability approve(e:Entry) -> Verdict { duration fixed 1min; result nondet { pass, fail }",
        "capability approve(e:Entry) -> Verdict { duration fixed 1min; outcomes kernel KB; kernel_reads latent [bad]",
    );
    add(
        &s,
        "latent case { bad: bool = false }\nkernel Seed { * -> { 1: entry with bad = false } }\nkernel KA given [bad] { false -> { 1: pass }; true -> { 1: pass } }\nkernel KB given [bad] { false -> { 1: pass }; true -> { 1: pass } }",
    )
}

pub fn quality_horizon(horizon: u64) -> String {
    let source = replace(
        &quality(),
        "bounds { cases 1; live_tasks 2; clock 10min }",
        "",
    );
    add(
        &source,
        &format!(
            "scenario Day {{ arrivals fixed [0min]; horizon {horizon}min; bounds {{ cases 1; live_tasks 1; clock 10min }} }}"
        ),
    )
}

/// Boundary-review model H: keeping the empty then arm adds unfinished work.
pub fn guard_horizon(horizon: u64) -> String {
    format!(
        r#"model H version "0.2" {{
type Entry = enum {{ entry }}
type Verdict = enum {{ pass, fail }}
type Ack = enum {{ posted }}
actor Invoice {{ source document }}
actor PurchaseOrder {{ source record }}
actor Extract {{ capacity 1; capability make() -> Entry {{ reads [Invoice]; duration fixed 1min; result nondet {{ entry }} }} }}
actor Clerk {{ capacity 1; capability match_po(e:Entry) -> Verdict {{ reads [PurchaseOrder]; duration fixed 1min; result nondet {{ fail }} }} }}
actor Ledger {{ capacity 1; capability post(p:Verdict) -> Ack {{ commits; duration fixed 2min; result nondet {{ posted }} }} }}
process Flow {{ e = Extract.make(); p = Clerk.match_po(e); if p == pass {{}} else {{ end rejected }}; z = Ledger.post(p); end done }}
cut Scope {{ include [Invoice,PurchaseOrder,Extract,Clerk,Ledger] }}
scenario Day {{ arrivals fixed [0min]; horizon {horizon}min; bounds {{ cases 1; live_tasks 1; clock 10min }} }}
verify safety no_resource_overcommit
}}"#
    )
}

pub fn start_only_writer() -> String {
    replace(
        &observed(true, false, false),
        "writes snap; duration fixed 1min;",
        "start { snap = entry }; duration fixed 1min;",
    )
}
