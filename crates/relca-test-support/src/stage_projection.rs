//! Project only the added stage fields for pre-existing test obligations.
//! The stage acceptance checks these fields separately; old expectations stay
//! literal. Every removal is checked, including raw-byte removal.
use serde_json::Value;

pub const FIELDS: &[&str] = &["stages", "stages_status", "stages_reason"];
pub const UNCHECKED: &[&str] = &[
    "stage_observation(stage_correspondence_not_defined)",
    "stage_observation(fallback_path)",
    "stage_observation(rule)",
    "responsibility_stage(contest_path)",
];

/// Strict entrypoint for an actual observation, not a legacy expectation.
pub fn observation(value: &Value) -> Value {
    let mut value = value.clone();
    let o = value.as_object_mut().expect("observation object");
    if o.contains_key("required_sources") {
        assert!(
            o.contains_key("stages"),
            "missing added responsibility stages"
        );
        assert!(
            o.contains_key("stages_status"),
            "missing added stages_status"
        );
        let status = o.remove("stages_status").expect("stages_status exists");
        o.remove("stages").expect("stages exists");
        if status != "derived" {
            assert!(
                o.contains_key("stages_reason"),
                "missing added stages_reason"
            );
            o.remove("stages_reason").expect("stages_reason exists");
        } else {
            assert!(
                !o.contains_key("stages_reason"),
                "derived stage has no reason"
            );
        }
    } else if o.get("status").and_then(Value::as_str) == Some("derived") {
        assert!(o.contains_key("sources"), "viewpoint observation sources");
        assert!(o.contains_key("stages"), "missing added observation stages");
        o.remove("stages").expect("stages exists");
    } else {
        assert!(
            !o.contains_key("stages"),
            "underived block has no partial stages"
        );
    }
    value
}

/// Legacy validators also receive hand-written old wire. Only project objects
/// carrying the new marker, and assert each field before it is removed. Unknown
/// fields and all pre-existing leaves are left to the original closed validator.
pub fn marked_observation(value: &Value) -> Option<Value> {
    let o = value.as_object()?;
    if !o.contains_key("stages") {
        return None;
    }
    if o.contains_key("sources") {
        if value["status"] != "derived" || !value["stages"].is_array() {
            return None;
        }
    } else if o.contains_key("required_sources") {
        match value["stages_status"].as_str() {
            Some("derived") if value["stages"].is_array() && !o.contains_key("stages_reason") => {}
            Some("not_derived" | "not_applicable")
                if value["stages"].is_null() && value["stages_reason"].is_string() => {}
            _ => return None,
        }
    } else {
        return None;
    }
    Some(observation(value))
}

pub fn root_unchecked(value: &Value) -> Option<Value> {
    let unchecked = marked_unchecked(value.get("unchecked")?)?;
    let mut out = value.clone();
    assert!(out.as_object_mut().unwrap().contains_key("unchecked"));
    out["unchecked"] = unchecked;
    Some(out)
}

pub fn marked_unchecked(value: &Value) -> Option<Value> {
    let a = value.as_array()?;
    if !a
        .iter()
        .any(|v| v.as_str().is_some_and(|s| UNCHECKED.contains(&s)))
    {
        return None;
    }
    let mut a = a.clone();
    for word in UNCHECKED {
        while let Some(i) = a.iter().position(|v| v.as_str() == Some(word)) {
            assert_eq!(a.get(i).and_then(Value::as_str), Some(*word));
            assert_eq!(a.remove(i).as_str(), Some(*word));
        }
    }
    Some(Value::Array(a))
}
pub fn unchecked(value: &Value) -> Value {
    marked_unchecked(value).expect("expected the added stage unchecked word before projection")
}

pub fn project(value: &Value) -> Value {
    fn walk(value: &mut Value) {
        if let Some(projected) = marked_observation(value) {
            *value = projected;
        }
        match value {
            Value::Object(o) => {
                for (key, value) in o {
                    if key == "unchecked"
                        && let Some(a) = value.as_array_mut()
                    {
                        for word in UNCHECKED {
                            while let Some(i) = a.iter().position(|v| v.as_str() == Some(word)) {
                                assert_eq!(a.get(i).and_then(Value::as_str), Some(*word));
                                assert_eq!(a.remove(i).as_str(), Some(*word));
                            }
                        }
                    }
                    walk(value);
                }
            }
            Value::Array(a) => a.iter_mut().for_each(walk),
            _ => {}
        }
    }
    let mut out = value.clone();
    walk(&mut out);
    out
}

/// Actual report/viewpoints entrypoints must carry the new fields. Do not use
/// this for hand-written legacy wire or the deliberately unchecked diff cells.
pub fn current(value: &Value) -> Value {
    fn require(value: &Value) {
        match value {
            Value::Object(o) => {
                if o.contains_key("framing")
                    && o.contains_key("retention")
                    && o.contains_key("observation")
                {
                    let _ = observation(&o["observation"]);
                }
                if o.contains_key("person")
                    && o.contains_key("target")
                    && o.contains_key("observation")
                {
                    let _ = observation(&o["observation"]);
                }
                o.values().for_each(require);
            }
            Value::Array(a) => a.iter().for_each(require),
            _ => {}
        }
    }
    require(value);
    project(value)
}

/// A production DTO may supply defaults while decoding a handwritten legacy
/// wire. Remove just those added defaults from its round-trip copy; current
/// wire keeps its complete new payload in this comparison.
pub fn legacy_dto(canonical: &Value, original: &Value) -> Value {
    fn walk(canonical: &mut Value, original: &Value) {
        if let (Some(c), Some(o)) = (canonical.as_object_mut(), original.as_object()) {
            if !o.contains_key("stages") && o.contains_key("required_sources") {
                assert!(c.contains_key("stages") && c.contains_key("stages_status"));
                assert!(c["stages"].is_null());
                assert_eq!(c["stages_status"], "");
                c.remove("stages").expect("added default stages exists");
                c.remove("stages_status")
                    .expect("added default stages_status exists");
                assert!(!c.contains_key("stages_reason"));
            } else if !o.contains_key("stages")
                && o.contains_key("sources")
                && o.get("status").is_some_and(|s| s == "derived")
            {
                assert!(c.contains_key("stages"));
                assert_eq!(c["stages"], serde_json::json!([]));
                c.remove("stages").expect("added default stages exists");
            }
            for (key, value) in c {
                if let Some(old) = o.get(key) {
                    walk(value, old);
                }
            }
        } else if let (Some(c), Some(o)) = (canonical.as_array_mut(), original.as_array()) {
            for (value, old) in c.iter_mut().zip(o) {
                walk(value, old);
            }
        }
    }
    let mut out = canonical.clone();
    walk(&mut out, original);
    out
}

/// Delete byte ranges, without serializing any retained value or whitespace.
pub fn bytes(raw: &[u8]) -> Vec<u8> {
    fn ws(raw: &[u8], i: &mut usize) {
        while raw.get(*i).is_some_and(u8::is_ascii_whitespace) {
            *i += 1;
        }
    }
    fn parse(
        raw: &[u8],
        i: &mut usize,
        path: &mut Vec<String>,
        cuts: &mut Vec<(usize, usize)>,
    ) -> Value {
        ws(raw, i);
        if !matches!(raw[*i], b'[' | b'{') {
            let mut stream = serde_json::Deserializer::from_slice(&raw[*i..]).into_iter::<Value>();
            let value = stream
                .next()
                .expect("JSON token")
                .expect("valid JSON token");
            *i += stream.byte_offset();
            return value;
        }
        let object = raw[*i] == b'{';
        let close = if object { b'}' } else { b']' };
        *i += 1;
        let start = *i;
        ws(raw, i);
        let mut entries = Vec::new();
        let mut result = if object {
            serde_json::json!({})
        } else {
            serde_json::json!([])
        };
        while raw[*i] != close {
            let begin = *i;
            let key = if object {
                let key = parse(raw, i, path, cuts)
                    .as_str()
                    .expect("object key")
                    .to_owned();
                ws(raw, i);
                assert_eq!(raw[*i], b':');
                *i += 1;
                key
            } else {
                entries.len().to_string()
            };
            path.push(key.clone());
            let value = parse(raw, i, path, cuts);
            path.pop();
            entries.push((begin, *i, key.clone(), value.clone()));
            if object {
                assert!(result.as_object_mut().unwrap().insert(key, value).is_none());
            } else {
                result.as_array_mut().unwrap().push(value);
            }
            ws(raw, i);
            if raw[*i] == b',' {
                *i += 1;
                ws(raw, i);
            } else {
                assert_eq!(raw[*i], close);
            }
        }
        let observation = path.last().is_some_and(|p| p == "observation")
            && (result.get("sources").is_some() || result.get("required_sources").is_some());
        let fields: Vec<_> = if observation {
            let projected = observation_fields(&result);
            projected.into_iter().map(String::from).collect()
        } else {
            Vec::new()
        };
        let mut removed = Vec::new();
        for (n, (_, _, key, value)) in entries.iter().enumerate() {
            if fields.contains(key)
                || (!object
                    && path.last().is_some_and(|p| p == "unchecked")
                    && value.as_str().is_some_and(|s| UNCHECKED.contains(&s)))
            {
                removed.push(n);
            }
        }
        let mut runs: Vec<(usize, usize)> = Vec::new();
        for n in removed {
            if let Some((_, last)) = runs.last_mut()
                && *last + 1 == n
            {
                *last = n;
            } else {
                runs.push((n, n));
            }
        }
        for (first, last) in runs {
            if last + 1 < entries.len() {
                cuts.push((entries[first].0, entries[last + 1].0));
            } else if first > 0 {
                cuts.push((entries[first - 1].1, entries[last].1));
            } else {
                cuts.push((start, entries[last].1));
            }
        }
        *i += 1;
        result
    }
    fn observation_fields(value: &Value) -> Vec<&'static str> {
        // This strict call asserts the existence of every removed field first.
        let _ = observation(value);
        FIELDS
            .iter()
            .copied()
            .filter(|key| value.get(key).is_some())
            .collect()
    }
    let mut cuts = Vec::new();
    let mut i = 0;
    parse(raw, &mut i, &mut Vec::new(), &mut cuts);
    ws(raw, &mut i);
    assert_eq!(i, raw.len());
    cuts.sort_unstable();
    let mut ranges: Vec<(usize, usize)> = Vec::new();
    for (lo, hi) in cuts {
        if let Some((_, end)) = ranges.last_mut()
            && lo <= *end
        {
            *end = (*end).max(hi);
        } else {
            ranges.push((lo, hi));
        }
    }
    let mut out = raw.to_vec();
    for (lo, hi) in ranges.into_iter().rev() {
        out.drain(lo..hi);
    }
    serde_json::from_slice::<Value>(&out).expect("projected JSON remains valid");
    out
}
