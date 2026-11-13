//! Resolve semantic operands by declared names, never accidental table IDs.
use super::{Result, unsupported};
use crate::ir;
use serde_json::{Value, json};

pub(crate) struct Names<'a>(pub &'a ir::Model);
impl Names<'_> {
    pub fn actor(&self, id: ir::ActorId) -> Result<String> {
        self.0
            .actors
            .get(id.0)
            .map(|a| a.name.clone())
            .ok_or_else(|| unsupported("invalid Actor reference"))
    }
    pub fn capability(&self, id: ir::CapabilityId) -> Result<String> {
        let c = self
            .0
            .capabilities
            .get(id.0)
            .ok_or_else(|| unsupported("invalid Capability reference"))?;
        Ok(format!("{}.{}", self.actor(c.actor)?, c.name))
    }
    pub fn ty(&self, id: ir::TypeId) -> Result<String> {
        if id == ir::CASE_ID_TYPE {
            return Ok("CaseId".into());
        }
        self.0
            .types
            .get(id.0)
            .map(|t| t.name.clone())
            .ok_or_else(|| unsupported("invalid type reference"))
    }
    pub fn value(&self, ty: ir::TypeId, id: ir::ValueId) -> Result<String> {
        self.0
            .types
            .get(ty.0)
            .and_then(|t| t.members.get(id.0))
            .cloned()
            .ok_or_else(|| unsupported("invalid enum value reference"))
    }
    pub fn state(&self, id: ir::StateVarId) -> Result<String> {
        let v = self
            .0
            .state_variables
            .get(id.0)
            .ok_or_else(|| unsupported("invalid state reference"))?;
        let owner = match v.kind {
            ir::StateKind::ObservedCase => "case.observed".into(),
            ir::StateKind::LatentCase => "case.latent".into(),
            ir::StateKind::ActorObserved(a) => format!("actor.observed.{}", self.actor(a)?),
            ir::StateKind::ActorLatent(a) => format!("actor.latent.{}", self.actor(a)?),
            ir::StateKind::Rule(a) => format!("actor.rule.{}", self.actor(a)?),
        };
        Ok(format!("{owner}.{}", v.name))
    }
    pub fn state_value(&self, id: ir::StateVarId, value: ir::StateValue) -> Result<Value> {
        let v = self
            .0
            .state_variables
            .get(id.0)
            .ok_or_else(|| unsupported("invalid state value owner"))?;
        match value {
            ir::StateValue::Value(x) => Ok(json!(self.value(v.base, x)?)),
            ir::StateValue::None if v.optional => Ok(Value::Null),
            ir::StateValue::Integer(x) if v.integer_max.is_some() => Ok(json!(x)),
            ir::StateValue::Set(x) if v.case_set_size.is_some() => Ok(json!({"case_set_bits":x})),
            _ => Err(unsupported("non-enum state value projection")),
        }
    }
    pub fn binding(&self, id: ir::BindingId) -> Result<String> {
        self.0
            .process
            .bindings
            .get(id.0)
            .map(|b| b.name.clone())
            .ok_or_else(|| unsupported("invalid binding reference"))
    }
    pub fn end(&self, id: ir::EndId) -> Result<String> {
        self.0
            .ends
            .get(id.0)
            .cloned()
            .ok_or_else(|| unsupported("invalid end reference"))
    }
    pub fn resource(&self, id: ir::ResourceId) -> Result<String> {
        self.0
            .resources
            .get(id.0)
            .map(|r| r.name.clone())
            .ok_or_else(|| unsupported("invalid resource reference"))
    }
    pub fn result_match(&self, c: &ir::ResultMatch) -> Result<Value> {
        let b = self
            .0
            .process
            .bindings
            .get(c.binding.0)
            .ok_or_else(|| unsupported("invalid result binding"))?;
        Ok(json!([
            self.binding(c.binding)?,
            self.value(b.ty, c.value)?
        ]))
    }
    pub fn condition(&self, c: &ir::Cond, depth: usize) -> Result<Value> {
        if depth > 128 {
            return Err(unsupported("condition projection depth"));
        }
        Ok(match c {
            ir::Cond::Eq(x) => json!(["result_eq", self.result_match(x)?]),
            ir::Cond::StateEq(x) => json!([
                "state_eq",
                self.state(x.variable)?,
                self.state_value(x.variable, x.value)?
            ]),
            ir::Cond::And(cs) | ir::Cond::Or(cs) => json!([
                if matches!(c, ir::Cond::And(_)) {
                    "and"
                } else {
                    "or"
                },
                cs.iter()
                    .map(|c| self.condition(c, depth + 1))
                    .collect::<Result<Vec<_>>>()?
            ]),
            ir::Cond::IntCompare {
                variable,
                operator,
                value,
            } => json!([
                "integer_compare",
                self.state(*variable)?,
                match operator {
                    ir::IntComparison::Eq => "==",
                    ir::IntComparison::Ne => "!=",
                    ir::IntComparison::Lt => "<",
                    ir::IntComparison::Le => "<=",
                    ir::IntComparison::Gt => ">",
                    ir::IntComparison::Ge => ">=",
                },
                value
            ]),
            ir::Cond::Contains { set, element } => json!([
                "contains",
                self.state(*set)?,
                match element {
                    ir::Element::CurrentCase => json!(["current_case"]),
                    ir::Element::Parameter(i) => json!(["parameter", i]),
                }
            ]),
        })
    }
    pub fn writes(&self, writes: &[ir::StateWrite]) -> Result<Value> {
        Ok(json!(
            writes
                .iter()
                .map(|w| {
                    let source = match w.source {
                        ir::WriteSource::Result => json!(["result"]),
                        ir::WriteSource::Constant(v) => {
                            json!(["constant", self.state_value(w.variable, v)?])
                        }
                        ir::WriteSource::Executor => json!(["executor"]),
                        ir::WriteSource::InsertCase { parameter } => {
                            json!(["insert_case", parameter])
                        }
                        ir::WriteSource::Integer {
                            operator, operand, ..
                        } => json!([
                            "integer",
                            match operator {
                                ir::AssignmentOp::Assign => "=",
                                ir::AssignmentOp::Add => "+=",
                                ir::AssignmentOp::Subtract => "-=",
                            },
                            operand
                        ]),
                    };
                    Ok(json!([self.state(w.variable)?, source]))
                })
                .collect::<Result<Vec<_>>>()?
        ))
    }
    fn call(&self, c: &ir::Call) -> Result<Value> {
        Ok(json!([
            self.binding(c.binding)?,
            self.capability(c.capability)?,
            c.args
                .iter()
                .map(|id| if *id == ir::CURRENT_CASE {
                    Ok("@current_case".to_owned())
                } else {
                    self.binding(*id)
                })
                .collect::<Result<Vec<_>>>()?
        ]))
    }
    pub fn block(&self, body: &ir::Block, depth: usize) -> Result<Value> {
        if depth > 128 {
            return Err(unsupported("control-tree projection depth"));
        }
        Ok(json!(
            body.iter()
                .filter(|s| !matches!(&s.kind, ir::Stmt::Response(_, r) if r.attempt != 1))
                .map(|s| Ok(match &s.kind {
                    ir::Stmt::LoopControl(_) =>
                        return Err(unsupported("loop_feature_combination")),
                    ir::Stmt::Call(c) => json!(["call", self.call(c)?]),
                    ir::Stmt::Query(c, command) => {
                        let source = self.0.capabilities[c.capability.0]
                            .receipt_query
                            .as_ref()
                            .ok_or_else(|| unsupported("query source"))?;
                        json!([
                            "receipt_query",
                            self.call(c)?,
                            self.binding(*command)?,
                            self.capability(source.target)?,
                            source.key_parameter,
                            match source.availability {
                                crate::ast::QueryAvailability::Available => "available",
                                crate::ast::QueryAvailability::Unavailable => "unavailable",
                                crate::ast::QueryAvailability::Nondet => "nondet",
                            }
                        ])
                    }
                    ir::Stmt::ResolveEffect { command, evidence } => json!([
                        "resolve_effect",
                        self.binding(*command)?,
                        self.binding(*evidence)?
                    ]),
                    ir::Stmt::StopReceiptUnknown => json!(["stop_receipt_unknown"]),
                    ir::Stmt::End(e) => json!(["end", self.end(*e)?]),
                    ir::Stmt::Wait(t) => json!(["wait", t]),
                    ir::Stmt::WaitUntil(c) => json!(["wait_until", self.condition(c, 0)?]),
                    ir::Stmt::If {
                        condition,
                        then_body,
                        else_body,
                    } => json!([
                        "if",
                        self.condition(condition, 0)?,
                        self.block(then_body, depth + 1)?,
                        self.block(else_body, depth + 1)?
                    ]),
                    ir::Stmt::Fork { id, branches, join } => {
                        let name = self
                            .0
                            .process
                            .forks
                            .get(id.0)
                            .ok_or_else(|| unsupported("invalid fork reference"))?;
                        let join = match join {
                            ir::Join::All => json!(["all"]),
                            ir::Join::Any {
                                matches,
                                tie,
                                remaining,
                                if_none,
                            } => {
                                let tie = match tie {
                                    ir::Tie::Nondet => json!(["nondet"]),
                                    ir::Tie::By(ids) => json!([
                                        "by",
                                        ids.iter()
                                            .map(|id| self.binding(*id))
                                            .collect::<Result<Vec<_>>>()?
                                    ]),
                                };
                                let remaining = match remaining {
                                    ir::Remaining::Drain => "drain",
                                    ir::Remaining::CancelThenDrain => "cancel_then_drain",
                                };
                                json!([
                                    "any",
                                    matches
                                        .iter()
                                        .map(|m| self.result_match(m))
                                        .collect::<Result<Vec<_>>>()?,
                                    tie,
                                    remaining,
                                    self.end(*if_none)?
                                ])
                            }
                        };
                        json!([
                            "fork",
                            name,
                            branches
                                .iter()
                                .map(|c| self.call(c))
                                .collect::<Result<Vec<_>>>()?,
                            join
                        ])
                    }
                    ir::Stmt::Response(_, r) => {
                        let mut control =
                            serde_json::to_value(crate::response_inputs::control(self.0, r))
                                .map_err(|_| unsupported("response control projection"))?;
                        // Only p and its declared measurement basis may vary.
                        // Channel kind, retry policy and mechanism stay fixed.
                        if matches!(r.loss, ir::ResponseLoss::Probability { .. }) {
                            control["loss"] = json!({"kind":"probability"});
                        }
                        if let Some(body) = &r.recovery {
                            json!([
                                "response",
                                self.call(&r.command_call)?,
                                control,
                                ["recovery", self.block(body, depth + 1)?]
                            ])
                        } else {
                            json!(["response", self.call(&r.command_call)?, control])
                        }
                    }
                }))
                .collect::<Result<Vec<_>>>()?
        ))
    }
    pub fn admission(&self, q: Option<&ir::AdmissionLimit>) -> Result<Value> {
        let Some(q) = q else {
            return Ok(Value::Null);
        };
        let overflow = match q.overflow {
            ir::Overflow::Reject => json!(["reject"]),
            ir::Overflow::WaitExternalFifo => json!(["external_fifo"]),
            ir::Overflow::WaitExternalPriority(t) => json!(["external_priority", queue_tie(t)]),
            ir::Overflow::End(e) => json!(["end", self.end(e)?]),
        };
        Ok(json!({"capacity":q.capacity,"overflow":overflow}))
    }
    pub fn cut(&self) -> Result<Value> {
        let Some(c) = &self.0.cut else {
            return Ok(Value::Null);
        };
        let entries = |rows: &[ir::CutEntry]| -> Result<Vec<Value>> {
            rows.iter()
                .map(|e| {
                    let target = match &e.target {
                        ir::CutTarget::Actor(a) => json!(["actor", self.actor(*a)?]),
                        ir::CutTarget::Label(s) => json!(["label", s]),
                    };
                    Ok(json!({"target":target,"reason":e.reason,"loses":e.loses}))
                })
                .collect()
        };
        Ok(
            json!({"name":c.name,"question":c.question,"include":c.include.iter().map(|id|self.actor(*id)).collect::<Result<Vec<_>>>()?,
            "exclude":entries(&c.exclude)?,"defer":entries(&c.defer)?,"externality":c.externality,"reentry":c.reentry,"horizon":c.horizon}),
        )
    }
}
pub(super) fn queue_tie(t: ir::QueueTie) -> &'static str {
    match t {
        ir::QueueTie::Nondet => "nondet",
        ir::QueueTie::FifoCaseId => "fifo_case_id",
    }
}
