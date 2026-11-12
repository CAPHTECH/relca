//! Explicit observation links and approval burden (ADR 0061).
mod input;
mod link;
mod rate;
mod time;

use crate::conformance::{
    checks::{Check, Tri},
    input::{Failure, Log},
    order::Order,
};
use crate::metrics::MetricNumber;
pub(super) use link::Context;
use serde_json::{Value, json};
use std::io::Read;

pub(super) struct Observation {
    parsed: input::Parsed,
}
pub(super) struct Analysis {
    pub value: Value,
    pub checks: Vec<Check>,
    pub input_limit: Option<&'static str>,
    pub order_limit: bool,
}
pub(super) fn read(reader: impl Read) -> Result<Observation, Failure> {
    input::read(reader).map(|parsed| Observation { parsed })
}
impl Observation {
    pub fn validate(
        &self,
        model: &crate::ir::Model,
        model_hash: &str,
        log: &Log,
    ) -> Result<(), Failure> {
        if let Some(s) = &self.parsed.spec {
            s.validate(model)?;
            // Header membership is already known even when later event lines
            // are cut. A bad sidecar CaseId is not an unknown real-world case.
            if log.header.as_ref().is_some_and(|h| {
                s.episodes
                    .iter()
                    .any(|e| !h.cases.iter().any(|c| c.case_id == e.case_id))
            }) {
                return Err(Failure::invalid("approval_spec_bad_field"));
            }
            // Only complete files get a whole-input identity. A cut log must
            // remain inconclusive rather than fail a prefix hash comparison.
            if s.model_sha256 != model_hash
                || log.sha256.as_ref().is_some_and(|h| h != &s.log_sha256)
            {
                return Err(Failure::invalid("approval_spec_hash_mismatch"));
            }
        }
        Ok(())
    }
    pub fn analyze(
        &self,
        ctx: &Context<'_>,
        order: &Order,
        log_limit: Option<&'static str>,
    ) -> Result<Analysis, Failure> {
        let p = &self.parsed;
        let input_limit = p.limit.or(log_limit);
        let mut checks = Vec::new();
        let mut episodes = Vec::new();
        let mut cases = Vec::new();
        let mut orphans = Vec::new();
        let (mut n, mut d, mut ineligible, mut unknown, mut no_approval) =
            (0u64, 0u64, 0u64, 0u64, 0u64);
        let (mut known_time, mut unknown_time) = (0u64, 0u64);
        let mut sum = MetricNumber::from(0);
        let mut time_reason = None;
        let mut rate_deferred = false;
        let mut order_limit = false;
        // A partial stream cannot establish the full candidate census or the
        // evidence of a sidecar-only Task. Keep the legacy prefix evidence and
        // processed input counts, but publish no episode-level conclusions.
        if let Some(spec) = p.spec.as_ref().filter(|_| input_limit.is_none()) {
            rate_deferred = rate::deferred(spec, ctx, false);
            let census = link::census(spec, ctx)?;
            no_approval = census.no_approval.len() as u64;
            let mut times = Vec::new();
            for l in &census.links {
                let r = rate::classify(l, spec, ctx, order);
                let t = time::measure(l, spec, ctx, order, &census.orphans);
                match r.classification {
                    "endorsed" => {
                        n += 1;
                        d += 1;
                    }
                    "not_endorsed" => d += 1,
                    "ineligible" => ineligible += 1,
                    _ => unknown += 1,
                }
                if let Some(v) = &t.value {
                    sum.0 += &v.0;
                    known_time += 1;
                } else {
                    unknown_time += 1;
                }
                let id = format!("approval:{}:{}", l.case_id, l.approval_id);
                for (suffix, reasons) in [("rate", &r.reasons), ("reinvestigation", &t.reasons)] {
                    let state = if reasons.is_empty() {
                        Tri::Conformant
                    } else {
                        Tri::Unknown
                    };
                    // Emit all reasons, not just the first missing prerequisite.
                    let mut row = Check::new(
                        format!("{id}:{suffix}"),
                        Some(l.case_id),
                        state,
                        reasons.first().map(String::as_str),
                        &l.indices,
                        ctx.events,
                        None,
                    );
                    checks.push(row.clone());
                    for (i, reason) in reasons.iter().enumerate().skip(1) {
                        row.id = format!("{id}:{suffix}:reason:{i}");
                        row.reason = Some(reason.clone());
                        checks.push(row.clone());
                    }
                    order_limit |= reasons.iter().any(|s| s == "order_limit");
                }
                episodes.push(json!({"case_id":l.case_id,"approval_task_id":l.approval_id,
                    "spec_pointer":l.declared.map(|(i,_)| format!("/episodes/{i}")),
                    "rate":r,"reinvestigation":t,"evidence":Check::new("evidence",Some(l.case_id),Tri::Unknown,None,&l.indices,ctx.events,None).evidence}));
                times.push(t);
            }
            if rate::deferred(spec, ctx, true) {
                time_reason = Some("not_configured");
            }
            if time::cross_episode_overlap(&times) {
                time_reason = Some("unsupported_cross_episode_overlap");
                checks.push(Check::new(
                    "approval:reinvestigation:overlap",
                    None,
                    Tri::Unknown,
                    time_reason,
                    &[],
                    ctx.events,
                    None,
                ));
            }
            for task in &census.orphans {
                let mut ids: Vec<_> = task
                    .indices
                    .iter()
                    .map(|i| ctx.events[*i].event_id.as_str())
                    .collect();
                ids.sort_unstable();
                ids.dedup();
                orphans
                    .push(json!({"case_id":task.case_id,"task_id":task.task_id,"event_ids":ids}));
                checks.push(Check::new(
                    format!("approval:{}:unassigned:{}", task.case_id, task.task_id),
                    Some(&task.case_id),
                    Tri::Unknown,
                    Some("unassigned_reinvestigation_task"),
                    &task.indices,
                    ctx.events,
                    None,
                ));
            }
            for scope in &ctx.header.cases {
                let rows: Vec<_> = episodes
                    .iter()
                    .filter(|r| r["case_id"] == scope.case_id)
                    .collect();
                let ru = rows
                    .iter()
                    .filter(|r| r["rate"]["status"] != "known")
                    .count();
                let tu = rows
                    .iter()
                    .filter(|r| r["reinvestigation"]["value"].is_null())
                    .count();
                let absent = rows.is_empty();
                if absent {
                    checks.push(Check::new(
                        format!("approval:{}:unobserved", scope.case_id),
                        Some(&scope.case_id),
                        Tri::Unknown,
                        Some("no_observed_approval"),
                        &[],
                        ctx.events,
                        None,
                    ));
                }
                let partial = absent
                    || ru + tu > 0
                    || scope.from_case_start != Some(true)
                    || scope.through_case_end != Some(true)
                    || census.orphans.iter().any(|t| t.case_id == scope.case_id);
                cases.push(json!({"case_id":scope.case_id,"episodes":rows.len(),"rate_unknown_episodes":ru,"time_unknown_episodes":tu,"no_observed_approval":absent,"coverage":coverage(partial)}));
            }
            cases.sort_by(|a, b| a["case_id"].as_str().cmp(&b["case_id"].as_str()));
        }
        if let Some(reason) = input_limit {
            checks.push(Check::new(
                "approval:input_limit",
                None,
                Tri::Unknown,
                Some(reason),
                &[],
                ctx.events,
                None,
            ));
        } else if d == 0 {
            checks.push(Check::new(
                "approval:rate",
                None,
                Tri::Unknown,
                Some(if rate_deferred {
                    "not_configured"
                } else {
                    "zero_denominator"
                }),
                &[],
                ctx.events,
                None,
            ));
        }
        let partial_scope = ctx
            .header
            .cases
            .iter()
            .any(|s| s.from_case_start != Some(true) || s.through_case_end != Some(true));
        let rate_reason = if input_limit.is_some() {
            Some("truncated")
        } else if rate_deferred {
            Some("not_configured")
        } else if d == 0 {
            Some("zero_denominator")
        } else {
            None
        };
        if input_limit.is_some() {
            time_reason = Some("truncated");
        } else if known_time == 0 && time_reason.is_none() {
            time_reason = Some("no_known_observations");
        }
        let ratio = |n: MetricNumber, d: u64| MetricNumber(n.0 / MetricNumber::from(d).0);
        let rate_value = (rate_reason.is_none()).then(|| ratio(MetricNumber::from(n), d));
        let total = time_reason.is_none().then(|| sum.clone());
        let mean = time_reason.is_none().then(|| ratio(sum, known_time));
        Ok(Analysis {
            value: json!({"schema":"relca-approval-burden/v1","rules":"approval-burden-observed-p2-1",
                "spec":{"sha256":p.sha256,"profile":p.spec.as_ref().map(|s| &s.profile),"status":"declared_not_certified","max_bytes":input::MAX_BYTES,"max_episodes":input::MAX_EPISODES,"processed_episodes":episodes.len(),"truncated":p.limit.is_some()},
                "scope_notice":"Explicit links and observed events only; not log completeness, latent truth, future probability or personal evaluation.",
                "episodes":episodes,"cases":input_limit.is_none().then_some(cases),"unassigned_reinvestigation_tasks":orphans,
                "rate":{"numerator":input_limit.is_none().then_some(n),"denominator":input_limit.is_none().then_some(d),"known_ineligible":input_limit.is_none().then_some(ineligible),"unknown_episodes":input_limit.is_none().then_some(unknown),"no_observed_approval_cases":input_limit.is_none().then_some(no_approval),"value":rate_value,"reason":rate_reason,"coverage":coverage(input_limit.is_some() || unknown+no_approval>0 || partial_scope)},
                "reinvestigation":{"total":total,"mean":mean,"known_episodes":input_limit.is_none().then_some(known_time),"unknown_episodes":input_limit.is_none().then_some(unknown_time),"reason":time_reason,"unit":"person_min","coverage":coverage(input_limit.is_some() || unknown_time+no_approval>0 || partial_scope || time_reason.is_some())}}),
            checks,
            input_limit,
            order_limit,
        })
    }
}
fn coverage(partial: bool) -> &'static str {
    if partial {
        "partial"
    } else {
        "observed_scope_only"
    }
}

#[cfg(test)]
#[allow(dead_code)]
#[path = "../../../../relca-cli/tests/support/approval_wire.rs"]
mod wire;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sidecar_case_outside_header_is_invalid_even_when_log_is_cut() {
        let (mut c, m, log, _) = prepared("a17_event_limit");
        c.spec["episodes"][0]["case_id"] = json!("outside-header");
        let spec = c.spec.to_string();
        let observed = read(spec.as_bytes()).unwrap();
        let log = crate::conformance::input::read(log.as_bytes()).unwrap();
        assert_eq!(log.limit, Some("event_limit"));
        let failure = observed
            .validate(&m, &input::hash(c.model.as_bytes()), &log)
            .unwrap_err();
        assert_eq!(failure, Failure::invalid("approval_spec_bad_field"));
    }
    #[test]
    fn i7_every_new_scope_result_links_schema_rules_and_conclusions() {
        for &name in acceptance::NAMES {
            let (c, _, log, spec) = prepared(name);
            let r = super::super::conform_approval(
                &c.model,
                log.as_bytes(),
                spec.as_bytes(),
                None,
                None,
            );
            let exit = r.verdict.exit_code();
            let value = serde_json::to_value(&r).unwrap();
            r.ledger
                .validate(&value)
                .unwrap_or_else(|e| panic!("{name}: {e}"));
            wire::report(&value, exit).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(exit, c.expected.exit, "{name}: {:?}", r.diagnostics);
        }
        let (c, _, log, spec) = prepared("a01_baseline");
        let r =
            super::super::conform_approval(&c.model, log.as_bytes(), spec.as_bytes(), None, None);
        for (path, version) in [
            ("/schema", "relca-conformance-approval/v9"),
            ("/scope", "future"),
            ("/approval_burden/schema", "relca-approval-burden/v9"),
            ("/approval_burden/rules", "future"),
        ] {
            let mut bad = serde_json::to_value(&r).unwrap();
            *bad.pointer_mut(path).unwrap() = json!(version);
            assert!(r.ledger.validate(&bad).is_err(), "{path}");
        }
    }
    #[test]
    fn i6_core_frozen_acceptance_and_execution_spy_before_ledger_registration() {
        for &name in acceptance::NAMES {
            let (c, _, log, spec) = prepared(name);
            let (r, calls) = crate::evaluation_spy::observe(|| {
                let mut reader = spec.as_bytes();
                super::super::conform_raw(&c.model, log.as_bytes(), None, None, Some(&mut reader))
            });
            assert!(
                calls[1..].iter().all(|n| *n == 0),
                "{name}: must not execute model"
            );
            assert_eq!(
                r.verdict.exit_code(),
                c.expected.exit,
                "{name}: {:?}",
                r.diagnostics
            );
            let v = serde_json::to_value(r).unwrap();
            if let Some(reason) = c.expected.reason {
                assert!(wire::has_reason(&v, reason), "{name}: {v}");
            }
            if c.expected.no_metrics {
                assert!(v["approval_burden"].is_null());
            } else {
                let b = &v["approval_burden"];
                wire::metrics(b).unwrap_or_else(|e| panic!("{name}: {e}"));
                if let Some(n) = &c.expected.numbers {
                    assert_eq!(
                        json!([
                            b["rate"]["numerator"],
                            b["rate"]["denominator"],
                            b["rate"]["value"],
                            b["rate"]["known_ineligible"],
                            b["rate"]["unknown_episodes"],
                            b["rate"]["no_observed_approval_cases"]
                        ]),
                        json!([n.n, n.d, n.rate, n.ineligible, n.unknown, n.no_approval]),
                        "{name}"
                    );
                    assert_eq!(
                        json!([
                            b["reinvestigation"]["total"],
                            b["reinvestigation"]["mean"],
                            b["reinvestigation"]["known_episodes"],
                            b["reinvestigation"]["unknown_episodes"]
                        ]),
                        json!([n.total, n.mean, n.known_time, n.unknown_time]),
                        "{name}"
                    );
                }
            }
        }
    }
    #[test]
    fn i5_wire_and_hand_arithmetic_for_every_observation_case() {
        for &name in acceptance::NAMES {
            let c = acceptance::case(name);
            if c.expected.no_metrics || name == "a17_spec_limit" || name == "a17_episode_limit" {
                continue;
            }
            link::with_case(name, |spec, ctx| {
                let (_, _, _, text) = prepared(name);
                let observed = read(text.as_bytes()).unwrap();
                let order = Order::source(ctx.events, false).unwrap();
                let out = observed.analyze(ctx, &order, None).unwrap();
                wire::metrics(&out.value).unwrap_or_else(|e| panic!("{name}: {e}"));
                assert_eq!(out.value["spec"]["profile"], json!(spec.profile));
                if let Some(n) = &c.expected.numbers {
                    assert_eq!(out.value["rate"]["numerator"], n.n, "{name}");
                    assert_eq!(out.value["rate"]["denominator"], n.d, "{name}");
                    assert_eq!(out.value["rate"]["value"], json!(n.rate), "{name}");
                    assert_eq!(
                        out.value["reinvestigation"]["total"],
                        json!(n.total),
                        "{name}"
                    );
                    assert_eq!(
                        out.value["reinvestigation"]["mean"],
                        json!(n.mean),
                        "{name}"
                    );
                }
            });
        }
    }
}

#[cfg(test)]
#[allow(dead_code)]
#[path = "../../../../relca-cli/tests/support/approval_inputs.rs"]
mod acceptance;

#[cfg(test)]
fn prepared(name: &'static str) -> (acceptance::Case, crate::ir::Model, String, String) {
    let mut c = acceptance::case(name);
    let mut log = c.log();
    if matches!(name, "a17_line_boundary" | "a17_line_limit") {
        let mut lines: Vec<_> = log.lines().map(String::from).collect();
        let n = 65_536 + usize::from(name == "a17_line_limit");
        let padding = n - lines[1].len();
        lines[1].push_str(&" ".repeat(padding));
        log = lines.join("\n") + "\n";
    }
    c.spec["model_sha256"] = input::hash(c.model.as_bytes()).into();
    c.spec["log_sha256"] = if c.hash_mismatch {
        "0".repeat(64)
    } else {
        input::hash(log.as_bytes())
    }
    .into();
    let mut spec = c.raw_spec.clone().unwrap_or_else(|| c.spec.to_string());
    if matches!(name, "a17_spec_boundary" | "a17_spec_limit") {
        let n = input::MAX_BYTES + usize::from(name == "a17_spec_limit");
        spec.push_str(&" ".repeat(n - spec.len()));
    }
    let m = crate::lower::lower(&crate::syntax::parse(&c.model).unwrap()).unwrap();
    (c, m, log, spec)
}
