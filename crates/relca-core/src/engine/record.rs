//! Independent public-IR boundary for finite records. Runtime values and
//! conditions remain the ordinary ValueId and Eq/StateEq/Or representation.
use super::*;

pub(super) fn validate(model: &ir::Model) -> EngineResult<()> {
    for ty in &model.types {
        let Some(shape) = &ty.record else {
            continue;
        };
        if shape.fields.is_empty() {
            return Err("empty record shape".into());
        }
        let mut names = BTreeSet::new();
        let mut size = 1usize;
        for f in &shape.fields {
            let domain = model.types.get(f.ty.0).ok_or("invalid record field type")?;
            if f.name.is_empty()
                || !names.insert(&f.name)
                || domain.record.is_some()
                || matches!(domain.name.as_str(), "PersonId" | "CaseId")
                || domain.members.is_empty()
                || domain.members.iter().collect::<BTreeSet<_>>().len() != domain.members.len()
            {
                return Err("invalid record field".into());
            }
            size = size
                .checked_mul(domain.members.len())
                .filter(|n| *n <= 1024)
                .ok_or("record domain exceeds the supported size")?;
        }
        if ty.members.len() != size || ty.members.iter().collect::<BTreeSet<_>>().len() != size {
            return Err("invalid record domain size".into());
        }
        for (id, actual) in ty.members.iter().enumerate() {
            let mut index = id;
            let mut fields = Vec::new();
            for f in shape.fields.iter().rev() {
                let domain = &model.types[f.ty.0].members;
                fields.push(format!("{} = {}", f.name, domain[index % domain.len()]));
                index /= domain.len();
            }
            fields.reverse();
            if *actual != format!("{} {{ {} }}", ty.name, fields.join(", ")) {
                return Err("invalid canonical record member".into());
            }
        }
    }
    for v in &model.state_variables {
        if model
            .types
            .get(v.base.0)
            .is_some_and(|t| t.record.is_some())
            && (v.kind != ir::StateKind::ObservedCase
                || v.integer_max.is_some()
                || !matches!(v.initial, ir::InitialValue::Fixed(x) if valid_state_value(v, x, model)))
        {
            return Err("unsupported record state".into());
        }
    }
    let is_record = |ty: ir::TypeId| model.types.get(ty.0).is_some_and(|t| t.record.is_some());
    for c in &model.capabilities {
        if let Some(a) = &c.atomic {
            if is_record(c.return_type) {
                return Err("atomic with record result is unsupported".into());
            }
            for w in &a.writes {
                let v = model
                    .state_variables
                    .get(w.variable.0)
                    .ok_or("invalid atomic write")?;
                if is_record(v.base) {
                    return Err("atomic record write is unsupported".into());
                }
            }
        }
    }
    for k in model
        .kernels
        .iter()
        .chain(model.capabilities.iter().flat_map(|c| c.result_kernels()))
    {
        for id in &k.given {
            let v = model
                .state_variables
                .get(id.0)
                .ok_or("invalid kernel given")?;
            if model
                .types
                .get(v.base.0)
                .is_some_and(|t| t.record.is_some())
            {
                return Err("record in kernel given is unsupported".into());
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{explain as x, reference};
    use std::hash::{Hash, Hasher};

    fn model(source: &str) -> ir::Model {
        crate::lower::lower(&crate::syntax::parse(source).unwrap()).unwrap()
    }
    const PRODUCT: &str = include_str!("../../../../fixtures/rec-prob-field/model.relca");

    #[test]
    fn record_private_eq_hash_keeps_six_values_none_bindings_and_pending_finish() {
        let m = model(PRODUCT);
        let id = m
            .state_variables
            .iter()
            .position(|v| v.name == "pair")
            .unwrap();
        let seed = State::initial(&m, 0);
        let digest = |s: &State| {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            s.hash(&mut h);
            h.finish()
        };
        // Change exactly one component at a time, so the whole-state key cannot
        // hide a missing state bank behind a differing binding or control PC.
        for bindings in [false, true] {
            let mut states = HashSet::new();
            let mut hashes = BTreeSet::new();
            for value in std::iter::once(None).chain((0..6).map(|n| Some(ir::ValueId(n)))) {
                let mut s = seed.clone();
                if bindings {
                    s.results[0] = value;
                } else {
                    s.state_values[id] = value.map_or(ir::StateValue::None, ir::StateValue::Value);
                }
                hashes.insert(digest(&s));
                states.insert(s);
            }
            assert_eq!(states.len(), 7);
            assert_eq!(hashes.len(), 7);
        }
        let mut pending = seed.clone();
        pending.tasks.push(Task {
            frozen_performance: None,
            stale: Vec::new(),
            executor: None,
            binding: ir::BindingId(0),
            capability: ir::CapabilityId(0),
            lifecycle: Lifecycle::Running {
                finish: 1,
                ack: None,
            },
        });
        let mut finished = pending.clone();
        finished.tasks[0].lifecycle = Lifecycle::Done;
        assert_ne!(pending, finished);
        assert_ne!(digest(&pending), digest(&finished));
        // A current case alias must remain separate from the historical result.
        let mut old = seed.clone();
        old.results[0] = Some(ir::ValueId(0));
        old.state_values[id] = ir::StateValue::Value(ir::ValueId(0));
        let mut fresh = old.clone();
        fresh.state_values[id] = ir::StateValue::Value(ir::ValueId(1));
        assert_ne!(old, fresh);
        assert_ne!(digest(&old), digest(&fresh));
    }

    fn rejected(m: &ir::Model) {
        assert!(validate(m).is_err());
        assert_eq!(
            reference::explore(m).stop,
            reference::StopReason::SemanticAbort
        );
        let out = super::super::explore(m);
        assert!(
            out.diagnostics
                .iter()
                .any(|d| d.code == crate::diag::U_SEM_0)
        );
        assert!(out.properties.is_empty() && out.terminals.is_empty());
    }

    #[test]
    fn record_private_forgeries_cover_overflow_reserved_person_and_initial_metadata() {
        let good = model(PRODUCT);
        let ty = good.types.iter().position(|t| t.name == "Pair").unwrap();
        let state = good
            .state_variables
            .iter()
            .position(|v| v.name == "pair")
            .unwrap();
        let mut m = good.clone();
        let boolean = m.types[ty].record.as_ref().unwrap().fields[1].ty;
        m.types[ty].record.as_mut().unwrap().fields = (0..usize::BITS + 1)
            .map(|i| ir::RecordField {
                name: format!("f{i}"),
                ty: boolean,
            })
            .collect();
        rejected(&m);
        let mut m = good.clone();
        m.types.push(ir::EnumType {
            name: "PersonId".into(),
            members: vec!["p".into()],
            record: None,
            span: m.process.span,
        });
        m.types[ty].record.as_mut().unwrap().fields[0].ty = ir::TypeId(m.types.len() - 1);
        rejected(&m);
        let mut m = good.clone();
        m.types[ty].record.as_mut().unwrap().fields[0].ty = ir::TypeId(usize::MAX);
        rejected(&m);
        // Public IR strings need not have come from the identifier lexer. Two
        // distinct tuples can therefore print identically unless record-member
        // uniqueness is checked as well as per-index canonical spelling.
        let mut m = good.clone();
        let fields = m.types[ty].record.as_ref().unwrap().fields.clone();
        m.types[fields[0].ty.0].members = vec!["red, flag = false".into(), "red".into()];
        m.types[fields[1].ty.0].members = vec!["true".into(), "false, flag = true".into()];
        m.types[ty].members = vec![
            "Pair { color = red, flag = false, flag = true }".into(),
            "Pair { color = red, flag = false, flag = false, flag = true }".into(),
            "Pair { color = red, flag = true }".into(),
            "Pair { color = red, flag = false, flag = true }".into(),
        ];
        rejected(&m);
        for fault in 0..4 {
            let mut m = good.clone();
            match fault {
                0 => m.state_variables[state].integer_max = Some(1),
                1 => m.state_variables[state].initial = ir::InitialValue::Nondet,
                2 => m.state_variables[state].initial = ir::InitialValue::Distribution(0),
                _ => m.state_variables[state].kind = ir::StateKind::ActorObserved(ir::ActorId(0)),
            }
            rejected(&m);
        }
    }

    #[test]
    fn record_private_replay_accepts_control_then_rejects_independent_tampering() {
        let src = include_str!("../../../../fixtures/rec-explain.broken-bypass/model.relca");
        let m = model(src);
        let (_, capture) = trace::explore(src, &m);
        assert_eq!(capture.failure, None);
        let target = "contract C #1 invariant";
        let (mut steps, case, choices) = capture.shortest(target).unwrap();
        let last = steps.last_mut().unwrap();
        last.violation = Some(x::Violation {
            state_invariant: None,
            id: target.into(),
            property_location: None,
            at_step: last.index,
            trigger_step: None,
            deadline: None,
            related_steps: Vec::new(),
        });
        let replay = |m: &ir::Model, steps: &[x::Stage]| {
            reference::replay::replay_choices(m, steps, target, case, &choices)
        };
        assert_eq!(replay(&m, &steps), Ok(()));
        let finish = steps
            .iter()
            .position(|s| s.kind == x::Kind::Finish && s.binding.as_deref() == Some("e"))
            .unwrap();
        for fault in 0..6 {
            let mut altered = steps.clone();
            match fault {
                0 => altered[finish].result = Some("0".into()),
                1 => {
                    altered[finish]
                        .values
                        .observed
                        .iter_mut()
                        .find(|v| v.name == "entry")
                        .unwrap()
                        .value = "Entry { confidence = low }".into()
                }
                2 => {
                    altered[finish - 1]
                        .values
                        .observed
                        .iter_mut()
                        .find(|v| v.name == "entry")
                        .unwrap()
                        .value = "Entry { confidence = high }".into()
                }
                3 => {
                    altered[finish]
                        .values
                        .observed
                        .iter_mut()
                        .find(|v| v.name == "entry")
                        .unwrap()
                        .value = "none".into()
                }
                4 => altered[finish].location.span.start += 1,
                _ => altered[finish].time += 1,
            }
            assert!(replay(&m, &altered).is_err(), "tamper {fault}");
        }
        let mut bad_id = m.clone();
        bad_id.capabilities[0].results = vec![ir::ValueId(1)];
        assert!(
            replay(&bad_id, &steps).is_err(),
            "valid but different ValueId"
        );
        let mut bad_shape = m.clone();
        let t = bad_shape
            .types
            .iter_mut()
            .find(|t| t.record.is_some())
            .unwrap();
        t.record.as_mut().unwrap().fields[0].ty = ir::TypeId(usize::MAX);
        assert!(
            replay(&bad_shape, &steps).is_err(),
            "shape guard at replay entry"
        );
    }
}
