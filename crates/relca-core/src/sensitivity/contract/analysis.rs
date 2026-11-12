//! Quality definitions, denominator identity and prices are contracts, not
//! sensitivity parameters. Source spans are not semantic operands.
use super::{Result, canonical_names, named, names::Names, unsupported};
use crate::ir;
use serde_json::{Value, json};

impl Names<'_> {
    fn quality_property(&self, id: usize) -> Result<String> {
        self.0
            .quality
            .as_ref()
            .and_then(|q| q.properties.get(id))
            .map(|p| p.name.clone())
            .ok_or_else(|| unsupported("quality property reference"))
    }
    fn cohort(&self, id: usize) -> Result<String> {
        self.0
            .quality
            .as_ref()
            .and_then(|q| q.metrics.get(id))
            .map(|p| p.name.clone())
            .ok_or_else(|| unsupported("cohort reference"))
    }
    pub fn quality(&self) -> Result<Value> {
        let Some(q) = &self.0.quality else {
            return Ok(Value::Null);
        };
        let properties = named(q.properties.iter().map(|p| {
            let target = match &p.target {
                ir::QualityTarget::State(c) => json!(["state", self.condition(c, 0)?]),
                ir::QualityTarget::Ends(es) => {
                    json!(["ends", canonical_names(es.iter().map(|e| self.end(*e)))?])
                }
            };
            Ok((p.name.clone(), json!({"target":target,"within":p.within})))
        }))?;
        let metrics = named(q.metrics.iter().map(|c| {
            let denominator = match c.denominator {
                ir::Denominator::Arrivals => json!(["arrivals"]),
                ir::Denominator::Property(p) => json!(["property", self.quality_property(p)?]),
            };
            Ok((
                c.name.clone(),
                json!({"numerator":self.quality_property(c.numerator)?,"denominator":denominator}),
            ))
        }))?;
        let quantiles = named(q.quantiles.iter().map(|q| Ok((q.name.clone(),json!({
            "p":q.p,"property":self.quality_property(q.property)?,"given_reached":q.given_reached
        })))))?;
        Ok(json!({"properties":properties,"cohorts":metrics,"quantiles":quantiles}))
    }
    pub fn metrics(&self) -> Result<Value> {
        let Some(s) = &self.0.metrics else {
            return Ok(Value::Null);
        };
        if s.actor_rates.len() != self.0.actors.len()
            || s.start_rates.len() != self.0.capabilities.len()
        {
            return Err(unsupported("metrics rate table shape"));
        }
        Ok(
            json!({"window":s.window,"breakdown_waits":s.breakdown_waits,"currency":s.currency,
            "actor_rates":named(s.actor_rates.iter().enumerate().map(|(i,r)| Ok((self.actor(ir::ActorId(i))?,json!(r)))))?,
            "start_rates":named(s.start_rates.iter().enumerate().map(|(i,r)| Ok((self.capability(ir::CapabilityId(i))?,json!(r)))))?,
            "required_actors":canonical_names(s.required_actors.iter().map(|a|self.actor(*a)))?,
            "required_capabilities":canonical_names(s.required_capabilities.iter().map(|c|self.capability(*c)))?}),
        )
    }
    pub fn comparison(&self) -> Result<Value> {
        let Some(s) = &self.0.comparison else {
            return Ok(Value::Null);
        };
        let hard = s
            .hard
            .iter()
            .map(|h| Ok(json!({"cohort":self.cohort(h.cohort)?,"op":h.op.text(),"bound":h.bound})))
            .collect::<Result<Vec<_>>>()?;
        let objectives = s
            .objectives
            .iter()
            .map(|o| {
                let target = match o.target {
                    ir::ObjectiveRef::Cohort(i) => json!(["cohort", self.cohort(i)?]),
                    ir::ObjectiveRef::Quantile(i) => json!([
                        "quantile",
                        self.0
                            .quality
                            .as_ref()
                            .and_then(|q| q.quantiles.get(i))
                            .ok_or_else(|| unsupported("quantile reference"))?
                            .name
                    ]),
                    ir::ObjectiveRef::Overall(x) => json!(["overall", x.text()]),
                };
                Ok(json!({"direction":o.direction,"target":target}))
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(json!({"plans":s.plans,"under":s.under,"cut":s.cut,"hard":hard,"objectives":objectives}))
    }
}
