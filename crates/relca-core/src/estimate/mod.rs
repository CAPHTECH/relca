//! ADR 0082: derive candidates from observations without running or changing a model.
mod model;
mod samples;
mod stats;
mod wire;

use crate::{
    Verdict,
    conformance::{identity, input, lifecycle, order::Order},
    diag, lower,
    probability::Rational,
    report, syntax,
};
use model::{Condition, Definition, Kind, Site};
use samples::Observations;
use serde_json::json;
use sha2::{Digest, Sha256};
use stats::{Calculator, fraction};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Read,
};
pub use wire::EstimateReport;
use wire::{Round, Row, Spec, Stats, Status};

pub fn estimate(
    src: &str,
    log: impl Read,
    spec: impl Read,
    configuration: Option<&str>,
) -> EstimateReport {
    let mut out = estimate_raw(src, log, spec, configuration);
    diag::locate(src, out.diagnostics.iter_mut().map(|d| &mut d.diagnostic));
    out
}

fn estimate_raw(
    src: &str,
    log: impl Read,
    spec: impl Read,
    configuration: Option<&str>,
) -> EstimateReport {
    let mut out = EstimateReport::default();
    out.configuration = configuration.map(str::to_owned);
    let sha: String = Sha256::digest(src.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    out.model.sha256 = Some(sha.clone());
    let spec = match Spec::read(spec) {
        Ok(s) => s,
        Err(reason) => {
            out.fail(Verdict::Invalid, diag::E_LOG_0, reason);
            return out;
        }
    };
    if spec.model_sha256 != sha {
        out.fail(Verdict::Invalid, diag::E_LOG_0, "spec_hash_mismatch");
        return out;
    }
    let mut parsed = input::Log::default();
    let loaded = input::read_into(log, &mut parsed);
    out.log = (&parsed).into();
    if let Err(f) = loaded {
        out.log_failure(f);
        return out;
    }
    let truncated = parsed.limit.is_some();
    if parsed
        .sha256
        .as_ref()
        .is_some_and(|h| h != &spec.log_sha256)
    {
        out.fail(Verdict::Invalid, diag::E_LOG_0, "spec_hash_mismatch");
        return out;
    }
    let ast = match syntax::parse(src) {
        Ok(x) => x,
        Err(ds) => {
            out.verdict = report::fold_verdict(&[], &ds);
            out.diagnostics = ds.into_iter().map(Into::into).collect();
            return out;
        }
    };
    out.model.name = Some(ast.name.value.clone());
    let m = match lower::lower_with_selection(&ast, configuration, None) {
        Ok(x) => x,
        Err(ds) => {
            out.verdict = report::fold_verdict(&[], &ds);
            out.diagnostics = ds.into_iter().map(Into::into).collect();
            return out;
        }
    };
    out.diagnostics = m.diagnostics.iter().cloned().map(Into::into).collect();
    let definitions = model::slots(src, &ast, &m);
    let ids: BTreeSet<_> = definitions.iter().map(|d| d.slot.id.as_str()).collect();
    if !spec.slots.known(|id| ids.contains(id)) {
        out.fail(Verdict::Invalid, diag::E_LOG_0, "unknown_slot");
        return out;
    }
    let header = parsed.header.as_ref().expect("valid log has a header");
    let resolved = match identity::resolve(&m, header, &parsed.events) {
        Ok(x) => x,
        Err(f) => {
            out.log_failure(f);
            return out;
        }
    };
    let mut order = match Order::source(&parsed.events, truncated) {
        Ok(x) => x,
        Err(f) => {
            out.log_failure(f);
            return out;
        }
    };
    if truncated {
        out.truncated = true;
        out.reason = Some("truncated");
        out.slots = definitions
            .into_iter()
            .map(|d| Row::new(d.slot, Stats::blank(Status::Withheld, "truncated")))
            .collect();
        return out;
    }
    if m.has_merges() {
        out.fail(
            Verdict::Unsupported,
            diag::U_LOG_1,
            "merge_feature_combination",
        );
        let d = &mut out
            .diagnostics
            .last_mut()
            .expect("merge diagnostic")
            .diagnostic;
        d.span = Some(m.process.span);
        *d = d.clone().described(
            "merge_feature_combination",
            "estimate_merge_log",
            diag::DetailKind::AnalysisNotConnected,
            m.process
                .bindings
                .iter()
                .filter(|b| b.merge.is_some())
                .map(|b| diag::Blocker::at("merge_binding", Some(b.name.clone()), Some(b.span)))
                .collect(),
            "docs/guide/results.md#ログから導いた候補",
        );
        return out;
    }
    let assembled = lifecycle::analyze(&m, &parsed.events, &resolved);
    if let Err(f) = order.add_semantic(&assembled.edges) {
        out.log_failure(f);
        return out;
    }
    let observations = samples::collect(
        &parsed.events,
        &assembled.tasks,
        &resolved,
        m.capabilities.len(),
    );
    out.unmapped_actor = resolved.unmapped_actor;
    out.unmapped_operation = resolved.unmapped_operation;

    let width = header.window.to - header.window.from;
    let planned: u128 = definitions
        .iter()
        .filter(|d| spec.slots.includes(&d.slot.id))
        .map(|d| d.planned_branches(width))
        .sum();
    let alpha = (planned > 0)
        .then(|| Rational((Rational::one().0 - &spec.confidence.0) / fraction(2 * planned, 1).0));
    let family = u64::try_from(planned).map_or_else(|_| json!(planned.to_string()), |m| json!(m));
    out.multiplicity = Some(
        json!({"confidence":spec.confidence,"m":family,"alpha":alpha,"interval_method":"clopper-pearson","method":"bonferroni"}),
    );
    let mut calculator = Calculator::new(&spec, &out.log, alpha);
    let unknown_operations: BTreeSet<_> = out
        .unmapped_operation
        .iter()
        .flat_map(|r| r.case_ids.iter().map(String::as_str))
        .collect();
    let mut groups = BTreeMap::new();
    for d in definitions {
        let selected = spec.slots.includes(&d.slot.id);
        let row = if selected {
            let obs = d.cap.map(|i| &observations[i]);
            let rounds =
                if let (Some(cap), Some(o), Site::Repeat { limit, .. }) = (d.cap, obs, d.site) {
                    Some(
                        groups
                            .entry(cap)
                            .or_insert_with(|| o.rounds(header, limit, &unknown_operations))
                            .as_slice(),
                    )
                } else {
                    None
                };
            evaluate(d, obs, rounds, header, &mut calculator)
        } else {
            Row::new(d.slot, Stats::blank(Status::NotRequested, "not_requested"))
        };
        let mut record = |stats: &Stats, round: Option<usize>| {
            if stats.status == Status::Derived && !stats.candidate.is_null() {
                let source = format!("log sha256:{} slot:{}", spec.log_sha256, row.slot.id);
                let source =
                    round.map_or_else(|| source.clone(), |n| format!("{source} round:{n}"));
                out.assumption_records.push(json!({"target":row.slot.target,"value":stats.candidate.to_string(),"status":"measured",
                    "basis":"観測窓から導いた候補であり、モデルに書かれた値ではない","source":source}));
            }
        };
        if row.stats.candidate.is_null() {
            for round in row.rounds.iter().flatten() {
                record(&round.stats, Some(round.round));
            }
        } else {
            record(
                &row.stats,
                row.rounds.as_ref().and_then(|r| r.first()).map(|r| r.round),
            );
        }
        out.slots.push(row);
    }
    let slots = &out.slots;
    let first = slots
        .iter()
        .find(|r| r.stats.status == Status::NotDerivable);
    if let Some(r) = first {
        out.verdict = Verdict::Unsupported;
        out.reason = r.stats.reason;
    } else if slots.iter().all(|r| r.stats.status == Status::NotRequested) {
        out.reason = Some("no_evidence");
    } else if let Some(r) = slots.iter().find(|r| r.stats.incomplete()) {
        out.reason = r.stats.reason.or(Some("partial_coverage"));
    } else if !out.unmapped_actor.is_empty() {
        out.reason = Some("unmapped_actor");
    } else if !out.unmapped_operation.is_empty() {
        out.reason = Some("unmapped_operation");
    } else if header
        .cases
        .iter()
        .any(|c| c.from_case_start != Some(true) || c.through_case_end != Some(true))
    {
        out.reason = Some("partial_coverage");
    } else {
        out.verdict = Verdict::Satisfied;
    }
    out
}

fn evaluate(
    d: Definition<'_>,
    obs: Option<&Observations<'_>>,
    rounds: Option<&[samples::Group<'_>]>,
    header: &input::Header,
    calc: &mut Calculator<'_>,
) -> Row {
    let shape = match &d.kind {
        Kind::Unsupported(reason)
        | Kind::Kernel {
            condition: Condition::Unsupported(reason),
            ..
        } => Some(*reason),
        _ => None,
    };
    let site_error = match d.site {
        Site::Unusable(reason) => Some(reason),
        _ => None,
    };
    if let Some(reason) = shape.or(site_error) {
        let mut row = Row::new(d.slot, Stats::blank(Status::NotDerivable, reason));
        if let Some(o) = obs {
            if matches!(reason, "round_not_identifiable" | "ambiguous_call_site") {
                row.occurrences_per_case = Some(o.occurrences(header));
            } else if matches!(
                reason,
                "latent_condition"
                    | "observed_condition"
                    | "branches_not_distinguishable"
                    | "nondeterministic"
            ) {
                row.stats.marginal_counts = o.marginal_counts();
            }
            row.stats.provenance = calc.provenance(o.evidence_lines());
        }
        return row;
    }
    let compute = |calc: &mut Calculator<'_>, group: &samples::Group<'_>| match &d.kind {
        Kind::Kernel { branches, .. } => calc.kernel(group, branches),
        Kind::Duration(cap) => calc.duration(group, cap),
        Kind::Unsupported(_) => unreachable!("handled structural refusal"),
    };
    if let Some(groups) = rounds {
        if let Kind::Kernel {
            condition: Condition::Round(r),
            ..
        } = &d.kind
        {
            let stats = compute(calc, &groups[r - 1]);
            let child = Round {
                round: *r,
                stats: stats.clone(),
            };
            let mut row = Row::new(d.slot, stats);
            row.rounds = Some(vec![child]);
            return row;
        }
        let round_rows: Vec<_> = groups
            .iter()
            .enumerate()
            .map(|(i, g)| Round {
                round: i + 1,
                stats: compute(calc, g),
            })
            .collect();
        let mut stats = match d.kind {
            Kind::Kernel { .. } => {
                Stats::blank(Status::NotDerivable, "round_pooling_not_supported")
            }
            Kind::Duration(_) => {
                let failed = round_rows.iter().find(|r| r.stats.incomplete());
                match failed {
                    Some(r) => Stats::blank(
                        Status::Withheld,
                        r.stats.reason.unwrap_or("partial_coverage"),
                    ),
                    None => Stats {
                        status: Status::Derived,
                        ..Stats::default()
                    },
                }
            }
            Kind::Unsupported(_) => unreachable!("handled structural refusal"),
        };
        stats.provenance = calc.provenance(obs.expect("rounds have a capability").evidence_lines());
        let mut row = Row::new(d.slot, stats);
        row.rounds = Some(round_rows);
        return row;
    }
    let o = obs.expect("handled structural refusal");
    let (group, multiple) = o.ordinary();
    let stats = if multiple {
        Stats {
            provenance: calc.provenance(o.evidence_lines()),
            ..Stats::blank(Status::Withheld, "multiple_observations_per_case")
        }
    } else {
        compute(calc, &group)
    };
    Row::new(d.slot, stats)
}
