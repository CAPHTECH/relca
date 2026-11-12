//! Independent validation of named If provenance before reference execution.
use super::*;
use crate::ir;

pub(super) fn validate(m: &Model) -> Execution {
    let routes = &m.process.degradations;
    if routes.is_empty() {
        return Ok(());
    }
    require(routes.len() == 1 && m.arrival_distribution.is_none() && m.contests().is_empty())?;
    fn branches(b: &[ir::Statement], dest: &mut Vec<ir::Call>) -> Execution {
        for s in b {
            match &s.kind {
                Stmt::Call(c) => dest.push(c.clone()),
                Stmt::Response(_, a) => {
                    require(matches!(a.loss, ir::ResponseLoss::None) && a.max_attempts == 1)?;
                    dest.push(a.command_call.clone());
                }
                Stmt::If {
                    then_body,
                    else_body,
                    ..
                } => {
                    branches(then_body, dest)?;
                    branches(else_body, dest)?;
                }
                Stmt::End(_) => (),
                _ => return Err(StopReason::SemanticAbort),
            }
        }
        Ok(())
    }
    branches(&m.process.body, &mut Vec::new())?;
    let r = &routes[0];
    require(!r.name.is_empty() && m.capabilities.get(r.producer.0).is_some())?;
    let mut previous = BTreeMap::new();
    let mut found = false;
    for s in &m.process.body {
        if s.span == r.span {
            require(!found)?;
            found = true;
            let Stmt::If {
                condition: Cond::Eq(test),
                then_body,
                else_body,
            } = &s.kind
            else {
                return Err(StopReason::SemanticAbort);
            };
            require(*test == r.condition && test.binding.0 < m.process.bindings.len())?;
            require(previous.get(&test.binding) == Some(&r.producer))?;
            let mut yes = Vec::new();
            let mut no = Vec::new();
            branches(then_body, &mut yes)?;
            branches(else_body, &mut no)?;
            require(yes == r.degraded_calls && no == r.normal_calls)?;
        }
        // This is correspondence, not a second path-knownness analysis. The
        // existing lower's If rules also cover producers inside a prefix If.
        let mut earlier = Vec::new();
        branches(std::slice::from_ref(s), &mut earlier)?;
        for call in earlier {
            previous.insert(call.binding, call.capability);
        }
    }
    require(found)
}
