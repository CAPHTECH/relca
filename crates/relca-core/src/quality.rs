//! Quality is an analyst observation, never a verification request or state.
//! Endpoint inputs retain both unknown intervals until aggregation is complete.
use crate::{ir, probability::Rational};
use num_rational::BigRational;
use num_traits::Zero;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
mod quantile;
pub(crate) use quantile::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Quality {
    pub rules: String,
    pub analysis: String,
    #[serde(serialize_with = "ordered_rows")]
    pub properties: Vec<Value>,
    #[serde(serialize_with = "ordered_rows")]
    pub cohort: Vec<Value>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "ordered_optional_rows"
    )]
    pub quantiles: Option<Vec<Value>>,
    #[serde(serialize_with = "ordered_rows")]
    pub outcomes_per_arrival: Vec<Value>,
    pub checked: Vec<String>,
    pub unchecked: Vec<String>,
}

impl Quality {
    /// Standalone Quality decoding checks shape, not the surrounding Cut. The
    /// exhaustive Report decoder supplies that context for deferred prior nulls.
    pub(crate) fn validate_report_defer(
        &self,
        declared_and_checked: bool,
    ) -> Result<(), &'static str> {
        if !declared_and_checked
            && self.analysis == "exhaustive"
            && self.properties.iter().any(|p| {
                p["cases"].as_array().is_some_and(|cases| {
                    cases.iter().any(|c| {
                        c["min"].is_null()
                            && matches!(c["reason"].as_str(), Some("truncated" | "undetermined"))
                    })
                })
            })
        {
            return Err("quality prior-reason null requires declared and checked defer");
        }
        Ok(())
    }

    /// ADR 0053 output projection shared by independent numeric collectors.
    /// Sample case hits and unknown counts remain structural observations.
    pub(crate) fn apply_deferred(&mut self, m: &ir::Model) {
        if !m.deferred_metrics().is_some_and(|d| d.process) {
            return;
        }
        fn withdraw(v: &mut Value) {
            let o = v.as_object_mut().expect("quality quantity");
            for k in ["min", "max", "mean", "lower", "upper", "basis", "point"] {
                if let Some(x) = o.get_mut(k) {
                    *x = Value::Null;
                }
            }
            if o.contains_key("mean") {
                o.insert("samples".into(), json!(0));
            }
            for k in ["numerator", "denominator"] {
                if let Some(x) = o.get_mut(k) {
                    withdraw(x);
                }
            }
            if o.get("reason").is_none_or(Value::is_null) {
                o.insert("reason".into(), json!("not_configured"));
            }
        }
        for p in &mut self.properties {
            for case in p["cases"].as_array_mut().expect("quality cases") {
                if case.get("reason").is_some() {
                    withdraw(case);
                }
            }
            withdraw(&mut p["expected_count"]);
        }
        for c in &mut self.cohort {
            withdraw(&mut c["value"]);
            c["bounds_with_undetermined"] = Value::Null;
        }
        for q in self.quantiles.iter_mut().flatten() {
            withdraw(&mut q["value"]);
            q["lower_bound"] = Value::Null;
        }
        for o in &mut self.outcomes_per_arrival {
            withdraw(&mut o["expected_count"]);
            withdraw(&mut o["rate"]);
        }
    }
}

/// Preserve the declared presentation order without allowing the JSON builders
/// to silently drop an unknown field. Validation remains separate from display.
pub(crate) fn ordered_rows<S: serde::Serializer>(rows: &[Value], s: S) -> Result<S::Ok, S::Error> {
    use serde::ser::SerializeSeq;
    let mut seq = s.serialize_seq(Some(rows.len()))?;
    for row in rows {
        seq.serialize_element(&Ordered(row))?;
    }
    seq.end()
}
pub(crate) fn ordered_optional_rows<S: serde::Serializer>(
    rows: &Option<Vec<Value>>,
    s: S,
) -> Result<S::Ok, S::Error> {
    match rows {
        Some(rows) => ordered_rows(rows, s),
        None => s.serialize_none(),
    }
}
struct Ordered<'a>(&'a Value);
impl Serialize for Ordered<'_> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::{SerializeMap, SerializeSeq};
        match self.0 {
            Value::Array(xs) => {
                let mut seq = s.serialize_seq(Some(xs.len()))?;
                for x in xs {
                    seq.serialize_element(&Ordered(x))?;
                }
                seq.end()
            }
            Value::Object(o) => {
                let fields: &[&str] = if o.contains_key("key") {
                    if o.contains_key("values") {
                        &["key", "denominator", "values"]
                    } else {
                        &["key", "denominator", "from", "to"]
                    }
                } else if o.contains_key("scope") {
                    &["scope", "subject", "end", "field"]
                } else if o.contains_key("status") {
                    &["configuration", "status", "value"]
                } else if o.contains_key("p") && o.contains_key("property") {
                    &["name", "p", "property", "form", "value", "lower_bound"]
                } else if o.contains_key("form") {
                    &[
                        "name",
                        "form",
                        "numerator",
                        "denominator",
                        "value",
                        "bounds_with_undetermined",
                    ]
                } else if o.contains_key("cases") {
                    &[
                        "name",
                        "condition",
                        "within",
                        "cases",
                        "expected_count",
                        "unknown_count",
                    ]
                } else if o.contains_key("case_id") {
                    &[
                        "case_id", "arrival", "min", "max", "hits", "unknown", "trials", "reason",
                    ]
                } else if o.contains_key("expected_count") {
                    &["kind", "end", "expected_count", "rate"]
                } else if o.contains_key("kind") {
                    &["kind", "count", "property", "covers", "reason"]
                } else if o.contains_key("censored") {
                    &["point", "observations", "censored", "reason"]
                } else if o.contains_key("point") {
                    &["point", "numerator", "denominator", "samples", "reason"]
                } else if o.contains_key("mean") {
                    &["mean", "min", "max", "samples", "reason"]
                } else {
                    &["lower", "upper", "basis", "reason"]
                };
                let mut map = s.serialize_map(Some(o.len()))?;
                for key in fields {
                    if let Some(value) = o.get(*key) {
                        map.serialize_entry(key, &Ordered(value))?;
                    }
                }
                for (key, value) in o {
                    if !fields.contains(&key.as_str()) {
                        map.serialize_entry(key, &Ordered(value))?;
                    }
                }
                map.end()
            }
            v => v.serialize(s),
        }
    }
}

impl<'de> Deserialize<'de> for Quality {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Raw {
            rules: String,
            analysis: String,
            #[serde(deserialize_with = "strict_rows")]
            properties: Vec<Value>,
            #[serde(deserialize_with = "strict_rows")]
            cohort: Vec<Value>,
            #[serde(default, deserialize_with = "strict_quantiles")]
            quantiles: Option<Vec<Value>>,
            #[serde(deserialize_with = "strict_rows")]
            outcomes_per_arrival: Vec<Value>,
            checked: Vec<String>,
            unchecked: Vec<String>,
        }
        let r = Raw::deserialize(d)?;
        let q = Quality {
            rules: r.rules,
            analysis: r.analysis,
            properties: r.properties,
            cohort: r.cohort,
            quantiles: r.quantiles,
            outcomes_per_arrival: r.outcomes_per_arrival,
            checked: r.checked,
            unchecked: r.unchecked,
        };
        validate_wire(&q).map_err(serde::de::Error::custom)?;
        Ok(q)
    }
}

// serde_json::Value alone would discard duplicate object keys before the
// closed-shape validator sees them. Retain that boundary during decoding.
fn strict_rows<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<Value>, D::Error> {
    Vec::<StrictValue>::deserialize(d).map(|xs| xs.into_iter().map(|x| x.0).collect())
}
fn strict_quantiles<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Option<Vec<Value>>, D::Error> {
    strict_rows(d).map(Some)
}
struct StrictValue(Value);
impl<'de> Deserialize<'de> for StrictValue {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = StrictValue;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a closed quality JSON value without duplicate keys or floating point")
            }
            fn visit_bool<E: serde::de::Error>(self, v: bool) -> Result<Self::Value, E> {
                Ok(StrictValue(json!(v)))
            }
            fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Self::Value, E> {
                Ok(StrictValue(json!(v)))
            }
            fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Self::Value, E> {
                Ok(StrictValue(json!(v)))
            }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
                Ok(StrictValue(json!(v)))
            }
            fn visit_string<E: serde::de::Error>(self, v: String) -> Result<Self::Value, E> {
                Ok(StrictValue(json!(v)))
            }
            fn visit_unit<E: serde::de::Error>(self) -> Result<Self::Value, E> {
                Ok(StrictValue(Value::Null))
            }
            fn visit_none<E: serde::de::Error>(self) -> Result<Self::Value, E> {
                Ok(StrictValue(Value::Null))
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> Result<Self::Value, A::Error> {
                let mut out = Vec::new();
                while let Some(x) = seq.next_element::<StrictValue>()? {
                    out.push(x.0);
                }
                Ok(StrictValue(Value::Array(out)))
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<Self::Value, A::Error> {
                let mut out = serde_json::Map::new();
                while let Some((k, v)) = map.next_entry::<String, StrictValue>()? {
                    if out.insert(k, v.0).is_some() {
                        return Err(serde::de::Error::custom("duplicate quality object key"));
                    }
                }
                Ok(StrictValue(Value::Object(out)))
            }
        }
        d.deserialize_any(Visitor)
    }
}
fn keys(v: &Value, ks: &[&str]) -> Result<(), &'static str> {
    let o = v.as_object().ok_or("quality object required")?;
    if o.len() != ks.len() || ks.iter().any(|k| !o.contains_key(*k)) {
        Err("quality keys differ")
    } else {
        Ok(())
    }
}
fn quantity(v: &Value) -> bool {
    serde_json::from_value::<crate::metrics::MetricNumber>(v.clone()).is_ok()
}
fn quantity_value(v: &Value) -> Option<BigRational> {
    serde_json::from_value::<crate::metrics::MetricNumber>(v.clone())
        .ok()
        .map(|n| n.0)
}
fn numeric_reason(v: &Value) -> bool {
    v.is_null()
        || v.as_str().is_some_and(|r| {
            [
                "zero_denominator",
                "denominator_may_be_zero",
                "undetermined",
                "truncated",
                "aborted",
                "no_samples",
                "not_configured",
            ]
            .contains(&r)
        })
}
fn validate_range(v: &Value) -> Result<(), &'static str> {
    keys(v, &["lower", "upper", "basis", "reason"])?;
    if !numeric_reason(&v["reason"]) {
        return Err("invalid quality reason");
    }
    if v["reason"] == "not_configured" && !v["lower"].is_null() {
        return Err("unconfigured quality range carries numbers");
    }
    if v["lower"].is_null() {
        if !v["upper"].is_null() || !v["basis"].is_null() || v["reason"].is_null() {
            return Err("false quality null");
        }
    } else if !quantity(&v["lower"])
        || !quantity(&v["upper"])
        || !matches!(v["basis"].as_str(), Some("exact" | "per_case_relaxation"))
    {
        return Err("invalid quality range");
    }
    if let (Some(l), Some(u)) = (quantity_value(&v["lower"]), quantity_value(&v["upper"]))
        && (l > u || (v["basis"] == "exact" && (l != u || !v["reason"].is_null())))
    {
        return Err("invalid quality range ordering or exactness");
    }
    Ok(())
}
fn validate_scalar(v: &Value) -> Result<(), &'static str> {
    validate_range(v)?;
    if v["reason"].is_null() == v["lower"].is_null() {
        return Err("quality scalar reason/value mismatch");
    }
    Ok(())
}
fn validate_sample(v: &Value) -> Result<(), &'static str> {
    keys(v, &["mean", "min", "max", "samples", "reason"])?;
    if v["samples"].as_u64().is_none() || !numeric_reason(&v["reason"]) {
        return Err("invalid quality sample");
    }
    if v["samples"] == 0 {
        if !["mean", "min", "max"].iter().all(|k| v[k].is_null())
            || !matches!(
                v["reason"].as_str(),
                Some("aborted" | "no_samples" | "not_configured")
            )
        {
            return Err("false empty quality sample");
        }
    } else if !["mean", "min", "max"].iter().all(|k| quantity(&v[k])) || !v["reason"].is_null() {
        return Err("invalid populated quality sample");
    }
    if let (Some(l), Some(mean), Some(u)) = (
        quantity_value(&v["min"]),
        quantity_value(&v["mean"]),
        quantity_value(&v["max"]),
    ) && (l > mean || mean > u)
    {
        return Err("invalid quality sample ordering");
    }
    Ok(())
}
pub(crate) fn validate_wire(q: &Quality) -> Result<(), &'static str> {
    if q.rules != "quality-cohort-p1-1"
        || !matches!(q.analysis.as_str(), "exhaustive" | "simulation")
    {
        return Err("invalid quality version or mode");
    }
    let sim = q.analysis == "simulation";
    for p in &q.properties {
        keys(
            p,
            if sim {
                &[
                    "name",
                    "condition",
                    "within",
                    "cases",
                    "expected_count",
                    "unknown_count",
                ]
            } else {
                &["name", "condition", "within", "cases", "expected_count"]
            },
        )?;
        if p["name"].as_str().is_none()
            || p["condition"].as_str().is_none()
            || (!p["within"].is_null() && p["within"].as_u64().is_none())
        {
            return Err("invalid quality property definition");
        }
        for c in p["cases"].as_array().ok_or("quality cases missing")? {
            keys(
                c,
                if sim {
                    &["case_id", "arrival", "hits", "unknown", "trials"]
                } else {
                    &["case_id", "arrival", "min", "max", "reason"]
                },
            )?;
            if c["case_id"].as_u64().is_none() || c["arrival"].as_u64().is_none() {
                return Err("invalid quality case identity");
            }
            if sim {
                if ["hits", "unknown", "trials"]
                    .iter()
                    .any(|k| c[k].as_u64().is_none())
                {
                    return Err("invalid quality trials");
                }
                if c["hits"]
                    .as_u64()
                    .unwrap()
                    .checked_add(c["unknown"].as_u64().unwrap())
                    .is_none_or(|n| n > c["trials"].as_u64().unwrap())
                {
                    return Err("quality hits and unknown exceed trials");
                }
            } else if !numeric_reason(&c["reason"])
                || (!(c["min"].is_null()
                    && c["max"].is_null()
                    && matches!(
                        c["reason"].as_str(),
                        Some("aborted" | "not_configured" | "truncated" | "undetermined")
                    ))
                    && (!quantity(&c["min"]) || !quantity(&c["max"])))
            {
                return Err("invalid quality case probability");
            }
            if !sim {
                if c["reason"] == "not_configured" && !c["min"].is_null() {
                    return Err("unconfigured quality case carries numbers");
                }
                if !c["reason"].is_null()
                    && !matches!(
                        c["reason"].as_str(),
                        Some("undetermined" | "truncated" | "aborted" | "not_configured")
                    )
                {
                    return Err("invalid case reason");
                }
                if let (Some(l), Some(u)) = (quantity_value(&c["min"]), quantity_value(&c["max"]))
                    && (l > u || u > number(1))
                {
                    return Err("quality probability out of range");
                }
            }
        }
        if sim {
            validate_sample(&p["expected_count"])?;
            validate_sample(&p["unknown_count"])?;
        } else {
            validate_range(&p["expected_count"])?;
        }
    }
    for c in &q.cohort {
        keys(
            c,
            &[
                "name",
                "form",
                "numerator",
                "denominator",
                "value",
                "bounds_with_undetermined",
            ],
        )?;
        if c["name"].as_str().is_none()
            || c["numerator"].as_str().is_none()
            || c["form"] != "ratio_of_expected_counts"
        {
            return Err("invalid quality ratio definition");
        }
        let d = &c["denominator"];
        match d["kind"].as_str() {
            Some("arrivals") => {
                keys(d, &["kind", "count"])?;
                if d["count"].as_u64().is_none() {
                    return Err("invalid arrival denominator");
                }
            }
            Some("property") => {
                keys(d, &["kind", "property", "covers", "reason"])?;
                if d["property"].as_str().is_none()
                    || !(d["covers"].is_null() && d["reason"] == "state_condition"
                        || d["reason"].is_null()
                            && d["covers"]
                                .as_array()
                                .is_some_and(|a| !a.is_empty() && a.iter().all(Value::is_string)))
                {
                    return Err("invalid property denominator");
                }
            }
            _ => return Err("invalid quality denominator kind"),
        }
        if sim {
            let v = &c["value"];
            keys(
                v,
                &["point", "numerator", "denominator", "samples", "reason"],
            )?;
            validate_sample(&v["numerator"])?;
            validate_sample(&v["denominator"])?;
            if v["samples"].as_u64().is_none()
                || !numeric_reason(&v["reason"])
                || (!v["point"].is_null() && !quantity(&v["point"]))
                || v["point"].is_null() == v["reason"].is_null()
            {
                return Err("invalid quality ratio point");
            }
        } else {
            validate_scalar(&c["value"])?;
        }
        let b = &c["bounds_with_undetermined"];
        if !b.is_null() {
            keys(b, &["lower", "upper"])?;
            if !quantity(&b["lower"]) || !quantity(&b["upper"]) {
                return Err("invalid conservative quality bounds");
            }
        }
    }
    for o in &q.outcomes_per_arrival {
        keys(o, &["kind", "end", "expected_count", "rate"])?;
        if !matches!(
            o["kind"].as_str(),
            Some(
                "end"
                    | "overflow"
                    | "rejected"
                    | "unfinished"
                    | "deadlock"
                    | "cut_off"
                    | "receipt_unknown"
            )
        ) {
            return Err("invalid quality outcome");
        }
        if sim {
            validate_sample(&o["expected_count"])?;
            let r = &o["rate"];
            keys(r, &["point", "samples", "reason"])?;
            if r["samples"].as_u64().is_none()
                || !numeric_reason(&r["reason"])
                || (!r["point"].is_null() && !quantity(&r["point"]))
                || r["point"].is_null() == r["reason"].is_null()
            {
                return Err("invalid quality outcome rate");
            }
        } else {
            validate_range(&o["expected_count"])?;
            validate_scalar(&o["rate"])?;
        }
    }
    quantile_wire(q)?;
    Ok(())
}

#[derive(Debug, Clone)]
pub(crate) struct Solution {
    pub b: [Rational; 4],
    pub reason: Option<&'static str>,
}
impl Solution {
    pub(crate) fn new(b: [Rational; 4], computational: bool) -> Self {
        let unknown = b[0] != b[1] || b[2] != b[3];
        Self {
            b,
            reason: unknown.then_some(if computational {
                "truncated"
            } else {
                "undetermined"
            }),
        }
    }
}
pub(crate) fn number(n: u64) -> BigRational {
    BigRational::from_integer(n.into())
}
/// A DAG reward observation. Some is an absorbing known value; None permits
/// continuation. Unknown adds a possible unexamined continuation, not chance.
pub(crate) fn reduce_with_bits(
    order: &[usize],
    actions: impl Fn(usize) -> Vec<Vec<(Rational, usize)>>,
    observe: impl Fn(usize) -> (Option<bool>, bool),
    bits: u64,
) -> Solution {
    reduce_stopped(
        order,
        actions,
        |i| {
            let (known, unknown) = observe(i);
            (known, unknown, true)
        },
        bits,
    )
}
/// Preserve the provenance of live unknown continuations, independently of the
/// presence of a declared horizon. Arithmetic overflow is always computational.
pub(crate) fn reduce_stopped(
    order: &[usize],
    actions: impl Fn(usize) -> Vec<Vec<(Rational, usize)>>,
    observe: impl Fn(usize) -> (Option<bool>, bool, bool),
    bits: u64,
) -> Solution {
    let mut table = vec![std::array::from_fn::<_, 4, _>(|_| Rational::zero()); order.len()];
    let mut computational = vec![false; order.len()];
    for &i in order.iter().rev() {
        let (known, unknown, cut) = observe(i);
        if let Some(hit) = known {
            table[i] = std::array::from_fn(|_| {
                if hit {
                    Rational::one()
                } else {
                    Rational::zero()
                }
            });
            continue;
        }
        let mut choices = Vec::new();
        computational[i] = unknown && cut;
        if unknown {
            choices.push([
                Rational::zero(),
                Rational::one(),
                Rational::zero(),
                Rational::one(),
            ]);
        }
        for action in actions(i) {
            let mut b = std::array::from_fn::<_, 4, _>(|_| Rational::zero());
            let mut numeric = false;
            for (p, j) in action {
                computational[i] |= !p.0.is_zero() && computational[j];
                numeric |= !p.within_bits(bits);
                for z in 0..4 {
                    let product = Rational(&p.0 * &table[j][z].0);
                    numeric |= !product.within_bits(bits);
                    b[z].0 += product.0;
                    numeric |= !b[z].within_bits(bits);
                }
            }
            computational[i] |= numeric;
            choices.push(if numeric {
                [
                    Rational::zero(),
                    Rational::one(),
                    Rational::zero(),
                    Rational::one(),
                ]
            } else {
                b
            });
        }
        if !choices.is_empty() {
            table[i] = std::array::from_fn(|z| {
                if z < 2 {
                    choices.iter().map(|b| b[z].clone()).min().unwrap()
                } else {
                    choices.iter().map(|b| b[z].clone()).max().unwrap()
                }
            });
        }
    }
    Solution::new(table[0].clone(), computational[0])
}
fn range(l: &BigRational, u: &BigRational, exact: bool, reason: Option<&str>) -> Value {
    json!({"lower":l.to_string(),"upper":u.to_string(),"basis":if exact {"exact"} else {"per_case_relaxation"},"reason":reason})
}
pub(crate) fn absent(reason: &str) -> Value {
    json!({"lower":null,"upper":null,"basis":null,"reason":reason})
}
pub(crate) fn definition(m: &ir::Model, p: &ir::QualityProperty) -> String {
    match &p.target {
        ir::QualityTarget::Ends(es) => es
            .iter()
            .map(|e| format!("end.{}", m.ends[e.0]))
            .collect::<Vec<_>>()
            .join(" || "),
        ir::QualityTarget::State(c) => condition(m, c, 0),
    }
}
fn condition(m: &ir::Model, c: &ir::Cond, parent: u8) -> String {
    let (prec, s) = match c {
        ir::Cond::And(cs) | ir::Cond::Or(cs) => {
            let and = matches!(c, ir::Cond::And(_));
            let p = if and { 2 } else { 1 };
            (
                p,
                cs.iter()
                    .map(|c| condition(m, c, p))
                    .collect::<Vec<_>>()
                    .join(if and { " && " } else { " || " }),
            )
        }
        ir::Cond::StateEq(x) => {
            let v = &m.state_variables[x.variable.0];
            let name = v.kind.actor().map_or_else(
                || v.name.clone(),
                |a| format!("{}.{}", m.actors[a.0].name, v.name),
            );
            (
                3,
                format!("{} == {}", name, literal(m, x.variable, x.value)),
            )
        }
        ir::Cond::IntCompare {
            variable,
            operator,
            value,
        } => {
            let v = &m.state_variables[variable.0];
            let name = v.kind.actor().map_or_else(
                || v.name.clone(),
                |a| format!("{}.{}", m.actors[a.0].name, v.name),
            );
            let op = match operator {
                ir::IntComparison::Eq => "==",
                ir::IntComparison::Ne => "!=",
                ir::IntComparison::Lt => "<",
                ir::IntComparison::Le => "<=",
                ir::IntComparison::Gt => ">",
                ir::IntComparison::Ge => ">=",
            };
            (3, format!("{name} {op} {value}"))
        }
        _ => (3, format!("{c:?}")),
    };
    if prec < parent { format!("({s})") } else { s }
}
fn literal(m: &ir::Model, id: ir::StateVarId, v: ir::StateValue) -> String {
    match v {
        ir::StateValue::Value(v) => m.types[m.state_variables[id.0].base.0].members[v.0].clone(),
        ir::StateValue::None => "none".into(),
        ir::StateValue::Integer(v) => v.to_string(),
        _ => format!("{v:?}"),
    }
}
pub(crate) fn denominator(m: &ir::Model, d: &ir::Denominator) -> Value {
    match d {
        ir::Denominator::Arrivals => json!({"kind":"arrivals","count":m.bounds.cases}),
        ir::Denominator::Property(i) => {
            let p = &m.quality.as_ref().unwrap().properties[*i];
            let (covers, reason) = match &p.target {
                ir::QualityTarget::Ends(es) => (
                    json!(es.iter().map(|e| &m.ends[e.0]).collect::<Vec<_>>()),
                    Value::Null,
                ),
                _ => (Value::Null, json!("state_condition")),
            };
            json!({"kind":"property","property":p.name,"covers":covers,"reason":reason})
        }
    }
}
pub(crate) fn inventory(m: &ir::Model, analysis: &str, aborted: bool) -> Quality {
    let spec = m.quality.as_ref().unwrap();
    let mut checked = vec!["case_reachability", "expected_counts"];
    if !spec.metrics.is_empty() {
        checked.extend(["ratio_of_expected_counts", "denominator_disclosure"]);
    }
    checked.push("outcomes_per_arrival");
    let deadline = spec
        .metrics
        .iter()
        .any(|c| spec.properties[c.numerator].within.is_some());
    let mut unchecked = vec![
        "expectation_of_ratio",
        "cohort_any_probability",
        "metric_intervals",
        "metric_constraints",
        "numerator_subset_of_denominator",
        "business_value",
        "rework_loss",
    ];
    if deadline {
        checked.push("deadline_attainment");
    } else {
        unchecked.push("deadline_attainment");
    }
    if aborted {
        unchecked.append(&mut checked);
    }
    if !spec.quantiles.is_empty() {
        if aborted {
            unchecked.push("quantiles");
        } else {
            checked.push("quantiles");
        }
    }
    Quality {
        rules: "quality-cohort-p1-1".into(),
        analysis: analysis.into(),
        properties: vec![],
        cohort: vec![],
        quantiles: (!spec.quantiles.is_empty()).then(|| {
            spec.quantiles
                .iter()
                .map(|c| quantile_row(m, c, absent("aborted"), Value::Null))
                .collect()
        }),
        outcomes_per_arrival: vec![],
        checked: checked.into_iter().map(str::to_owned).collect(),
        unchecked: unchecked.into_iter().map(str::to_owned).collect(),
    }
}
pub(crate) fn outcome_keys(m: &ir::Model) -> Vec<(&'static str, Option<String>)> {
    let mut keys: Vec<_> = m
        .ends
        .iter()
        .enumerate()
        .map(|(i, e)| {
            (
                if m.overflow_end == Some(ir::EndId(i)) {
                    "overflow"
                } else {
                    "end"
                },
                Some(e.clone()),
            )
        })
        .collect();
    keys.extend([("rejected", None), ("unfinished", None), ("deadlock", None)]);
    if !m.response_attempts().is_empty() {
        keys.push(("receipt_unknown", None));
    }
    if m.horizon.is_some() {
        keys.push(("cut_off", None));
    }
    keys
}
fn sum(v: &[Solution]) -> (BigRational, BigRational, bool, Option<&'static str>) {
    let l = v.iter().map(|s| s.b[0].0.clone()).sum();
    let u = v.iter().map(|s| s.b[3].0.clone()).sum();
    let reason = if v.iter().any(|s| s.reason == Some("truncated")) {
        Some("truncated")
    } else {
        v.iter().find_map(|s| s.reason)
    };
    (l, u, v.iter().all(|s| s.b[0] == s.b[3]), reason)
}
pub(crate) fn exhaustive(
    m: &ir::Model,
    values: &[Vec<Solution>],
    outcomes: &[Vec<Solution>],
    abort: bool,
) -> Quality {
    let mut q = inventory(m, "exhaustive", abort);
    let spec = m.quality.as_ref().unwrap();
    let times = m.arrivals.clone().unwrap_or_else(|| vec![0]);
    let totals: Vec<_> = values.iter().map(|v| sum(v)).collect();
    for (i, p) in spec.properties.iter().enumerate() {
        let (l, u, exact, reason) = &totals[i];
        let cases:Vec<_>=values[i].iter().enumerate().map(|(k,s)|json!({"case_id":k+1,"arrival":times[k],"min":if abort {Value::Null} else {json!(s.b[0].to_string())},"max":if abort {Value::Null} else {json!(s.b[3].to_string())},"reason":if abort {Some("aborted")} else {s.reason}})).collect();
        q.properties.push(json!({"name":p.name,"condition":definition(m,p),"within":p.within,"cases":cases,"expected_count":if abort {absent("aborted")} else {range(l,u,*exact,*reason)}}));
    }
    for c in &spec.metrics {
        let (nl, nu, ne, nr) = &totals[c.numerator];
        let (dl, du, de, dr) = match c.denominator {
            ir::Denominator::Arrivals => {
                (number(m.bounds.cases), number(m.bounds.cases), true, None)
            }
            ir::Denominator::Property(i) => totals[i].clone(),
        };
        let unknown = if *nr == Some("truncated") || dr == Some("truncated") {
            Some("truncated")
        } else {
            nr.or(dr)
        };
        let reason = if abort {
            Some("aborted")
        } else if unknown.is_some() {
            unknown
        } else if du.is_zero() {
            Some("zero_denominator")
        } else if dl.is_zero() {
            Some("denominator_may_be_zero")
        } else {
            None
        };
        let bounds = if !abort && unknown.is_some() && !dl.is_zero() {
            json!({"lower":(nl/&du).to_string(),"upper":(nu/&dl).to_string()})
        } else {
            Value::Null
        };
        let value = match reason {
            Some(r) => absent(r),
            None => range(&(nl / &du), &(nu / &dl), *ne && de, None),
        };
        q.cohort.push(json!({"name":c.name,"form":"ratio_of_expected_counts","numerator":spec.properties[c.numerator].name,"denominator":denominator(m,&c.denominator),"value":value,"bounds_with_undetermined":bounds}));
    }
    for ((kind, end), v) in outcome_keys(m).into_iter().zip(outcomes) {
        let (l, u, exact, reason) = sum(v);
        q.outcomes_per_arrival.push(json!({"kind":kind,"end":end,"expected_count":if abort {absent("aborted")} else {range(&l,&u,exact,reason)},"rate":if abort {absent("aborted")} else if let Some(r)=reason {absent(r)} else {range(&(l/number(m.bounds.cases)),&(u/number(m.bounds.cases)),exact,None)}}));
    }
    q
}

#[derive(Debug, Clone)]
pub(crate) struct Observation {
    pub truths: Vec<Vec<crate::outcome::QueryTruth>>,
    pub outcomes: Vec<usize>,
    pub computational: bool,
    pub arrivals: Vec<Vec<Arrival>>,
}
pub(crate) fn sample(v: &[u64], reason: Option<&str>) -> Value {
    if let Some(r) = reason {
        return json!({"mean":null,"min":null,"max":null,"samples":0,"reason":r});
    }
    json!({"mean":(number(v.iter().sum())/number(v.len() as u64)).to_string(),"min":v.iter().min().unwrap().to_string(),"max":v.iter().max().unwrap().to_string(),"samples":v.len(),"reason":null})
}
pub(crate) fn replication_ratio(
    n: &[u64],
    d: &[u64],
    un: &[u64],
    ud: &[u64],
    abort: bool,
) -> (Value, Value) {
    replication_ratio_stopped(n, d, un, ud, abort, true)
}
fn replication_ratio_stopped(
    n: &[u64],
    d: &[u64],
    un: &[u64],
    ud: &[u64],
    abort: bool,
    computational: bool,
) -> (Value, Value) {
    let missing = if abort {
        Some("aborted")
    } else if n.is_empty() {
        Some("no_samples")
    } else {
        None
    };
    let ns: u64 = n.iter().sum();
    let ds: u64 = d.iter().sum();
    let uns: u64 = un.iter().sum();
    let uds: u64 = ud.iter().sum();
    let reason = missing.or(if uns + uds > 0 {
        Some(if computational {
            "truncated"
        } else {
            "undetermined"
        })
    } else if ds == 0 {
        Some("zero_denominator")
    } else {
        None
    });
    let bounds = if missing.is_none() && uns + uds > 0 && ds > 0 {
        json!({"lower":(number(ns)/number(ds+uds)).to_string(),"upper":(number(ns+uns)/number(ds)).to_string()})
    } else {
        Value::Null
    };
    (
        json!({"point":if reason.is_none() {json!((number(ns)/number(ds)).to_string())} else {Value::Null},"numerator":sample(n,missing),"denominator":sample(d,missing),"samples":if missing.is_some() {0} else {n.len()},"reason":reason}),
        bounds,
    )
}
pub(crate) fn simulation(m: &ir::Model, observations: &[Observation], abort: bool) -> Quality {
    use crate::outcome::QueryTruth;
    let missing = if abort {
        Some("aborted")
    } else if observations.is_empty() {
        Some("no_samples")
    } else {
        None
    };
    let mut q = inventory(m, "simulation", missing.is_some());
    let spec = m.quality.as_ref().unwrap();
    let times = m.arrivals.clone().unwrap_or_else(|| vec![0]);
    let mut totals = Vec::new();
    for (i, p) in spec.properties.iter().enumerate() {
        let n: Vec<_> = observations
            .iter()
            .map(|o| {
                o.truths[i]
                    .iter()
                    .filter(|t| **t == QueryTruth::Hit)
                    .count() as u64
            })
            .collect();
        let u: Vec<_> = observations
            .iter()
            .map(|o| {
                o.truths[i]
                    .iter()
                    .filter(|t| **t == QueryTruth::Unknown)
                    .count() as u64
            })
            .collect();
        let cases:Vec<_>=times.iter().enumerate().map(|(k,a)|json!({"case_id":k+1,"arrival":a,"hits":if abort {0} else {observations.iter().filter(|o|o.truths[i][k]==QueryTruth::Hit).count()},"unknown":if abort {0} else {observations.iter().filter(|o|o.truths[i][k]==QueryTruth::Unknown).count()},"trials":if abort {0} else {observations.len()}})).collect();
        q.properties.push(json!({"name":p.name,"condition":definition(m,p),"within":p.within,"cases":cases,"expected_count":sample(&n,missing),"unknown_count":sample(&u,missing)}));
        totals.push((n, u));
    }
    for c in &spec.metrics {
        let (n, un) = &totals[c.numerator];
        let (d, ud) = match c.denominator {
            ir::Denominator::Arrivals => (
                vec![m.bounds.cases; observations.len()],
                vec![0; observations.len()],
            ),
            ir::Denominator::Property(i) => totals[i].clone(),
        };
        let computational = observations.iter().any(|o| {
            o.computational && (o.truths[c.numerator].contains(&QueryTruth::Unknown)
                || matches!(c.denominator, ir::Denominator::Property(i) if o.truths[i].contains(&QueryTruth::Unknown)))
        });
        let (value, bounds) = if computational {
            replication_ratio(n, &d, un, &ud, abort)
        } else {
            replication_ratio_stopped(n, &d, un, &ud, abort, false)
        };
        q.cohort.push(json!({"name":c.name,"form":"ratio_of_expected_counts","numerator":spec.properties[c.numerator].name,"denominator":denominator(m,&c.denominator),"value":value,"bounds_with_undetermined":bounds}));
    }
    let keys = outcome_keys(m);
    let unknown = keys.iter().position(|(kind, _)| *kind == "receipt_unknown");
    for (i, (kind, end)) in keys.into_iter().enumerate() {
        let counts: Vec<_> = observations
            .iter()
            .map(|o| {
                o.outcomes
                    .iter()
                    .filter(|k| **k == i || kind == "unfinished" && Some(**k) == unknown)
                    .count() as u64
            })
            .collect();
        q.outcomes_per_arrival.push(json!({"kind":kind,"end":end,"expected_count":sample(&counts,missing),"rate":{"point":if missing.is_some() {Value::Null} else {json!((number(counts.iter().sum())/number(observations.len() as u64*m.bounds.cases)).to_string())},"samples":if missing.is_some() {0} else {observations.len()},"reason":missing}}));
    }
    if !spec.quantiles.is_empty() {
        q.quantiles = Some(
            spec.quantiles
                .iter()
                .map(|c| {
                    let all = observations
                        .iter()
                        .flat_map(|o| o.arrivals[c.property].iter().cloned())
                        .collect::<Vec<_>>();
                    let (v, lb) = sample_quantile(&c.p, c.given_reached, &all, abort);
                    quantile_row(m, c, v, lb)
                })
                .collect(),
        );
    }
    q
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn horizon_reason_contract_checks_both_endpoint_pairs() {
        // ADR0045 M1/M2: pure business stop is undetermined; each side's
        // unequal endpoints is independently enough to withhold the value.
        for endpoints in [[0, 1, 1, 1], [0, 0, 0, 1], [0, 1, 0, 1]] {
            let b = endpoints.map(|n| Rational(number(n)));
            assert_eq!(Solution::new(b.clone(), false).reason, Some("undetermined"));
            assert_eq!(Solution::new(b, true).reason, Some("truncated"));
        }
        let b = [0, 0, 1, 1].map(|n| Rational(number(n)));
        assert_eq!(Solution::new(b, false).reason, None);
    }
    #[test]
    fn unknown_numerator_precedes_zero_denominator_and_abort_withdraws_both() {
        for abort in [false, true] {
            let (value, bounds) = replication_ratio(&[0], &[0], &[1], &[0], abort);
            assert_eq!(value["reason"], if abort { "aborted" } else { "truncated" });
            assert!(value["point"].is_null());
            assert!(bounds.is_null());
            assert_eq!(value["samples"], if abort { 0 } else { 1 });
        }
    }
    #[test]
    fn endpoint_private_contract() {
        crate::quality_private_contract::endpoint_contract(|v| {
            let b = std::array::from_fn(|i| {
                Rational(v[i].as_str().unwrap().parse::<BigRational>().unwrap())
            });
            let s = Solution::new(b, true);
            json!({"unknown":s.reason.is_some(),"basis":if s.b[0]==s.b[3] {"exact"} else {"per_case_relaxation"},"reason":s.reason})
        });
    }
    #[test]
    fn replication_private_contract() {
        crate::quality_private_contract::replication_contract(|v| {
            let a = |k: &str| {
                v[k].as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_u64().unwrap())
                    .collect::<Vec<_>>()
            };
            let (r, b) = replication_ratio(
                &a("numerator"),
                &a("denominator"),
                &a("unknown_numerator"),
                &a("unknown_denominator"),
                false,
            );
            json!({"point":r["point"],"numerator_sample":r["numerator"],"denominator_sample":r["denominator"],"reason":r["reason"],"bounds":b})
        });
    }
}
