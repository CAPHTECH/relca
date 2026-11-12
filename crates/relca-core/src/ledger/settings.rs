use super::*;
use crate::ir;

impl Ledger {
    pub(crate) fn conformance(input_unavailable: bool) -> Self {
        let mut ledger = Self::prelower("log_conformance", input_unavailable);
        ledger.backends["builtin"]["name"] = "builtin_event_contracts".into();
        for field in ["seed", "replications", "confidence", "random_method"] {
            ledger.execution[field] = na("no_analysis");
        }
        ledger.execution["limits"]["state_budget"] = na("no_analysis");
        ledger
    }
    pub(crate) fn aggregate(mode: &str) -> Self {
        let mut ledger = Self::prelower(mode, false);
        ledger.backends["builtin"] =
            json!({"name":"none","version":na("no_analysis"),"status":"not_applicable"});
        ledger.execution["status"] = "not_applicable".into();
        for field in [
            "seed",
            "replications",
            "confidence",
            "random_method",
            "bounds",
        ] {
            ledger.execution[field] = na("aggregate_per_source");
        }
        for field in ["configuration", "scenario", "process"] {
            ledger.execution["selection"]["effective"][field] = na("aggregate_per_source");
        }
        ledger.execution["limits"]["state_budget"] = na("no_analysis");
        ledger.actor_versions =
            json!({"status":"not_applicable","entries":[],"reason":"aggregate_per_source"});
        ledger
    }
    pub(crate) fn simulation_mode(&mut self, input_unavailable: bool) {
        let requested = self.execution["selection"]["requested"].clone();
        *self = Self::prelower("simulation", input_unavailable);
        self.execution["selection"]["requested"] = requested;
    }

    pub(crate) fn simulation_settings(&mut self, m: &ir::Model) {
        self.lowered(m);
        self.execution["limits"]["state_budget"] = na("no_analysis");
        if let Some(spec) = &m.simulation {
            self.execution["seed"] = known(spec.seed);
            self.execution["replications"] = known(spec.replications);
            self.execution["confidence"] = known(spec.confidence.to_string());
            self.execution["random_method"] = known("sha256-counter-v1");
        }
    }

    pub(crate) fn simulation_driver(&mut self) {
        self.backends["builtin"]["name"] = "builtin_simulation".into();
        self.backends["builtin"]["version"] = known(env!("CARGO_PKG_VERSION"));
        if self.execution["status"] == "unavailable" {
            self.execution["status"] = "ran".into();
            self.backends["builtin"]["status"] = "ran".into();
        }
    }
    pub fn requested(&mut self, configuration: Option<&str>, scenario: Option<&str>) {
        self.execution["selection"]["requested"] =
            json!({"configuration":configuration,"scenario":scenario});
    }

    pub(crate) fn lowered(&mut self, m: &ir::Model) {
        // Never index malformed metadata at the report's private injection seam.
        if crate::actor_table::validate(m).is_err() {
            return;
        }
        let requested = &self.execution["selection"]["requested"];
        self.execution["selection"]["effective"] = json!({
            "configuration":requested["configuration"].as_str().map_or_else(undeclared, known),
            "scenario":m.scenario.as_ref().map_or_else(undeclared, |s| known(s.name.clone())),
            "process":known(m.process.name.clone())});
        self.execution["bounds"] = known("/bounds");
        self.execution["limits"]["state_budget"] = unavailable("execution_settings_unavailable");
        let actors = crate::actor_table::project(m)
            .into_iter()
            .map(|a| {
                let via = if a.via_model.status == crate::actor_table::Status::Declared {
                    known(a.via_model.value)
                } else {
                    undeclared()
                };
                let rule = a.rule.map_or_else(undeclared, |r| {
                    known(json!({"declared":r.version.declared,"effective":r.version.effective}))
                });
                json!({"actor":a.name,"via_model":via,"rule":rule})
            })
            .collect::<Vec<_>>();
        self.actor_versions = json!({"status":"known","entries":actors,"reason":null});
    }

    /// An arbitrary injected closure is not evidence of which analyzer ran.
    pub(crate) fn unknown_driver_called(&mut self) {
        self.execution["status"] = "unavailable".into();
        self.backends["builtin"]["status"] = "unavailable".into();
        self.backends["builtin"]["name"] = "none".into();
        self.backends["builtin"]["version"] = na("no_analysis");
    }

    /// Called only by the adapter that actually supplies this budget to engine.
    pub(crate) fn finite_driver(&mut self, budget: usize) {
        self.backends["builtin"]["name"] = "builtin_finite".into();
        self.backends["builtin"]["version"] = known(env!("CARGO_PKG_VERSION"));
        if self.execution["status"] == "unavailable" {
            self.execution["status"] = "ran".into();
            self.backends["builtin"]["status"] = "ran".into();
        }
        if self.execution["status"] == "ran" && self.actor_versions["status"] == "known" {
            self.execution["limits"]["state_budget"] = known(budget);
        }
    }
}

/// Commands that refuse without reading any model normally have two fields.
/// Ledger failures additionally retain their diagnostics instead of hiding them.
#[derive(Debug, Serialize)]
pub struct Refusal {
    pub ledger: Ledger,
    pub uncertainty: crate::uncertainty::Uncertainty,
    pub verdict: crate::Verdict,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<crate::diag::Diagnostic>,
}
pub fn refusal(mode: &str, verdict: crate::Verdict) -> Refusal {
    let mut ledger = Ledger::aggregate(mode);
    for field in ["seed", "replications", "confidence", "random_method"] {
        ledger.execution[field] = na("no_analysis");
    }
    ledger.execution["bounds"] = unavailable("input_unavailable");
    for field in ["configuration", "scenario", "process"] {
        ledger.execution["selection"]["effective"][field] = unavailable("input_unavailable");
    }
    ledger.actor_versions = json!({"status":"unavailable","entries":[],"reason":"not_lowered"});
    let mut r = Refusal {
        ledger,
        uncertainty: crate::uncertainty::Uncertainty::unread(),
        verdict,
        diagnostics: Vec::new(),
    };
    let host = serde_json::to_value(&r).expect("refusal wire");
    complete(&mut r.ledger, &host, &mut r.verdict, &mut r.diagnostics);
    if !r.diagnostics.is_empty() {
        // The conditional field is now evidence too. Reconnect its subject;
        // complete preserves the single error and the independently bad verdict.
        let host = serde_json::to_value(&r).expect("refusal diagnostic wire");
        complete(&mut r.ledger, &host, &mut r.verdict, &mut r.diagnostics);
    }
    r
}

pub fn unsupported(mode: &str, message: &str) -> Refusal {
    let mut r = refusal(mode, crate::Verdict::Unsupported);
    r.diagnostics.push(crate::diag::Diagnostic {
        code: crate::diag::U_SEM_0.into(),
        severity: crate::diag::Severity::Error,
        message: message.into(),
        span: None,
    });
    let host = serde_json::to_value(&r).expect("unsupported refusal wire");
    complete(&mut r.ledger, &host, &mut r.verdict, &mut r.diagnostics);
    r
}
