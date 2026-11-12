//! I01: closed manifest shape and finite, explicit scope. No filesystem or RNG.
use crate::{Verdict, diag, probability::Rational};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const MAX_MANIFEST_BYTES: usize = 1 << 20;
pub const MAX_POINTS: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Exhaustive,
    Simulation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureKind {
    ManifestInvalid,
    SchemaUnsupported,
    TargetInvalid,
    CapacityLimit,
    InputUnavailable,
    FeatureUnsupported,
    ContractMismatch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputError {
    pub kind: FailureKind,
    pub detail: String,
    pub point: Option<String>,
}

impl InputError {
    pub fn new(kind: FailureKind, detail: impl Into<String>) -> Self {
        Self {
            kind,
            detail: detail.into(),
            point: None,
        }
    }
    pub fn at_point(mut self, id: &str) -> Self {
        self.point = Some(id.into());
        self
    }
    pub fn reason(&self) -> &'static str {
        match self.kind {
            FailureKind::ManifestInvalid => "sensitivity_manifest_invalid",
            FailureKind::SchemaUnsupported => "sensitivity_schema_unsupported",
            FailureKind::TargetInvalid => "sensitivity_target_invalid",
            FailureKind::CapacityLimit => "sensitivity_capacity_limit",
            FailureKind::InputUnavailable => "sensitivity_input_unavailable",
            FailureKind::FeatureUnsupported => "sensitivity_feature_combination",
            FailureKind::ContractMismatch => "sensitivity_contract_mismatch",
        }
    }
    pub fn verdict(&self) -> Verdict {
        match self.kind {
            FailureKind::SchemaUnsupported
            | FailureKind::CapacityLimit
            | FailureKind::FeatureUnsupported => Verdict::Unsupported,
            _ => Verdict::Invalid,
        }
    }
    pub fn diagnostic(&self) -> diag::Diagnostic {
        let code = match self.kind {
            FailureKind::ManifestInvalid => diag::E_SYN_0,
            FailureKind::SchemaUnsupported => diag::U_SYN_0,
            FailureKind::TargetInvalid => diag::E_NAME_0,
            FailureKind::CapacityLimit => diag::U_SEM_0,
            FailureKind::InputUnavailable => diag::E_CON_0,
            FailureKind::FeatureUnsupported => diag::U_SEM_0,
            FailureKind::ContractMismatch => diag::E_CMP_0,
        };
        let context = self
            .point
            .as_ref()
            .map(|id| format!(" point {id:?}:"))
            .unwrap_or_default();
        diag::Diagnostic {
            code: code.into(),
            severity: diag::Severity::Error,
            message: format!("{}:{context} {}", self.reason(), self.detail),
            // Manifest/point context is not an offset in an unread model.
            span: None,
        }
    }
}
impl std::fmt::Display for InputError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.diagnostic().message)
    }
}
impl std::error::Error for InputError {}

fn nullable<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<Option<T>, D::Error> {
    Option::deserialize(d)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema: String,
    pub scope: Scope,
    pub points: Vec<Point>,
    pub targets: Vec<Target>,
    #[serde(deserialize_with = "nullable")]
    pub confidence: Option<Rational>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scope {
    pub kind: String,
    pub rationale: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Point {
    pub id: String,
    pub model: String,
    #[serde(deserialize_with = "nullable")]
    pub configuration: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub scenario: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Target {
    pub kind: String,
    pub id: String,
}

impl Manifest {
    /// Separate decoding from validation so a runner can retain all declared
    /// point IDs after a later refusal. Neither operation executes a prefix.
    pub fn decode(bytes: &[u8]) -> Result<Self, InputError> {
        if bytes.len() > MAX_MANIFEST_BYTES {
            return Err(InputError::new(
                FailureKind::CapacityLimit,
                "manifest exceeds 1MiB",
            ));
        }
        serde_json::from_slice(bytes)
            .map_err(|e| InputError::new(FailureKind::ManifestInvalid, e.to_string()))
    }

    /// Must run on the entire manifest before reading models. The source loader
    /// and same-question gate are additional mandatory pre-execution checks.
    pub fn validate(&self, mode: Mode) -> Result<(), InputError> {
        use FailureKind::*;
        let invalid = |detail| InputError::new(ManifestInvalid, detail);
        if self.schema != "relca-sensitivity/v1" || self.scope.kind != "enumerated_models" {
            return Err(InputError::new(
                SchemaUnsupported,
                "only relca-sensitivity/v1 enumerated_models is supported",
            ));
        }
        if self.scope.rationale.trim().is_empty()
            || self.points.is_empty()
            || self.targets.is_empty()
        {
            return Err(invalid(
                "nonempty rationale, points and targets are required",
            ));
        }
        if self.points.len() > MAX_POINTS {
            return Err(InputError::new(
                CapacityLimit,
                "more than 256 declared points",
            ));
        }
        let confidence_ok = match (mode, &self.confidence) {
            (Mode::Exhaustive, None) => true,
            (Mode::Simulation, Some(c)) => c > &Rational::zero() && c < &Rational::one(),
            _ => false,
        };
        if !confidence_ok {
            return Err(invalid(
                "confidence must be null for exhaustive, or strictly between 0 and 1 for simulation",
            ));
        }
        let mut points = BTreeSet::new();
        for p in &self.points {
            if p.id.trim().is_empty()
                || p.model.is_empty()
                || [&p.configuration, &p.scenario]
                    .iter()
                    .any(|s| s.as_ref().is_some_and(|s| s.trim().is_empty()))
            {
                return Err(invalid("empty point ID, path or selection").at_point(&p.id));
            }
            if !points.insert(&p.id) {
                return Err(InputError::new(TargetInvalid, "duplicate point ID").at_point(&p.id));
            }
        }
        let mut targets = BTreeSet::new();
        for t in &self.targets {
            if t.kind != "probability_query" {
                return Err(InputError::new(
                    SchemaUnsupported,
                    "only probability_query targets are supported",
                ));
            }
            if t.id.trim().is_empty() || !targets.insert(&t.id) {
                return Err(InputError::new(
                    TargetInvalid,
                    "empty or duplicate target ID",
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn control() -> Value {
        serde_json::from_str(include_str!("../../tests/data/sensitivity/values.json")).unwrap()
    }
    fn decode(v: &Value) -> Result<Manifest, InputError> {
        Manifest::decode(&serde_json::to_vec(v).unwrap())
    }

    #[test]
    fn handwritten_points_are_preserved_without_implicit_product() {
        let m = decode(&control()).unwrap();
        m.validate(Mode::Exhaustive).unwrap();
        assert_eq!(
            m.points.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(),
            ["base", "high", "miss"]
        );
        assert_eq!(m.targets[0].id, "probability reached #1");
        assert_eq!(decode(&json!(m)).unwrap(), m);
    }
    #[test]
    fn every_object_is_closed_and_nullable_selections_are_mandatory() {
        for parent in ["", "/scope", "/points/0", "/targets/0"] {
            let mut bad = control();
            bad.pointer_mut(parent)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert("extra".into(), json!(true));
            assert_eq!(decode(&bad).unwrap_err().kind, FailureKind::ManifestInvalid);
        }
        for (parent, key) in [
            ("", "confidence"),
            ("/points/0", "configuration"),
            ("/points/0", "scenario"),
        ] {
            let mut bad = control();
            bad.pointer_mut(parent)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .remove(key);
            assert!(decode(&bad).is_err(), "accepted missing {key}");
        }
    }
    #[test]
    fn duplicate_json_keys_are_not_collapsed_to_last_wins() {
        let raw = serde_json::to_string(&control()).unwrap();
        for token in [
            "\"schema\":\"relca-sensitivity/v1\"",
            "\"kind\":\"enumerated_models\"",
            "\"id\":\"base\"",
            "\"id\":\"probability reached #1\"",
        ] {
            assert_eq!(raw.matches(token).count(), 1);
            let bad = raw.replacen(token, &format!("{token},{token}"), 1);
            assert_eq!(
                Manifest::decode(bad.as_bytes()).unwrap_err().kind,
                FailureKind::ManifestInvalid
            );
        }
    }
    #[test]
    fn capacity_is_inclusive_and_never_a_point_prefix() {
        let mut m = decode(&control()).unwrap();
        let point = m.points[0].clone();
        m.points = (0..MAX_POINTS)
            .map(|i| Point {
                id: format!("p{i}"),
                ..point.clone()
            })
            .collect();
        m.validate(Mode::Exhaustive).unwrap();
        m.points.push(Point {
            id: "overflow".into(),
            ..point
        });
        assert_eq!(
            m.validate(Mode::Exhaustive).unwrap_err().kind,
            FailureKind::CapacityLimit
        );
        assert_eq!(m.points.len(), 257);
        let mut bytes = serde_json::to_vec(&control()).unwrap();
        bytes.resize(MAX_MANIFEST_BYTES, b' ');
        Manifest::decode(&bytes)
            .unwrap()
            .validate(Mode::Exhaustive)
            .unwrap();
        bytes.push(b' ');
        assert_eq!(
            Manifest::decode(&bytes).unwrap_err().kind,
            FailureKind::CapacityLimit
        );
    }
    #[test]
    fn mode_confidence_and_explicit_scope_fail_closed() {
        let mut v = control();
        assert!(decode(&v).unwrap().validate(Mode::Simulation).is_err());
        for c in ["0", "1", "7/8"] {
            v["confidence"] = json!(c);
            let m = decode(&v).unwrap();
            assert_eq!(m.validate(Mode::Simulation).is_ok(), c == "7/8");
            assert!(m.validate(Mode::Exhaustive).is_err());
        }
        v = control();
        v["schema"] = json!("relca-sensitivity/v2");
        assert_eq!(
            decode(&v)
                .unwrap()
                .validate(Mode::Exhaustive)
                .unwrap_err()
                .kind,
            FailureKind::SchemaUnsupported
        );
        v = control();
        v["scope"]["kind"] = json!("cartesian_product");
        assert_eq!(
            decode(&v)
                .unwrap()
                .validate(Mode::Exhaustive)
                .unwrap_err()
                .kind,
            FailureKind::SchemaUnsupported
        );
        v = control();
        v["points"][1]["id"] = v["points"][0]["id"].clone();
        assert_eq!(
            decode(&v)
                .unwrap()
                .validate(Mode::Exhaustive)
                .unwrap_err()
                .kind,
            FailureKind::TargetInvalid
        );
    }
    #[test]
    fn diagnostics_have_registered_codes_and_no_fabricated_model_offsets() {
        for (kind, code, verdict) in [
            (FailureKind::ManifestInvalid, "E-SYN-0", Verdict::Invalid),
            (
                FailureKind::SchemaUnsupported,
                "U-SYN-0",
                Verdict::Unsupported,
            ),
            (FailureKind::TargetInvalid, "E-NAME-0", Verdict::Invalid),
            (FailureKind::CapacityLimit, "U-SEM-0", Verdict::Unsupported),
            (FailureKind::InputUnavailable, "E-CON-0", Verdict::Invalid),
            (
                FailureKind::FeatureUnsupported,
                "U-SEM-0",
                Verdict::Unsupported,
            ),
            (FailureKind::ContractMismatch, "E-CMP-0", Verdict::Invalid),
        ] {
            let e = InputError::new(kind, "hand failure").at_point("p");
            let d = e.diagnostic();
            assert_eq!(e.verdict(), verdict);
            assert_eq!(d.code, code);
            assert_eq!(diag::severity_of(code), Some(d.severity));
            assert!(d.message.starts_with(e.reason()));
            assert_eq!(d.span, None);
        }
    }
}
