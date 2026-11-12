//! Engine-side public IR checks. Not shared with reference validation.
use crate::ir;

pub(super) fn validate(m: &ir::Model) -> super::EngineResult<()> {
    if m.process.degradations.is_empty() {
        return Ok(());
    }
    let invalid = || super::Halt::from("invalid_q6_ir: degradation correspondence");
    if m.process.degradations.len() != 1
        || m.arrival_distribution.is_some()
        || !m.contests().is_empty()
    {
        return Err(invalid());
    }
    fn collect(b: &[ir::Statement]) -> Option<Vec<ir::Call>> {
        let mut out = Vec::new();
        for s in b {
            match &s.kind {
                ir::Stmt::Call(c) => out.push(c.clone()),
                ir::Stmt::Response(_, r)
                    if r.loss == ir::ResponseLoss::None && r.max_attempts == 1 =>
                {
                    out.push(r.command_call.clone())
                }
                ir::Stmt::If {
                    then_body,
                    else_body,
                    ..
                } => {
                    out.extend(collect(then_body)?);
                    out.extend(collect(else_body)?);
                }
                ir::Stmt::End(_) => (),
                _ => return None,
            }
        }
        Some(out)
    }
    collect(&m.process.body).ok_or_else(invalid)?;
    let r = &m.process.degradations[0];
    if r.name.is_empty() || r.producer.0 >= m.capabilities.len() {
        return Err(invalid());
    }
    let sites: Vec<_> = m
        .process
        .body
        .iter()
        .enumerate()
        .filter(|(_, s)| s.span == r.span)
        .collect();
    if sites.len() != 1 {
        return Err(invalid());
    }
    let (position, s) = sites[0];
    let ir::Stmt::If {
        condition: ir::Cond::Eq(c),
        then_body,
        else_body,
    } = &s.kind
    else {
        return Err(invalid());
    };
    if *c != r.condition
        || c.binding.0 >= m.process.bindings.len()
        || collect(then_body).as_ref() != Some(&r.degraded_calls)
        || collect(else_body).as_ref() != Some(&r.normal_calls)
    {
        return Err(invalid());
    }
    let prefix = &m.process.body[..position];
    // Lower uses the ordinary If definitely-available rule, including arms
    // that End. Do not replace it with a direct-child-only provenance rule.
    let producer = collect(prefix)
        .ok_or_else(invalid)?
        .into_iter()
        .find(|call| call.binding == c.binding)
        .ok_or_else(invalid)?;
    if producer.capability != r.producer {
        return Err(invalid());
    }
    Ok(())
}
