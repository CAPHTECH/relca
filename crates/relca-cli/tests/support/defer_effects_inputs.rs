//! ADR 0077's closed, hand-derived population. No production resolver is used.
use serde_json::{Value, json};

pub const INVOICE: &str = include_str!("../../../../examples/invoice/phase1.relca");
pub const RESUBMISSION: &str =
    "    defer Resubmission reason \"one receipt only\" loses \"ultimate settlement\"\n";
pub const VENDOR: &str = "    defer Vendor.contest_rate reason \"fixed recheck duration is not measured\" loses \"frequency and effort\"\n";

pub fn source(k: u8) -> String {
    assert!(k <= 7);
    let targets: &[&str] = match k {
        0 => &[],
        1 | 6 => &["Ghost"],
        2 => &["A.nosuch"],
        3 | 4 => &["R.y"],
        5 => &["R"],
        7 => &["Ghost", "R.y"],
        _ => unreachable!(),
    };
    let defer = targets
        .iter()
        .map(|t| format!("defer {t} reason \"later\" loses \"load\";\n"))
        .collect::<String>();
    let body = if k == 4 || k == 7 {
        "a = A.x(); end done"
    } else {
        "a = A.x(); r = R.y(); end done"
    };
    let indicators = if k == 6 {
        ""
    } else {
        r#"
metrics { window 12min; currency "JPY"; rate A per_minute 10; rate R per_minute 20; rate A.x per_start 1; rate R.y per_start 2; rate R.z per_start 4; breakdown waits }
property case.Done = reached(end.done)
metric cohort.Rate = expected(count(case.Done)) / arrivals
metric quantile.P95 = quantile(0.95, case.Done)
"#
    };
    format!(
        r#"model DeferEffects version "0.2" {{
type V = enum {{ pass, fail }}
people [p,q]
resource Desk capacity 1
actor A {{ executors [p]; capability x() -> V {{ duration fixed 3min; result nondet {{ pass }}; cancel forbidden_after_start }} }}
actor R {{ executors [q]; capability z() -> V {{ duration fixed 4min; result nondet {{ pass }}; cancel forbidden_after_start }} capability y() -> V {{ uses Desk: 1; duration fixed 2min; result nondet {{ pass }}; cancel forbidden_after_start }} }}
cut C {{ include [A,R]; {defer} }}
process P {{ {body} }}
{indicators}
bounds {{ cases 1; live_tasks 3; clock 12min }}
simulation replications 3 seed 7 confidence 0.5
verify reachable end.done
}}"#
    )
}

pub fn opaque(target: &str) -> Value {
    json!({"target":target,"resolution":"opaque","capabilities":[],"withdraws":"process","because":[{"kind":"opaque"}],"scope":null})
}
pub fn call(target: &str, resolution: &str, caps: &[&str], kind: &str, cap: &str) -> Value {
    json!({"target":target,"resolution":resolution,"capabilities":caps,"withdraws":"process","because":[{"kind":kind,"capability":cap}],"scope":null})
}
pub fn scoped(target: &str, actor: &str, people: &[&str], resources: &[&str]) -> Value {
    json!({"target":target,"resolution":"capability","capabilities":[target],"withdraws":"scoped","because":[],"scope":{"actors":[actor],"capabilities":[target],"people":people,"resources":resources}})
}
pub fn effects(entries: Vec<Value>) -> Value {
    let process = entries.iter().any(|e| e["withdraws"] == "process");
    json!({"process":process,"entries":entries})
}
pub fn expected(k: u8) -> Option<Value> {
    let entries = match k {
        0 | 6 => return None,
        1 => vec![opaque("Ghost")],
        2 => vec![opaque("A.nosuch")],
        3 => vec![call("R.y", "capability", &["R.y"], "call", "R.y")],
        4 => vec![scoped("R.y", "R", &["q"], &["Desk"])],
        5 => vec![call("R", "actor", &["R.y", "R.z"], "call", "R.y")],
        7 => vec![opaque("Ghost"), scoped("R.y", "R", &["q"], &["Desk"])],
        _ => panic!("closed K population"),
    };
    Some(effects(entries))
}
pub fn warning(target: &str) -> String {
    let hint = match target {
        "A.nosuch" => " (Actor A has no capability nosuch)",
        "Vendor.contest_rate" => " (Actor Vendor has no capability contest_rate)",
        _ => "",
    };
    format!(
        "defer {target} matches no Actor or Actor.capability{hint}: every metrics and quality value is withdrawn as not_configured"
    )
}
pub fn invoice(variant: &str) -> String {
    assert!(INVOICE.contains(RESUBMISSION) && INVOICE.contains(VENDOR));
    let s = INVOICE.replace(RESUBMISSION, "").replace(VENDOR, "");
    let replacement = match variant {
        "P0" => return INVOICE.into(),
        "Pa" => "",
        "Pb" => RESUBMISSION,
        "Pc" => "    defer Clerk.recheck reason \"later\" loses \"load\"\n",
        "Pd" => "    defer Extractor.extract reason \"later\" loses \"load\"\n",
        _ => panic!("closed P population"),
    };
    s.replace("    externality", &format!("{replacement}    externality"))
}
pub fn invoice_expected(variant: &str, plan: &str) -> Option<Value> {
    Some(effects(match variant {
        "P0" => vec![opaque("Resubmission"), opaque("Vendor.contest_rate")],
        "Pa" => return None,
        "Pb" => vec![opaque("Resubmission")],
        "Pc" => vec![call(
            "Clerk.recheck",
            "capability",
            &["Clerk.recheck"],
            "contest_route",
            "Clerk.recheck",
        )],
        "Pd" if plan == "ToBeA" => vec![call(
            "Extractor.extract",
            "capability",
            &["Extractor.extract"],
            "call",
            "Extractor.extract",
        )],
        "Pd" => vec![scoped("Extractor.extract", "Extractor", &[], &[])],
        _ => panic!("closed P population"),
    }))
}
pub fn selected_cuts() -> String {
    source(0).replace("process P {", "cut Opaque { include [A,R]; defer Ghost reason \"later\" loses \"load\" }\nconfiguration Clean { cut C; process P }\nconfiguration Hidden { cut Opaque; process P }\nprocess P {")
}
pub fn comparison(opaque_target: bool) -> String {
    let s = source(if opaque_target { 1 } else { 3 });
    let i = s.rfind('}').unwrap();
    format!(
        "{}\nprocess Q {{ a = A.x(); end done }}\nconfiguration Included {{ cut C; process P }}\nconfiguration Unrelated {{ cut C; process Q }}\ncompare [Included,Unrelated] cut C hard {{ cohort.Rate >= 0.1 }} objectives {{ minimize metrics.overall.work; minimize cohort.Rate; minimize quantile.P95 }}\n}}",
        &s[..i]
    )
}
