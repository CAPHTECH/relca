//! Stage correspondence is unchecked. Exclude the new output before both the
//! comparison's counting serialization and allocation, preserving its budget.
use super::*;
use serde::ser::{SerializeSeq, SerializeStruct};

pub(super) struct AgencyView<'a>(pub &'a Agency);
pub(super) struct DutiesView<'a>(pub Option<&'a [crate::responsibility::Responsibility]>);
struct ViewpointsView<'a>(&'a Viewpoints);
struct ObservationView<'a>(&'a viewpoints::Observation);
struct DutyView<'a>(&'a crate::responsibility::Responsibility);
struct DutyObservationView<'a>(&'a crate::responsibility::Observation);
struct Filtered<'a>(&'a [String], &'a [&'a str]);

impl Serialize for Filtered<'_> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        let mut out = s.serialize_seq(None)?;
        for word in self.0 {
            if !self.1.contains(&word.as_str()) {
                out.serialize_element(word)?;
            }
        }
        out.end()
    }
}
impl Serialize for AgencyView<'_> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        let a = self.0;
        let mut out = s.serialize_struct("Agency", 11)?;
        if let Some(v) = &a.degradation {
            out.serialize_field("degradation", v)?;
        }
        out.serialize_field("rules", &a.rules)?;
        out.serialize_field("origin", &a.origin)?;
        out.serialize_field("lineage", &a.lineage)?;
        out.serialize_field("lineage_groups", &a.lineage_groups)?;
        out.serialize_field("refusal", &a.refusal)?;
        out.serialize_field(
            "unchecked",
            &Filtered(
                &a.unchecked,
                &[
                    "stage_observation(fallback_path)",
                    "stage_observation(rule)",
                    "responsibility_stage(contest_path)",
                ],
            ),
        )?;
        if let Some(v) = &a.common_causes {
            out.serialize_field("common_causes", v)?;
        }
        if let Some(v) = &a.model_uncertainties {
            out.serialize_field("model_uncertainties", v)?;
        }
        if let Some(v) = &a.source_descriptions {
            out.serialize_field("source_descriptions", v)?;
        }
        if let Some(v) = &a.viewpoints {
            out.serialize_field("viewpoints", &ViewpointsView(v))?;
        }
        out.end()
    }
}
impl Serialize for ViewpointsView<'_> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        let v = self.0;
        let mut out = s.serialize_struct("Viewpoints", 7)?;
        out.serialize_field("rules", &v.rules)?;
        out.serialize_field("scope", &v.scope)?;
        out.serialize_field("observation", &ObservationView(&v.observation))?;
        out.serialize_field("framing", &v.framing)?;
        out.serialize_field("proposal", &v.proposal)?;
        out.serialize_field("retention", &v.retention)?;
        if let Some(v) = &v.effects {
            out.serialize_field("effects", v)?;
        }
        out.end()
    }
}
impl Serialize for ObservationView<'_> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        match self.0 {
            viewpoints::Observation::Derived {
                sources,
                people,
                fallback,
                ..
            } => {
                let mut out = s.serialize_struct("Observation", 4)?;
                out.serialize_field("status", "derived")?;
                out.serialize_field("sources", sources)?;
                out.serialize_field("people", people)?;
                out.serialize_field("fallback", fallback)?;
                out.end()
            }
            v => v.serialize(s),
        }
    }
}
impl Serialize for DutiesView<'_> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        let Some(rows) = self.0 else {
            return s.serialize_none();
        };
        let mut out = s.serialize_seq(Some(rows.len()))?;
        for row in rows {
            out.serialize_element(&DutyView(row))?;
        }
        out.end()
    }
}
impl Serialize for DutyView<'_> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        let r = self.0;
        let mut out = s.serialize_struct("Responsibility", 9)?;
        out.serialize_field("id", &r.id)?;
        out.serialize_field("rules", &r.rules)?;
        out.serialize_field("person", &r.person)?;
        out.serialize_field("target", &r.target)?;
        out.serialize_field("actors", &r.actors)?;
        out.serialize_field("observation", &DutyObservationView(&r.observation))?;
        out.serialize_field("refusal", &r.refusal)?;
        out.serialize_field("time_frame", &r.time_frame)?;
        out.serialize_field("warnings", &Filtered(&r.warnings, &[diag::W_RESP_3]))?;
        out.end()
    }
}
impl Serialize for DutyObservationView<'_> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        let o = self.0;
        let mut out = s.serialize_struct("Observation", 6)?;
        out.serialize_field("status", &o.status)?;
        out.serialize_field("required_sources", &o.required_sources)?;
        out.serialize_field("direct", &o.direct)?;
        out.serialize_field("indirect", &o.indirect)?;
        out.serialize_field("missing_sources", &o.missing_sources)?;
        out.serialize_field("reason", &o.reason)?;
        out.end()
    }
}
