//! Hand expectations for the explicitly registered Agency-limit family.
#![allow(dead_code)]
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};
#[path = "../../../relca-core/tests/support/agency_vp_fixture_names.rs"]
mod names;
pub use names::NAMES;
fn require(ok: bool, message: &str) -> Result<(), String> {
    if ok { Ok(()) } else { Err(message.into()) }
}
pub fn name(id: &str) -> Option<&str> {
    id.strip_prefix("fixture-").filter(|n| NAMES.contains(n))
}
pub fn validate_set(jobs: &[Value]) -> Result<(), String> {
    let found: Vec<_> = jobs
        .iter()
        .filter_map(|j| j["id"].as_str().and_then(name))
        .collect();
    require(
        found.len() == NAMES.len()
            && found.into_iter().collect::<BTreeSet<_>>() == NAMES.into_iter().collect(),
        "Agency viewpoint fixture census",
    )
}
fn subset(w: &Value, a: &Value, path: &str) -> Result<(), String> {
    match (w, a) {
        (Value::Object(w), Value::Object(a)) => {
            for (k, v) in w {
                subset(
                    v,
                    a.get(k).ok_or_else(|| format!("missing {path}/{k}"))?,
                    &format!("{path}/{k}"),
                )?;
            }
            Ok(())
        }
        (Value::Array(w), Value::Array(a)) => {
            if w.is_empty() {
                return require(a.is_empty(), path);
            }
            let mut used = BTreeSet::new();
            for v in w {
                let i = a
                    .iter()
                    .enumerate()
                    .find(|(i, x)| !used.contains(i) && subset(v, x, path).is_ok())
                    .map(|(i, _)| i)
                    .ok_or_else(|| format!("missing hand row at {path}: {v}"))?;
                used.insert(i);
            }
            Ok(())
        }
        _ => require(w == a, &format!("hand value at {path}: {w} != {a}")),
    }
}
fn count(v: &Value, cap: &str, lo: usize, hi: usize) -> Result<(), String> {
    let row = v["agency"]["viewpoints"]["retention"]["capabilities"]
        .as_array()
        .ok_or("missing retention rows")?
        .iter()
        .find(|r| r["capability"] == cap)
        .ok_or("missing capability")?;
    require(
        row["status"] == "derived" && row["normal"] == json!({"min":lo,"max":hi}),
        &format!("hand exercise {cap}"),
    )
}
fn classes(v: &Value, end: &str) -> Result<BTreeMap<String, String>, String> {
    let f = &v["agency"]["viewpoints"]["framing"];
    require(
        f["status"] == "derived" && f["complete"] == true,
        "complete framing",
    )?;
    let e = f["ends"]
        .as_array()
        .ok_or("missing ends")?
        .iter()
        .find(|e| e["end"] == end)
        .ok_or("missing end")?;
    require(
        e["status"] == "derived" && e["reachable"] == true,
        "reachable end",
    )?;
    let mut out = BTreeMap::new();
    for key in ["selectors", "read_only"] {
        for r in e[key].as_array().ok_or("missing classification rows")? {
            let call = r["call"].as_str().ok_or("missing call")?.to_owned();
            require(!out.contains_key(&call), "duplicate classification")?;
            out.insert(
                call,
                if key == "selectors" {
                    r["selects"].as_str().ok_or("selects")?.to_owned()
                } else {
                    "read_only".into()
                },
            );
        }
    }
    Ok(out)
}
fn warning_contract(v: &Value) -> Result<(), String> {
    let vp = &v["agency"]["viewpoints"];
    let limited: Vec<_> = ["observation", "framing", "retention", "effects"]
        .into_iter()
        .filter(|b| vp[*b]["reason"] == "derivation_limit" && vp[*b]["limit"].is_object())
        .collect();
    let diagnostics = v["diagnostics"].as_array().ok_or("diagnostics")?;
    let warnings: Vec<_> = diagnostics
        .iter()
        .filter(|d| d["code"] == "W-AGY-5")
        .collect();
    require(
        warnings.len() == limited.len(),
        "one warning per final limited block",
    )?;
    for block in limited {
        let r = &vp[block]["limit"];
        let matching: Vec<_> = warnings
            .iter()
            .filter(|d| {
                d["message"]
                    .as_str()
                    .is_some_and(|m| m.contains(&format!("viewpoint {block} not derived:")))
            })
            .collect();
        require(matching.len() == 1, "warning block identity")?;
        require(matching[0]["severity"] == "warning", "warning severity")?;
        let text = matching[0]["message"].as_str().unwrap();
        require(
            r["requested"]
                .as_u64()
                .zip(r["limit"].as_u64())
                .is_some_and(|(r, l)| r > l),
            "over-limit request",
        )?;
        for key in ["counter", "limit", "requested", "phase"] {
            let token = r[key]
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| r[key].to_string());
            require(
                !token.is_empty() && text.contains(&token),
                "warning record consistency",
            )?;
        }
    }
    Ok(())
}
pub fn check(id: &str, args: &[String], v: &Value, exit: i32) -> Result<(), String> {
    let n = name(id).ok_or("unregistered Agency viewpoint job")?;
    require(
        args.get(1) == Some(&format!("fixtures/{n}/model.relca")),
        "fixture source identity",
    )?;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let expected: Value = serde_json::from_slice(
        &std::fs::read(root.join("fixtures").join(n).join("expected.json"))
            .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    subset(&expected, v, "")?;
    require(
        exit == match n {
            "agy-vp-repeat-control.broken-no-leave" => 1,
            "agy-vp-response-control" => 2,
            _ => 0,
        },
        "hand exit",
    )?;
    warning_contract(v)?;
    match n {
        "agy-vp-limit-control" | "agy-vp-limit-control.broken-refusal" => {
            count(v, "Intake.start", 1, 1)?;
            count(v, "Judge.check", 1, 10)?;
            count(v, "Ledger.post", 0, 1)?;
            let broken = n == "agy-vp-limit-control.broken-refusal";
            let done = (1..=10)
                .map(|i| {
                    (
                        format!("j{i}"),
                        if broken && i == 10 {
                            "read_only"
                        } else {
                            "all_contexts"
                        }
                        .into(),
                    )
                })
                .collect();
            let last = if broken { 9 } else { 10 };
            let rejected = (1..=last)
                .map(|i| {
                    (
                        format!("j{i}"),
                        if i == last {
                            "some_contexts"
                        } else {
                            "read_only"
                        }
                        .into(),
                    )
                })
                .collect();
            require(
                classes(v, "done")? == done && classes(v, "rejected")? == rejected,
                "hand gate classifications",
            )?;
        }
        "agy-vp-scale" | "agy-vp-scale.broken-default-budget" => {
            count(v, "Intake.start", 1, 1)?;
            count(v, "Judge.check", 11, 11)?;
            count(v, "Ledger.post", 0, 1)?;
            if n == "agy-vp-scale" {
                for (end, class) in [("done", "all_contexts"), ("rejected", "some_contexts")] {
                    require(
                        classes(v, end)?
                            == (1..=11).map(|i| (format!("j{i}"), class.into())).collect(),
                        "hand And classifications",
                    )?;
                }
            }
        }
        "agy-vp-merge-scale" => {
            for cap in ["Intake.start", "Office.write", "Office.review"] {
                count(v, cap, 1, 1)?;
            }
            count(v, "Office.fix", 0, 1)?;
            count(v, "Judge.check", 7, 7)?;
            count(v, "Ledger.post", 0, 1)?;
            for (end, class) in [("done", "some_contexts"), ("rejected", "all_contexts")] {
                let mut want: BTreeMap<_, _> =
                    (1..=7).map(|i| (format!("j{i}"), class.into())).collect();
                want.insert("r".into(), "read_only".into());
                require(classes(v, end)? == want, "hand merge classifications")?;
            }
        }
        "agy-vp-merge-scale.broken-default-budget" => {
            require(
                v["agency"].get("viewpoints").is_none(),
                "do not fabricate unavailable viewpoints",
            )?;
            require(
                v["diagnostics"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|d| d["code"] == "W-AGY-4")
                    .count()
                    == 1,
                "main limit warning",
            )?;
        }
        "agy-vp-response-control.broken-delivered" => {
            count(v, "Intake.start", 1, 1)?;
            count(v, "Judge.check", 12, 12)?;
            count(v, "Ledger.post", 0, 1)?;
        }
        "agy-vp-response-control" => {
            if v["agency"]["viewpoints"]["retention"]["status"] == "derived" {
                count(v, "Intake.start", 1, 1)?;
                count(v, "Judge.check", 1, 1)?;
                count(v, "Ledger.post", 0, 0)?;
            }
            require(
                v["terminals"].as_array().is_some_and(Vec::is_empty),
                "unknown receipt is not a business terminal",
            )?;
        }
        "agy-vp-repeat-control" | "agy-vp-repeat-control.broken-no-leave" => {}
        _ => return Err("unhandled Agency viewpoint fixture".into()),
    }
    Ok(())
}
