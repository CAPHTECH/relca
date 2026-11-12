//! ADR 0062's optional, closed local wire contract. No scheduler state lives here.
use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Starts {
    pub direction: String,
    pub tie: String,
    pub actors: Vec<String>,
    pub assign_first_idle: bool,
    pub scope: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct External {
    pub direction: String,
    pub tie: String,
    pub scope: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Resolution {
    pub capability: String,
    pub direction: String,
    pub tie: String,
    pub scope: String,
}

macro_rules! policy_fields {
    ($macro:ident) => {
        $macro! {
            rules: String,
            case_ranks: Vec<u64>,
            known_at: String,
            immutable: bool,
            #[serde(deserialize_with = "nullable")]
            starts: Option<Starts>,
            #[serde(deserialize_with = "nullable")]
            external: Option<External>,
            #[serde(deserialize_with = "nullable")]
            version_resolution: Option<Vec<Resolution>>,
        }
    };
}
macro_rules! policy_type {
    ($( $(#[$a:meta])* $name:ident: $ty:ty,)*) => {
        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(try_from = "PolicyWire")]
        pub struct PriorityPolicy { $( pub $name: $ty, )* }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct PolicyWire { $( $(#[$a])* $name: $ty, )* }
        impl TryFrom<PolicyWire> for PriorityPolicy {
            type Error = String;
            fn try_from(w: PolicyWire) -> Result<Self, String> {
                let value = Self { $( $name: w.$name, )* };
                value.validate()?;
                Ok(value)
            }
        }
    };
}
policy_fields!(policy_type);

fn nullable<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<Option<T>, D::Error> {
    Option::deserialize(d)
}

impl PriorityPolicy {
    fn validate(&self) -> Result<(), String> {
        let valid_tie = |t: &str| matches!(t, "nondet" | "fifo_case_id");
        if self.rules != "priority-queues-v1"
            || self.case_ranks.is_empty()
            || self.known_at != "receipt"
            || !self.immutable
        {
            return Err("invalid priority policy header".into());
        }
        if let Some(s) = &self.starts {
            let unique: std::collections::BTreeSet<_> = s.actors.iter().collect();
            if s.direction != "descending"
                || s.scope != "policy_queued_only"
                || (!valid_tie(&s.tie) && s.tie != "fifo_case_id_call_order")
                || unique.len() != s.actors.len()
                || s.actors.iter().any(String::is_empty)
            {
                return Err("invalid priority starts".into());
            }
        }
        if let Some(s) = &self.external
            && (s.direction != "descending"
                || s.scope != "held_readmission_only"
                || !valid_tie(&s.tie))
        {
            return Err("invalid priority external".into());
        }
        if let Some(rows) = &self.version_resolution {
            let unique: std::collections::BTreeSet<_> =
                rows.iter().map(|s| &s.capability).collect();
            if rows.is_empty()
                || rows.len() != unique.len()
                || rows.iter().any(|s| {
                    s.direction != "descending"
                        || s.scope != "due_checked_intersection_only"
                        || !matches!(s.tie.as_str(), "nondet" | "case_id")
                        || s.capability.is_empty()
                })
            {
                return Err("invalid priority resolution".into());
            }
        }
        Ok(())
    }
}

pub(crate) fn record(m: &ir::Model) -> Option<PriorityPolicy> {
    let input = m.case_priority.as_ref()?;
    // A hand-built invalid public IR is refused by the execution entry. Do not
    // panic or fabricate a scope while preparing its diagnostic report.
    if input.classes.policy.iter().any(|a| a.0 >= m.actors.len())
        || m.capabilities.iter().any(|c| c.actor.0 >= m.actors.len())
    {
        return None;
    }
    let tie = |t| match t {
        ir::QueueTie::Nondet => "nondet",
        ir::QueueTie::FifoCaseId => "fifo_case_id",
    };
    let starts = m.start_policy.as_ref().and_then(|p| {
        p.priority.map(|t| Starts {
            direction: "descending".into(),
            tie: if p.call_order {
                "fifo_case_id_call_order"
            } else {
                tie(t)
            }
            .into(),
            actors: input
                .classes
                .policy
                .iter()
                .map(|a| m.actors[a.0].name.clone())
                .collect(),
            assign_first_idle: p.assign_first_idle,
            scope: "policy_queued_only".into(),
        })
    });
    let external = m.external_priority().map(|t| External {
        direction: "descending".into(),
        tie: tie(t).into(),
        scope: "held_readmission_only".into(),
    });
    let rows: Vec<_> = m
        .capabilities
        .iter()
        .filter_map(|c| {
            c.version_check.as_ref()?.resolve.map(|t| Resolution {
                capability: format!("{}.{}", m.actors[c.actor.0].name, c.name),
                direction: "descending".into(),
                tie: match t {
                    ir::ResolutionTie::Nondet => "nondet",
                    ir::ResolutionTie::CaseId => "case_id",
                }
                .into(),
                scope: "due_checked_intersection_only".into(),
            })
        })
        .collect();
    Some(PriorityPolicy {
        rules: "priority-queues-v1".into(),
        case_ranks: input.ranks.clone(),
        known_at: "receipt".into(),
        immutable: true,
        starts,
        external,
        version_resolution: (!rows.is_empty()).then_some(rows),
    })
}

pub(crate) fn comparison_mismatch(a: &ir::Model, b: &ir::Model) -> bool {
    (a.uses_priority() || b.uses_priority()) && record(a) != record(b)
}

pub(super) fn validate_report(r: &Report) -> Result<(), String> {
    let Some(p) = &r.priority_policy else {
        return Ok(());
    };
    if r.bounds.cases != Some(p.case_ranks.len() as u64) {
        return Err("priority case count differs from bounds".into());
    }
    let actors = r
        .actors
        .as_ref()
        .ok_or("priority scopes without lowered Actors")?;
    if let Some(s) = &p.starts {
        let expected: Vec<_> = actors
            .iter()
            .filter(|a| a.control.value == "policy")
            .map(|a| a.name.clone())
            .collect();
        if s.actors != expected {
            return Err("priority start scope differs from Actor controls".into());
        }
    }
    if let Some(rows) = &p.version_resolution {
        for row in rows {
            let (actor, capability) = row
                .capability
                .split_once('.')
                .ok_or("invalid priority capability name")?;
            let declared = actors.iter().find(|a| a.name == actor).is_some_and(|a| {
                a.control.value == "policy"
                    && a.capability
                        .value
                        .as_array()
                        .is_some_and(|cs| cs.iter().any(|c| c == capability))
            });
            if !declared {
                return Err("priority resolution outside declared policy Capability".into());
            }
        }
    }
    let matching: Vec<_> = r
        .assumptions
        .iter()
        .filter(|a| a.field == "case_priority")
        .collect();
    if matching.len() != 1
        || serde_json::from_str::<PriorityPolicy>(&matching[0].value)
            .ok()
            .as_ref()
            != Some(p)
    {
        return Err("priority wire differs from resolved assumptions".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    #[test]
    fn priority_decoder_requires_each_nullable_scope_and_strict_integer_rank() {
        let wire = json!({"rules":"priority-queues-v1","case_ranks":[0,u64::MAX],"known_at":"receipt","immutable":true,
            "starts":null,"external":null,"version_resolution":null});
        let parsed: PriorityPolicy = serde_json::from_value(wire.clone()).unwrap();
        assert_eq!(serde_json::to_value(parsed).unwrap(), wire);
        for key in wire.as_object().unwrap().keys() {
            let mut bad = wire.clone();
            bad.as_object_mut().unwrap().remove(key);
            assert!(
                serde_json::from_value::<PriorityPolicy>(bad).is_err(),
                "missing {key}"
            );
        }
        for rank in [json!(-1), json!(1.5), Value::Null, json!("9")] {
            let mut bad = wire.clone();
            bad["case_ranks"][0] = rank;
            assert!(serde_json::from_value::<PriorityPolicy>(bad).is_err());
        }
        let mut bad = wire;
        bad["version_resolution"] = json!([]);
        assert!(serde_json::from_value::<PriorityPolicy>(bad).is_err());
    }
}
