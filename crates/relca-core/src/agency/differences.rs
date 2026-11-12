//! Closed semantic comparison, deliberately not a recursive report/JSON diff.
use super::*;
use serde_json::{Value, json};
mod degradation;
mod response;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DifferenceReason {
    AgencyUnavailable,
    ComparisonMismatch,
    DerivationLimit,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum DifferenceStatus {
    Derived { rows: Vec<MeaningRow> },
    NotDerived { reason: DifferenceReason },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, try_from = "DifferencesWire")]
pub struct Differences {
    pub rules: String,
    #[serde(flatten)]
    pub result: DifferenceStatus,
    pub unchecked: Vec<String>,
}
#[derive(Deserialize)]
struct DifferencesWire {
    rules: String,
    #[serde(flatten)]
    result: DifferenceStatus,
    unchecked: Vec<String>,
}
impl TryFrom<DifferencesWire> for Differences {
    type Error = String;
    fn try_from(w: DifferencesWire) -> std::result::Result<Self, String> {
        if w.rules != "agency-diff-p2-1" || w.unchecked != unchecked() {
            return Err("invalid Agency difference vocabulary".into());
        }
        if let DifferenceStatus::Derived { rows } = &w.result {
            let mut seen = BTreeSet::new();
            for row in rows {
                row.validate()?;
                let key =
                    serde_json::to_string(&(row.viewpoint.as_str(), row.item.as_str(), &row.key))
                        .map_err(|e| e.to_string())?;
                if !seen.insert(key) {
                    return Err("duplicate Agency meaning row".into());
                }
            }
        }
        Ok(Self {
            rules: w.rules,
            result: w.result,
            unchecked: w.unchecked,
        })
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MeaningStatus {
    Changed,
    NotComparable,
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MeaningKey {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub person: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capability: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effect: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub call: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject_sources: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<crate::responsibility::TargetDescription>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MeaningRow {
    pub viewpoint: String,
    pub item: String,
    #[serde(flatten)]
    pub key: MeaningKey,
    pub status: MeaningStatus,
    pub from: Value,
    pub to: Value,
}
impl MeaningRow {
    fn validate(&self) -> std::result::Result<(), String> {
        let table = TABLES
            .iter()
            .find(|t| t.viewpoint == self.viewpoint && t.items.contains(&self.item.as_str()))
            .ok_or("unknown Agency meaning item")?;
        let key = &self.key;
        let fields = [
            key.actor.is_some(),
            key.person.is_some(),
            key.end.is_some(),
            key.capability.is_some(),
            key.effect.is_some(),
            key.call.is_some(),
            key.subject_sources.is_some(),
            key.target.is_some(),
        ];
        let mut wanted = [false; 8];
        match table.key {
            "actor" => wanted[0] = true,
            "person" => wanted[1] = true,
            "end" => wanted[2] = true,
            "capability" => wanted[3] = true,
            "call" => wanted[5] = true,
            "effect" => {
                wanted[4] = true;
                wanted[5] = true;
            }
            "subject_sources" => wanted[6] = true,
            "responsibility" => {
                wanted[1] = true;
                wanted[7] = true;
            }
            _ => {}
        }
        // Keyless rows are only meaningful when an entire table was unavailable.
        if fields != wanted
            && !(fields.iter().all(|f| !f) && self.status == MeaningStatus::NotComparable)
        {
            return Err("wrong Agency meaning identity".into());
        }
        match self.status {
            MeaningStatus::Changed => {
                if self.from == self.to || (self.from.is_null() && self.to.is_null()) {
                    return Err("unchanged meaning row".into());
                }
                for value in [&self.from, &self.to] {
                    if !value.is_null() {
                        validate_value(&self.viewpoint, &self.item, value)?;
                    }
                }
            }
            MeaningStatus::NotComparable => {
                for (owner, value) in [("from", &self.from), ("to", &self.to)] {
                    let object = value.as_object().ok_or("status must be an object")?;
                    if object.len() != 2 || !object.contains_key("reason") {
                        return Err("invalid comparability status".into());
                    }
                    match value["status"].as_str() {
                        Some("derived") if value["reason"].is_null() => {}
                        Some("not_derived") if value["reason"].is_string() => {}
                        Some("unchecked")
                            if value["reason"].is_null() || value["reason"].is_string() => {}
                        _ => return Err("invalid comparability reason".into()),
                    }
                    if self.viewpoint == "degradation" {
                        degradation::validate_status(
                            value,
                            owner,
                            key.call.as_deref(),
                            &self.item,
                        )?;
                    }
                }
                if self.from["status"] == "derived" && self.to["status"] == "derived" {
                    return Err("known values must be compared".into());
                }
            }
        }
        Ok(())
    }
}
fn validate_value(view: &str, item: &str, value: &Value) -> std::result::Result<(), String> {
    fn typed<T: serde::de::DeserializeOwned>(v: &Value) -> std::result::Result<(), String> {
        serde_json::from_value::<T>(v.clone())
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
    match (view, item) {
        ("observation", _) | ("origin", _) => typed::<Vec<String>>(value),
        ("framing", "reachable") | ("retention", "contest_route" | "join_any_cancellation") => {
            typed::<bool>(value)
        }
        ("framing", "selectors") => typed::<Vec<viewpoints::Selector>>(value),
        ("framing", "read_only") => typed::<Vec<viewpoints::ReadOnly>>(value),
        ("framing", "actors") => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Actors {
                actors: Vec<String>,
                complete: bool,
            }
            let x: Actors = serde_json::from_value(value.clone()).map_err(|e| e.to_string())?;
            let _ = x.actors;
            if !x.complete {
                return Err("incomplete F cannot be compared".into());
            }
            Ok(())
        }
        ("retention", "normal") => typed::<viewpoints::Count>(value),
        ("degradation", "common_prefix") => {
            let counts: Vec<super::degradation::Count> =
                serde_json::from_value(value.clone()).map_err(|e| e.to_string())?;
            let mut names = BTreeSet::new();
            if counts.is_empty()
                || counts.iter().any(|c| {
                    c.capability.is_empty() || c.min > c.max || !names.insert(&c.capability)
                })
            {
                return Err("invalid degradation common prefix counts".into());
            }
            Ok(())
        }
        ("degradation", "normal" | "degraded") => {
            let side: super::degradation::Side =
                serde_json::from_value(value.clone()).map_err(|e| e.to_string())?;
            if side.status != "derived"
                || side.reason.is_some()
                || side.ends.is_none()
                || side.observation.is_none()
            {
                return Err("unknown degradation side cannot be compared as known".into());
            }
            Ok(())
        }
        ("retention", "fallback")
            if matches!(value.as_str(), Some("unchecked" | "kept_by_intent")) =>
        {
            Ok(())
        }
        ("effects", "e") => typed::<effects::Carrier>(value),
        ("effects", "j") => typed::<Vec<effects::Judge>>(value),
        ("effects", "u") => effects::decode_refusers(value.clone())
            .map(|_| ())
            .map_err(|e| e.to_string()),
        ("effects", "state_refusers") => typed::<Vec<effects::StateRefuser>>(value),
        ("effects", "concentration.generation_roots") => typed::<Vec<ViaCall>>(value),
        ("effects", _) => typed::<effects::Truth>(value),
        ("refusal", "judgements") => decode_judgements(value.clone())
            .map(|_| ())
            .map_err(|e| e.to_string()),
        ("lineage_groups", "independent_sources") => typed::<Vec<String>>(value),
        ("lineage_groups", _) => typed::<usize>(value),
        ("responsibility", "observation") => typed::<crate::responsibility::Observation>(value),
        ("responsibility", "refusal") => typed::<crate::responsibility::Refusal>(value),
        ("responsibility", "time_frame") => typed::<crate::responsibility::TimeFrame>(value),
        _ => Err("unknown Agency meaning value".into()),
    }
}
fn unchecked() -> Vec<String> {
    ["fallback_path", "workload_transfer", "via_model_paths"]
        .map(String::from)
        .to_vec()
}
impl Differences {
    pub fn unavailable(reason: DifferenceReason) -> Self {
        Self {
            rules: "agency-diff-p2-1".into(),
            result: DifferenceStatus::NotDerived { reason },
            unchecked: unchecked(),
        }
    }
    pub fn is_derived(&self) -> bool {
        matches!(self.result, DifferenceStatus::Derived { .. })
    }
}
pub struct ComparisonSide<'a> {
    pub agency: Option<&'a Agency>,
    pub responsibilities: Option<&'a [crate::responsibility::Responsibility]>,
}
pub fn differences(from: ComparisonSide<'_>, to: ComparisonSide<'_>) -> Differences {
    with_limits(from, to, DEFAULT)
}
pub(crate) fn differences_for_models(
    from: ComparisonSide<'_>,
    from_model: &ir::Model,
    to: ComparisonSide<'_>,
    to_model: &ir::Model,
) -> Differences {
    with_context(from, to, DEFAULT, Some((from_model, to_model)))
}
pub(super) fn with_limits(
    from: ComparisonSide<'_>,
    to: ComparisonSide<'_>,
    limits: Limits,
) -> Differences {
    with_context(from, to, limits, None)
}
fn with_context(
    from: ComparisonSide<'_>,
    to: ComparisonSide<'_>,
    limits: Limits,
    models: Option<(&ir::Model, &ir::Model)>,
) -> Differences {
    let (Some(a), Some(z)) = (from.agency, to.agency) else {
        return Differences::unavailable(DifferenceReason::AgencyUnavailable);
    };
    match compare(
        a,
        z,
        from.responsibilities,
        to.responsibilities,
        &mut viewpoints::budget(limits),
        models,
    ) {
        Ok(rows) => Differences {
            rules: "agency-diff-p2-1".into(),
            result: DifferenceStatus::Derived { rows },
            unchecked: unchecked(),
        },
        Err(_) => Differences::unavailable(DifferenceReason::DerivationLimit),
    }
}
struct Table {
    viewpoint: &'static str,
    collection: &'static str,
    key: &'static str,
    items: &'static [&'static str],
}
const TABLES: &[Table] = &[
    Table {
        viewpoint: "observation",
        collection: "sources",
        key: "actor",
        items: &[
            "normal.actors",
            "normal.people",
            "normal.unattributed_actors",
            "contest.actors",
            "contest.people",
            "contest.unattributed_actors",
            "contest.routes",
        ],
    },
    Table {
        viewpoint: "observation",
        collection: "people",
        key: "person",
        items: &["normal.direct", "normal.indirect", "contest.direct"],
    },
    Table {
        viewpoint: "origin",
        collection: "",
        key: "actor",
        items: &["direct", "indirect"],
    },
    Table {
        viewpoint: "framing",
        collection: "ends",
        key: "end",
        items: &["reachable", "selectors", "read_only"],
    },
    Table {
        viewpoint: "framing",
        collection: "",
        key: "",
        items: &["actors"],
    },
    Table {
        viewpoint: "retention",
        collection: "capabilities",
        key: "capability",
        items: &[
            "normal",
            "fallback",
            "contest_route",
            "join_any_cancellation",
        ],
    },
    Table {
        viewpoint: "effects",
        collection: "rows",
        key: "effect",
        items: &[
            "e",
            "j",
            "u",
            "state_refusers",
            "concentration.single_actor",
            "concentration.single_person",
            "concentration.single_generation_root",
            "concentration.generation_roots",
        ],
    },
    Table {
        viewpoint: "refusal",
        collection: "",
        key: "effect",
        items: &["judgements"],
    },
    Table {
        viewpoint: "lineage_groups",
        collection: "",
        key: "subject_sources",
        items: &["judgements", "actors", "independent_sources"],
    },
    Table {
        viewpoint: "responsibility",
        collection: "",
        key: "responsibility",
        items: &["observation", "refusal", "time_frame"],
    },
    Table {
        viewpoint: "degradation",
        collection: "routes",
        key: "call",
        items: &["normal", "degraded", "common_prefix"],
    },
];
#[derive(Clone)]
enum Cell {
    Known(Value),
    Unknown(String, Option<String>),
}
fn unknown(reason: &str) -> Cell {
    Cell::Unknown("not_derived".into(), Some(reason.into()))
}
fn status(cell: &Cell) -> Value {
    match cell {
        Cell::Known(_) => json!({"status":"derived","reason":null}),
        Cell::Unknown(s, r) => json!({"status":s,"reason":r}),
    }
}
fn path<'a>(mut v: &'a Value, name: &str) -> &'a Value {
    for p in name.split('.') {
        v = &v[p];
    }
    v
}
struct Projection {
    rows: Vec<(MeaningKey, Value)>,
    /// This side's keys in declaration order: the rows' keys, plus (for
    /// `refusal`) the effects whose legacy row this side did not derive.
    keys: Vec<MeaningKey>,
    unknown: Option<String>,
    /// Keys of this side whose row was not derived (U13), with the reason.
    skipped: Vec<MeaningKey>,
    /// A key absent from this side is not a derived absence (U13).
    absent: Option<String>,
}
/// The legacy `refusal` rows and `lineage_groups` skip every effect whose path
/// has a state guard or that is atomic, recording only `state_guards` in
/// `agency.unchecked` (agency.rs). Read the gap from the same side's effects
/// block, which has a row for every effect call (ADR 0043 U13, review M1).
struct LegacyGaps {
    response: bool,
    /// Effect keys in call order, when the effects block was derived.
    effects: Option<Vec<MeaningKey>>,
    skipped: Vec<MeaningKey>,
    /// `lineage_groups` on this side omits the skipped effects' judgements.
    incomplete: bool,
}
fn effect_key(row: &Value) -> Result<MeaningKey> {
    Ok(MeaningKey {
        effect: Some(row["effect"].as_str().ok_or_else(invalid)?.into()),
        call: Some(row["call"].as_str().ok_or_else(invalid)?.into()),
        ..MeaningKey::default()
    })
}
fn legacy_gaps(a: &Value, b: &mut Budget) -> Result<LegacyGaps> {
    let response = a["unchecked"]
        .as_array()
        .is_some_and(|xs| xs.iter().any(|w| w == "response_control"));
    let guards = a["unchecked"]
        .as_array()
        .ok_or_else(invalid)?
        .iter()
        .any(|w| w == "state_guards");
    let refused = a["refusal"]
        .as_array()
        .ok_or_else(invalid)?
        .iter()
        .map(|r| {
            b.copy(2)?;
            effect_key(r)
        })
        .collect::<Result<Vec<_>>>()?;
    let effects = &a["viewpoints"]["effects"];
    if effects["status"] != "derived" {
        // Without the effects rows the skipped effects cannot be named; any
        // state guard leaves both legacy tables possibly incomplete.
        return Ok(LegacyGaps {
            response,
            effects: None,
            skipped: Vec::new(),
            incomplete: guards,
        });
    }
    let mut keys = Vec::new();
    let mut skipped = Vec::new();
    for row in effects["rows"].as_array().ok_or_else(invalid)? {
        b.work()?;
        b.copy(2)?;
        let key = effect_key(row)?;
        // A Rule-only omitted legacy row is a known absence only if the
        // effects derivation independently established process J/U as known.
        let rule_empty = a["rules"] == "agency-p1-rule-1"
            && row["process_judgements"] == "derived"
            && row["state_refusers"].as_array().is_some_and(Vec::is_empty);
        if !refused.contains(&key) && !rule_empty {
            skipped.push(key.clone());
        }
        keys.push(key);
    }
    if refused.iter().any(|k| !keys.contains(k)) || !skipped.is_empty() && !guards && !response {
        return Err(invalid());
    }
    Ok(LegacyGaps {
        response,
        effects: Some(keys),
        // Response clears the legacy refusal table, so `skipped` no longer
        // identifies missing J. Its lineage reconstruction still starts from
        // legacy all_j; retain the producer's state-guard incompleteness.
        incomplete: if response {
            guards
        } else {
            !skipped.is_empty()
        },
        skipped,
    })
}
fn projection(
    a: &Value,
    duties: &Value,
    t: &Table,
    gaps: &LegacyGaps,
    b: &mut Budget,
) -> Result<Projection> {
    b.work()?;
    let block = if t.viewpoint == "responsibility" {
        duties
    } else if matches!(t.viewpoint, "origin" | "refusal" | "lineage_groups") {
        &a[t.viewpoint]
    } else {
        &a["viewpoints"][t.viewpoint]
    };
    if block["status"] == "not_derived" || block.is_null() && t.viewpoint != "responsibility" {
        return Ok(Projection {
            rows: Vec::new(),
            keys: Vec::new(),
            unknown: Some(
                block["reason"]
                    .as_str()
                    .unwrap_or("derivation_limit")
                    .into(),
            ),
            skipped: Vec::new(),
            absent: None,
        });
    }
    let mut rows = Vec::new();
    if t.key.is_empty() {
        b.copy(1)?;
        rows.push((MeaningKey::default(), block.clone()));
    } else {
        let list = if t.collection.is_empty() {
            block
        } else {
            &block[t.collection]
        };
        if let Some(list) = list.as_array() {
            for row in list {
                b.copy(2)?;
                let mut key = MeaningKey::default();
                let text = |name: &str| row[name].as_str().map(String::from).ok_or_else(invalid);
                match t.key {
                    "actor" => key.actor = Some(text("actor")?),
                    "person" => key.person = Some(text("person")?),
                    "end" => key.end = Some(text("end")?),
                    "capability" => key.capability = Some(text("capability")?),
                    "effect" => {
                        key.effect = Some(text("effect")?);
                        key.call = Some(text("call")?);
                    }
                    "subject_sources" => {
                        key.subject_sources = Some(
                            serde_json::from_value(row["subject_sources"].clone())
                                .map_err(|_| invalid())?,
                        )
                    }
                    "responsibility" => {
                        key.person = Some(text("person")?);
                        key.target = Some(
                            serde_json::from_value(row["target"].clone()).map_err(|_| invalid())?,
                        );
                    }
                    _ => return Err(invalid()),
                }
                rows.push((key, row.clone()));
            }
        }
    }
    let mut keys: Vec<MeaningKey> = rows.iter().map(|(k, _)| k.clone()).collect();
    let mut skipped = Vec::new();
    let mut unknown = None;
    let mut absent = (t.viewpoint == "refusal" && gaps.response).then(|| "response_control".into());
    match t.viewpoint {
        "refusal" => match &gaps.effects {
            Some(effects) => {
                b.copy(effects.len() + gaps.skipped.len())?;
                keys = effects.clone();
                skipped = gaps.skipped.clone();
            }
            None if gaps.incomplete => absent = Some("state_guards".to_string()),
            None => {}
        },
        "lineage_groups" if gaps.incomplete => unknown = Some("state_guards".to_string()),
        _ => {}
    }
    Ok(Projection {
        rows,
        keys,
        unknown,
        skipped,
        absent,
    })
}
fn cell(p: &Projection, key: &MeaningKey, t: &Table, item: &str) -> Cell {
    if let Some(reason) = &p.unknown {
        return unknown(reason);
    }
    let Some((_, row)) = p.rows.iter().find(|(k, _)| k == key) else {
        if p.skipped.contains(key) {
            return unknown(p.absent.as_deref().unwrap_or("state_guards"));
        }
        if let Some(reason) = &p.absent {
            return unknown(reason);
        }
        return Cell::Known(Value::Null);
    };
    if row["status"] == "not_derived" {
        return unknown(row["reason"].as_str().unwrap_or("derivation_limit"));
    }
    if t.viewpoint == "framing" && item == "actors" {
        return if row["complete"] == false {
            unknown(
                row["ends"]
                    .as_array()
                    .and_then(|ends| ends.iter().find(|e| e["status"] == "not_derived"))
                    .and_then(|e| e["reason"].as_str())
                    .unwrap_or("state_guards"),
            )
        } else {
            Cell::Known(json!({"actors":row["actors"],"complete":true}))
        };
    }
    if t.viewpoint == "effects"
        && matches!(item, "j" | "u")
        && row["process_judgements"]["status"] == "not_derived"
    {
        return unknown(
            row["process_judgements"]["reason"]
                .as_str()
                .unwrap_or("state_guards"),
        );
    }
    let value = path(row, item);
    if value["status"] == "not_derived"
        || t.viewpoint == "responsibility" && value["status"] == "unchecked"
    {
        return Cell::Unknown(
            value["status"].as_str().unwrap().into(),
            value["reason"].as_str().map(String::from),
        );
    }
    if t.viewpoint == "effects"
        && item.starts_with("concentration.")
        && value == "unknown"
        && row["process_judgements"]["status"] == "not_derived"
    {
        return unknown(
            row["process_judgements"]["reason"]
                .as_str()
                .unwrap_or("state_guards"),
        );
    }
    Cell::Known(value.clone())
}
fn compare(
    a: &Agency,
    z: &Agency,
    ad: Option<&[crate::responsibility::Responsibility]>,
    zd: Option<&[crate::responsibility::Responsibility]>,
    b: &mut Budget,
    models: Option<(&ir::Model, &ir::Model)>,
) -> Result<Vec<MeaningRow>> {
    viewpoints::depth(b, 4)?;
    // Reserve the bounded semantic serialization before constructing it. This
    // ledger is private to differences, not effects, legacy or execution bounds.
    fn size<T: Serialize>(x: &T, b: &mut Budget) -> Result<Value> {
        // Serializing into a counting sink avoids allocating an unbudgeted copy.
        struct Counter(usize);
        impl std::io::Write for Counter {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                self.0 = self
                    .0
                    .checked_add(bytes.len())
                    .ok_or_else(|| std::io::Error::other("size overflow"))?;
                Ok(bytes.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let mut count = Counter(0);
        serde_json::to_writer(&mut count, x).map_err(|_| invalid())?;
        b.copy(count.0)?;
        serde_json::to_value(x).map_err(|_| invalid())
    }
    let mut a = size(a, b)?;
    let mut z = size(z, b)?;
    // Work only on the already budgeted comparison copies. Child reports and
    // raw metrics, trace labels and sampler keys retain their own IR namespace.
    if let Some((from, to)) = models {
        response::normalize(&mut a, from, b)?;
        response::normalize(&mut z, to, b)?;
    }
    let ad = size(&ad, b)?;
    let zd = size(&zd, b)?;
    let mut out = Vec::new();
    let a_gaps = legacy_gaps(&a, b)?;
    let z_gaps = legacy_gaps(&z, b)?;
    for t in TABLES {
        if t.viewpoint == "degradation" {
            degradation::append(&a, &z, &mut out, b)?;
            continue;
        }
        let a = projection(&a, &ad, t, &a_gaps, b)?;
        let z = projection(&z, &zd, t, &z_gaps, b)?;
        let mut keys = Vec::new();
        for key in a.keys.iter().chain(&z.keys) {
            b.work()?;
            if !keys.contains(key) {
                b.copy(1)?;
                keys.push(key.clone());
            }
        }
        if keys.is_empty() && (a.unknown.is_some() || z.unknown.is_some()) {
            b.copy(1)?;
            keys.push(MeaningKey::default());
        }
        for item in t.items {
            for key in &keys {
                b.work()?;
                let x = cell(&a, key, t, item);
                let y = cell(&z, key, t, item);
                let (status, from, to) = match (&x, &y) {
                    (Cell::Known(x), Cell::Known(y)) if x == y => continue,
                    (Cell::Known(x), Cell::Known(y)) => {
                        (MeaningStatus::Changed, x.clone(), y.clone())
                    }
                    _ => (MeaningStatus::NotComparable, status(&x), status(&y)),
                };
                b.copy(5)?;
                let row = MeaningRow {
                    viewpoint: t.viewpoint.into(),
                    item: (*item).into(),
                    key: key.clone(),
                    status,
                    from,
                    to,
                };
                row.validate().map_err(|_| invalid())?;
                out.push(row);
            }
        }
    }
    Ok(out)
}
