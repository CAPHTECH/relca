//! Finite event contracts and static authority declarations. IDs are resolved in
//! their owning tables; durations remain outside these obligations.
use super::{Result, named, names::Names, unsupported};
use crate::ir;
use serde_json::{Value, json};
impl Names<'_> {
    pub fn responsibilities(&self) -> Result<Value> {
        use crate::responsibility::{Metadata, Target};
        let Some(Metadata {
            parties,
            standings,
            duties,
            contests,
            responsibilities: _,
            contest_paths: _,
        }) = &self.0.responsibility
        else {
            return Ok(Value::Null);
        };
        if !contests.is_empty() {
            return Err(unsupported("contest sensitivity not projected"));
        }
        let mut parties = parties
            .iter()
            .map(|a| self.actor(*a))
            .collect::<Result<Vec<_>>>()?;
        parties.sort();
        let mut standings = standings
            .iter()
            .map(|s| {
                let target = match s.target {
                    crate::contest::ContestTarget::End(e) => json!(["end", self.end(e)?]),
                    crate::contest::ContestTarget::Commit(c) => {
                        json!(["commit", self.capability(c)?])
                    }
                };
                Ok(json!({"actor":self.actor(s.actor)?,"target":target,"basis":s.basis}))
            })
            .collect::<Result<Vec<_>>>()?;
        standings.sort_by_key(Value::to_string);
        let mut duties = duties
            .iter()
            .map(|d| {
                let target = match &d.target {
                    Target::Capability(c) => json!(["capability", self.capability(*c)?]),
                    Target::Maintenance(a) => json!(["maintenance", self.actor(*a)?]),
                    Target::Condition(c) => json!(["condition", self.condition(c, 0)?]),
                    Target::Formula(f) => json!(["formula", self.duty_formula(f, 0)?]),
                };
                Ok(json!({"person":self.person(d.person)?,"target":target,"within":d.within}))
            })
            .collect::<Result<Vec<_>>>()?;
        duties.sort_by_key(Value::to_string);
        Ok(json!({"parties":parties,"standings":standings,"duties":duties}))
    }
    fn duty_formula(&self, f: &crate::responsibility::Formula, depth: usize) -> Result<Value> {
        use crate::responsibility::Formula;
        if depth > 128 {
            return Err(unsupported("responsibility formula depth"));
        }
        Ok(match f {
            Formula::State(c) => json!(["state", self.condition(c, 0)?]),
            Formula::Bool(x) => json!(["bool", self.condition(&ir::Cond::StateEq(*x), 0)?]),
            Formula::Not(f) => json!(["not", self.duty_formula(f, depth + 1)?]),
            Formula::And(fs) | Formula::Or(fs) => json!([
                if matches!(f, Formula::And(_)) {
                    "and"
                } else {
                    "or"
                },
                fs.iter()
                    .map(|f| self.duty_formula(f, depth + 1))
                    .collect::<Result<Vec<_>>>()?
            ]),
        })
    }
    pub fn person(&self, id: ir::PersonId) -> Result<String> {
        self.0
            .people
            .get(id.0)
            .map(|p| p.name.clone())
            .ok_or_else(|| unsupported("person reference"))
    }
    pub fn people(&self, ids: &[ir::PersonId]) -> Result<Vec<String>> {
        let mut names = ids
            .iter()
            .map(|p| self.person(*p))
            .collect::<Result<Vec<_>>>()?;
        names.sort();
        Ok(names)
    }
    fn authority(&self, id: ir::AuthorityId) -> Result<String> {
        self.0
            .authorities
            .get(id.0)
            .map(|a| a.name.clone())
            .ok_or_else(|| unsupported("authority reference"))
    }
    fn clause(&self, id: ir::ClauseId) -> Result<String> {
        self.0
            .contracts
            .get(id.0)
            .map(|c| c.property_id(id))
            .ok_or_else(|| unsupported("contract reference"))
    }
    fn event(&self, e: ir::Event) -> Result<Value> {
        let c = self
            .0
            .capabilities
            .get(e.capability.0)
            .ok_or_else(|| unsupported("event capability"))?;
        let kind = match e.kind {
            ir::EventKind::Start => json!(["start"]),
            ir::EventKind::Commit => json!(["commit"]),
            ir::EventKind::CancelAck => json!(["cancel_ack"]),
            ir::EventKind::Finish(v) => json!([
                "finish",
                v.map(|v| self.value(c.return_type, v)).transpose()?
            ]),
        };
        Ok(json!([self.capability(e.capability)?, kind]))
    }
    fn trigger(&self, t: ir::Trigger) -> Result<Value> {
        Ok(match t {
            ir::Trigger::Received => json!(["received"]),
            ir::Trigger::Admitted => json!(["admitted"]),
            ir::Trigger::Event(e) => json!(["event", self.event(e)?]),
        })
    }
    fn target(&self, t: ir::Target) -> Result<Value> {
        Ok(match t {
            ir::Target::End(e) => json!(["end", self.end(e)?]),
            ir::Target::Event(e) => json!(["event", self.event(e)?]),
        })
    }
    pub fn verify_name(&self, p: ir::Property) -> Result<String> {
        Ok(match p {
            ir::Property::ProbabilityReached(q) => {
                if q >= self.0.probability_queries.len() {
                    return Err(unsupported("probability query reference"));
                }
                format!("probability reached #{}", q + 1)
            }
            ir::Property::NoResourceOvercommit => "safety no_resource_overcommit".into(),
            ir::Property::ReachableEnd(e) => format!("reachable end.{}", self.end(e)?),
            ir::Property::NoLostUpdate(v) => format!("no_lost_update {}", self.state(v)?),
            ir::Property::StateInRange => "safety state_in_range".into(),
            ir::Property::RecoverableEnd => "recoverable end".into(),
            ir::Property::TerminatingEnd => "terminating end".into(),
            ir::Property::AlmostSureEnd => "almost_sure end".into(),
            ir::Property::Contract(c) => self.clause(c)?,
            ir::Property::Contest(i) => self
                .0
                .contests()
                .get(i)
                .ok_or_else(|| unsupported("contest reference"))?
                .property_id(),
        })
    }
    pub fn contracts(&self) -> Result<Value> {
        use ir::ContractKind as K;
        named(self.0.contracts.iter().enumerate().map(|(i, c)| {
            let kind = match &c.kind {
                K::Invariant { event, condition } => json!([
                    "invariant",
                    self.event(*event)?,
                    condition
                        .as_ref()
                        .map(|c| self.condition(c, 0))
                        .transpose()?
                ]),
                K::StateInvariant {
                    antecedent,
                    condition,
                } => json!([
                    "state_invariant",
                    antecedent
                        .as_ref()
                        .map(|c| self.condition(c, 0))
                        .transpose()?,
                    self.condition(condition, 0)?
                ]),
                K::AtMostOnce { event } => json!(["at_most_once", self.event(*event)?]),
                K::Response {
                    trigger,
                    targets,
                    within,
                } => json!([
                    "response",
                    self.trigger(*trigger)?,
                    targets
                        .iter()
                        .map(|t| self.target(*t))
                        .collect::<Result<Vec<_>>>()?,
                    within
                ]),
                K::NamedResponse {
                    debtor,
                    label,
                    non_waivable,
                    trigger,
                    targets,
                    within,
                } => {
                    let debtor = debtor
                        .map(|d| match d {
                            ir::Debtor::Person(p) => Ok(json!(["person", self.person(p)?])),
                            ir::Debtor::Actor(a) => Ok(json!(["actor", self.actor(a)?])),
                        })
                        .transpose()?;
                    json!([
                        "named_response",
                        debtor,
                        label,
                        non_waivable,
                        self.trigger(*trigger)?,
                        targets
                            .iter()
                            .map(|t| self.target(*t))
                            .collect::<Result<Vec<_>>>()?,
                        within
                    ])
                }
                K::Excludes { before, start } => {
                    json!(["excludes", self.event(*before)?, self.event(*start)?])
                }
                K::RequiresEvent { before, start } => {
                    json!(["requires", self.event(*before)?, self.event(*start)?])
                }
                K::Waive {
                    response,
                    event,
                    reason,
                    authority,
                } => json!([
                    "waive",
                    self.clause(*response)?,
                    self.event(*event)?,
                    reason,
                    self.authority(*authority)?
                ]),
                K::Separation { left, right } => {
                    json!(["separation", self.event(*left)?, self.event(*right)?])
                }
                K::Permitted { capability } => json!(["permitted", self.capability(*capability)?]),
                K::AuthorityBasis {
                    capability,
                    authority,
                } => json!([
                    "authority_basis",
                    self.capability(*capability)?,
                    self.authority(*authority)?
                ]),
                K::ContestResponse { contest } => json!([
                    "contest_response",
                    self.0
                        .contests()
                        .get(*contest)
                        .ok_or_else(|| unsupported("contest response reference"))?
                        .property_id()
                ]),
            };
            Ok((c.property_id(ir::ClauseId(i)), kind))
        }))
    }
    pub fn authorities(&self) -> Result<Value> {
        named(self.0.authorities.iter().map(|a| {
            let r=&a.restrictions;
            let revoke=r.revocable_by.map(|r|match r {ir::AuthorityRevoker::Person(p)=>Ok(json!(["person",self.person(p)?])),ir::AuthorityRevoker::Actor(a)=>Ok(json!(["actor",self.actor(a)?]))}).transpose()?;
            Ok((a.name.clone(),json!({"basis":a.basis,"issued_by":a.issued_by.map(|p|self.person(p)).transpose()?,"not_by":self.people(&a.not_by)?,
                "grants":named(a.grants.iter().map(|g|Ok((self.capability(g.capability)?,json!(self.people(&g.persons)?)))))?,
                "rule_grants":named(a.rule_grants.iter().map(|g|Ok((self.actor(g.rule)?,json!(self.people(&g.persons)?)))))?,
                "under":a.under.as_ref().map(|b|->Result<Value>{Ok(json!([self.actor(b.rule)?,b.version]))}).transpose()?,
                "scope":r.scope.as_ref().map(|c|self.condition(c,0)).transpose()?,"expires":r.expires,"effective_expires":r.effective_expires,
                "revocable_by":revoke,"delegated_from":r.delegated_from.map(|a|self.authority(a)).transpose()?})))
        }))
    }
}
