//! Operational endorsement, not psychological agreement or an outcome score.
use super::{
    input::{Spec, capability},
    link::{Context, Link, correspondence, push},
};
use crate::{conformance::order::Order, ir};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub(super) struct Rate {
    pub status: &'static str,
    pub classification: &'static str,
    pub reasons: Vec<String>,
}
pub(super) fn status(reasons: &[String]) -> &'static str {
    if reasons.iter().any(|r| r.starts_with("unsupported")) {
        "unsupported"
    } else if reasons.is_empty() {
        "known"
    } else {
        "unknown"
    }
}
pub(super) fn deferred(spec: &Spec, ctx: &Context<'_>, time: bool) -> bool {
    let p = &spec.profile;
    let ops: Vec<_> = [&p.approval_operation, &p.proposal_operation]
        .into_iter()
        .chain(p.reinvestigation_operations.iter().filter(|_| time))
        .collect();
    ctx.model.cut.as_ref().is_some_and(|c| {
        c.defer.iter().any(|e| match &e.target {
            ir::CutTarget::Actor(a) => ops.iter().any(|op| {
                capability(ctx.model, op).is_some_and(|id| ctx.model.capabilities[id.0].actor == *a)
            }),
            ir::CutTarget::Label(label) => ops.contains(&label),
        })
    })
}
/// Strict timestamp separation is positive evidence without an order search.
/// At equal ticks only graph reachability suffices; exhausted search is unknown.
pub(super) fn prior(
    a: usize,
    b: usize,
    ctx: &Context<'_>,
    order: &Order,
    reasons: &mut Vec<String>,
) {
    if ctx.events[a].time.latest < ctx.events[b].time.earliest {
        return;
    }
    match order.precedes(a, b) {
        Some(true) => (),
        Some(false) => push(reasons, "ambiguous_approval_order"),
        None => push(reasons, "order_limit"),
    }
}
pub(super) fn classify(link: &Link<'_>, spec: &Spec, ctx: &Context<'_>, order: &Order) -> Rate {
    let mut reasons = correspondence(link, spec, ctx);
    if deferred(spec, ctx, false) {
        push(&mut reasons, "not_configured");
    }
    if let Some((p, a)) = link
        .proposal
        .and_then(|t| t.finish)
        .zip(link.approval.and_then(|t| t.start))
    {
        prior(p, a, ctx, order, &mut reasons);
    }
    let proposal = link
        .proposal
        .and_then(|t| t.finish)
        .and_then(|i| ctx.events[i].result.as_ref());
    let approval = link
        .approval
        .and_then(|t| t.finish)
        .and_then(|i| ctx.events[i].result.as_ref());
    if proposal.is_none() || approval.is_none() {
        push(&mut reasons, "missing_result");
    }
    let eligible = proposal.is_some_and(|r| spec.profile.proposal_approve_results.contains(r));
    if eligible && approval.is_some_and(|r| !spec.profile.approval_decision_results.contains(r)) {
        push(&mut reasons, "task_correspondence");
    }
    reasons.sort();
    let status = status(&reasons);
    let classification = if status != "known" {
        status
    } else if !eligible {
        "ineligible"
    } else if approval.is_some_and(|r| spec.profile.approval_approve_results.contains(r)) {
        "endorsed"
    } else {
        "not_endorsed"
    };
    Rate {
        status,
        classification,
        reasons,
    }
}

#[cfg(test)]
mod tests {
    use super::super::link::{census, with_case};
    use super::*;
    #[test]
    fn i3_rate_matches_fixed_hand_counts_without_execution() {
        for (name, n, d, i, u) in [
            ("a01_baseline", 2, 3, 0, 0),
            ("a01_approved_control", 3, 3, 0, 0),
            ("a02_ineligible", 2, 3, 1, 0),
            ("a02_eligible_control", 2, 4, 0, 0),
            ("a03_unobserved_case", 2, 3, 0, 0),
            ("a04_null", 2, 3, 0, 0),
            ("a09_cancel_ack", 0, 0, 0, 1),
            ("a10_two_decisions", 1, 2, 0, 0),
            ("a10_duplicate_finish", 1, 2, 0, 1),
            ("a11_missing_link", 1, 2, 0, 1),
            ("a12_version_mismatch", 1, 2, 0, 1),
            ("a12_missing_version", 1, 2, 0, 1),
            ("a13_zero_denominator", 0, 0, 3, 0),
            ("a14_reverse", 2, 3, 0, 0),
        ] {
            with_case(name, |s, c| {
                let x = census(s, c).unwrap();
                let order = Order::source(c.events, false).unwrap();
                let mut got = (0, 0, 0, 0);
                for l in &x.links {
                    match classify(l, s, c, &order).classification {
                        "endorsed" => {
                            got.0 += 1;
                            got.1 += 1;
                        }
                        "not_endorsed" => got.1 += 1,
                        "ineligible" => got.2 += 1,
                        _ => got.3 += 1,
                    }
                }
                assert_eq!(got, (n, d, i, u), "{name}");
            });
        }
    }
}
