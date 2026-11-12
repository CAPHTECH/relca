#![allow(dead_code)]
use serde_json::{Value, json};

pub const BEFORE: &str = "x = A.one(); spare = A.two(); y = R.use(x); end done";
pub const AFTER: &str = "x = A.two(); spare = A.one(); y = R.use(x); end done";
pub const JOIN_ANY: &str =
    "join any f where result == pass tie by [x, spare] remaining drain if none -> no_match";
pub fn join_source(same: bool, all: bool, intent: &str) -> String {
    let before =
        format!("fork f {{ x = A.one(); spare = A.two() }}; {JOIN_ANY}; end done; end no_match");
    let after = if same {
        before.clone()
    } else {
        format!("fork f {{ x = A.two(); spare = A.one() }}; {JOIN_ANY}; end done; end no_match")
    };
    let s = source(&before, &after, intent);
    if all {
        s.replace(JOIN_ANY, "join all f")
    } else {
        s
    }
}
pub fn join_positive(run: impl Fn(&str, &str, &str) -> Value) {
    let src = join_source(false, false, "");
    let parsed = relca_core::syntax::parse(&src).unwrap();
    for name in ["From", "To"] {
        relca_core::lower::lower_configuration(&parsed, name).unwrap();
    }
    for s in [src.clone(), src.replace("tie by [x, spare]", "tie nondet")] {
        let v = run(&s, "From", "To");
        let rows = vec![row("spare", "A.two", "A.one"), row("x", "A.one", "A.two")];
        exact(&v, rows.clone(), 2, 2);
        assert_eq!(v["differences"], json!(rows));
        exact(
            &run(&s, "To", "From"),
            vec![row("spare", "A.one", "A.two"), row("x", "A.two", "A.one")],
            2,
            2,
        );
    }
}
pub fn join_controls(run: impl Fn(&str, &str, &str) -> Value) {
    for (same, all) in [(true, false), (false, true)] {
        let v = run(&join_source(same, all, ""), "From", "To");
        exact(&v, vec![], 0, 0);
        assert_eq!(v["differences"], json!([]));
    }
}
pub fn join_intent(run: impl Fn(&str, &str, &str) -> Value) {
    let src = join_source(
        false,
        false,
        "intent { change relation x reason \"transfer\" }",
    );
    let mut x = row("x", "A.one", "A.two");
    x["reason"] = json!("transfer");
    let v = run(&src, "From", "To");
    exact(&v, vec![row("spare", "A.two", "A.one"), x], 1, 2);
    assert_eq!(warnings(&v, "E-DIFF-0"), 0);
}
pub fn source(before: &str, after: &str, intent: &str) -> String {
    format!(
        r#"model Producer version "0.2" {{
  type V = enum {{ pass, fail }}
  people [p]
  actor A {{ capacity 1
    capability one() -> V {{ duration fixed 1min; result nondet {{ pass }} }}
    capability two() -> V {{ duration fixed 1min; result nondet {{ pass }} }}
  }}
  actor R {{ capacity 2
    capability use(v:V) -> V {{ reads [v]; duration fixed 1min; result nondet {{ pass }} }}
    capability twice(v:V,w:V) -> V {{ reads [v,w]; duration fixed 1min; result nondet {{ pass }} }}
  }}
  process Before {{ {before} }}
  process After {{ {after} }}
  cut C {{ include [A,R] }}
  configuration From {{ cut C; process Before }}
  configuration To {{ cut C; process After {intent} }}
  bounds {{ cases 1; live_tasks 4; clock 20min }}
  verify safety no_resource_overcommit
}}"#
    )
}
pub fn row(target: &str, from: &str, to: &str) -> Value {
    json!({"kind":"relation","target":target,"change":"modified","reason":null,"loses":null,
        "from":{"producer":from},"to":{"producer":to}})
}
pub fn producer_rows(v: &Value) -> Vec<Value> {
    v["differences"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["kind"] == "relation" && r["change"] == "modified")
        .cloned()
        .collect()
}
pub fn warnings(v: &Value, code: &str) -> usize {
    v["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["code"] == code)
        .count()
}
pub fn exact(v: &Value, rows: Vec<Value>, w1: usize, w3: usize) {
    assert_eq!(v["verdict"], "satisfied", "{v}");
    assert_eq!(producer_rows(v), rows, "producer rows: {v}");
    assert_eq!(warnings(v, "W-DIFF-1"), w1, "{v}");
    assert_eq!(warnings(v, "W-DIFF-3"), w3, "{v}");
}
pub fn positive(run: impl Fn(&str, &str, &str) -> Value) {
    let src = source(BEFORE, AFTER, "");
    let v = run(&src, "From", "To");
    exact(&v, vec![row("x", "A.one", "A.two")], 1, 1);
    assert_eq!(v["differences"], json!([row("x", "A.one", "A.two")]));
    assert_eq!(
        v["unchecked"],
        json!(["relation_state_producers", "relation_binding_identity"])
    );
    assert!(
        v["checked"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s == "relation")
    );
    for side in ["from", "to"] {
        assert_eq!(
            v[side]["report"]["terminals"][0]["elapsed"],
            json!({"min":3,"max":3})
        );
        assert_eq!(
            v[side]["report"]["terminals"][0]["work"],
            json!({"min":3,"max":3})
        );
    }
    exact(
        &run(&src, "To", "From"),
        vec![row("x", "A.two", "A.one")],
        1,
        1,
    );
    let reordered = source(
        BEFORE,
        "spare = A.one(); x = A.two(); y = R.use(x); end done",
        "",
    );
    exact(
        &run(&reordered, "From", "To"),
        vec![row("x", "A.one", "A.two")],
        1,
        1,
    );
    let cross = source(BEFORE,AFTER,"").replace("A.two()","B.one()")
        .replace("  actor R", "  actor B { grain document; capacity 1; capability one() -> V { duration fixed 1min; result nondet { pass } } }\n  actor R")
        .replace("include [A,R]","include [A,B,R]");
    exact(
        &run(&cross, "From", "To"),
        vec![row("x", "A.one", "B.one")],
        1,
        1,
    );
    let fork = source(
        "fork f { x = A.one(); spare = A.two() }; join all f; y = R.use(x); end done",
        "fork f { x = A.two(); spare = A.one() }; join all f; y = R.use(x); end done",
        "",
    );
    exact(
        &run(&fork, "From", "To"),
        vec![row("x", "A.one", "A.two")],
        1,
        1,
    );
}
pub fn controls(run: impl Fn(&str, &str, &str) -> Value) {
    for (before, after) in [
        (BEFORE.to_owned(), BEFORE.to_owned()),
        (BEFORE.into(), AFTER.replace("x", "z")),
        (
            BEFORE.replace("y = R.use(x);", ""),
            AFTER.replace("y = R.use(x);", ""),
        ),
        (BEFORE.into(), BEFORE.replace("y =", "other =")),
    ] {
        exact(
            &run(&source(&before, &after, ""), "From", "To"),
            vec![],
            0,
            0,
        );
    }
    let state = source(
        &BEFORE.replace(
            "y = R.use(x); end done",
            "if flag == true { end done } else { end other }",
        ),
        &AFTER.replace(
            "y = R.use(x); end done",
            "if flag == true { end done } else { end other }",
        ),
        "",
    )
    .replace(
        "  people [p]",
        "  observed case { flag: bool = false }\n  people [p]",
    )
    .replace(
        "capability one() -> V {",
        "capability one() -> V { writes flag; finish { flag = true };",
    )
    .replace(
        "capability two() -> V {",
        "capability two() -> V { writes flag; finish { flag = true };",
    );
    exact(&run(&state, "From", "To"), vec![], 0, 0);
}
pub fn lexical_sources() -> Vec<String> {
    let variants = [
        (
            "y = R.use(x); z = R.use(x); end done",
            "y = R.use(x); z = R.use(x); end done",
        ),
        (
            "fork f { y = R.use(x); z = R.use(x) }; join all f; end done",
            "fork f { y = R.use(x); z = R.use(x) }; join all f; end done",
        ),
        ("y = R.twice(x,x); end done", "y = R.twice(x,x); end done"),
        (
            "if x == pass { end done } else { end other }",
            "if x == pass { end done } else { end other }",
        ),
        (
            "if spare == pass { end done } else { y = R.use(x); end done }",
            "if spare == pass { end done } else { y = R.use(x); end done }",
        ),
        ("end done; y = R.use(x)", "end done; y = R.use(x)"),
    ];
    variants
        .into_iter()
        .map(|(a, b)| {
            // spare must keep its producer in the condition-only/dead-branch case.
            let before = format!("spare = A.one(); x = A.one(); {a}");
            let after = format!("spare = A.one(); x = A.two(); unused = A.one(); {b}");
            source(&before, &after, "")
        })
        .collect()
}
pub fn lexical(run: impl Fn(&str, &str, &str) -> Value) {
    for src in lexical_sources() {
        let v = run(&src, "From", "To");
        assert_eq!(v["verdict"], "satisfied", "{v}");
        assert_eq!(
            producer_rows(&v),
            vec![row("x", "A.one", "A.two")],
            "{src}: {v}"
        );
    }
    let a = "x = A.one(); spare = A.two(); y = R.twice(x,spare); end done";
    let b = "x = A.two(); spare = A.one(); y = R.twice(x,spare); end done";
    exact(
        &run(&source(a, b, ""), "From", "To"),
        vec![row("spare", "A.two", "A.one"), row("x", "A.one", "A.two")],
        2,
        2,
    );
}
pub fn intent(run: impl Fn(&str, &str, &str) -> Value) {
    let src = source(
        BEFORE,
        AFTER,
        "intent { change relation x reason \"transfer\" }",
    );
    let mut r = row("x", "A.one", "A.two");
    r["reason"] = json!("transfer");
    exact(&run(&src, "From", "To"), vec![r], 0, 1);
    for (body, code, verdict) in [
        (
            "change relation x reason \"transfer\"",
            "E-DIFF-0",
            "invalid",
        ),
        (
            "change relation missing reason \"transfer\"",
            "E-NAME-0",
            "invalid",
        ),
        ("override relation x by A", "U-SEM-0", "unsupported"),
        ("maintain relation x by p", "U-SEM-0", "unsupported"),
    ] {
        let src = source(
            BEFORE,
            if code == "E-DIFF-0" { BEFORE } else { AFTER },
            &format!("intent {{ {body} }}"),
        );
        let v = run(&src, "From", "To");
        assert_eq!(v["verdict"], verdict, "{body}: {v}");
        assert!(
            v["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .chain(v["to"]["report"]["diagnostics"].as_array().unwrap())
                .any(|d| d["code"] == code),
            "{v}"
        );
    }
    let src = source(
        BEFORE,
        AFTER,
        "intent { change relation x reason \"transfer\"; preserve Ctl on relation x }",
    )
    .replace(
        "  verify safety",
        "  contract Ctl { at_most_once A.one.commit per case.id }\n  verify safety",
    )
    .replace(
        "capability one() -> V {",
        "capability one() -> V { commits;",
    );
    let v = run(&src, "From", "To");
    assert_eq!(v["verdict"], "satisfied", "{v}");
    assert_eq!(warnings(&v, "W-DIFF-1"), 0);
    assert_eq!(warnings(&v, "W-DIFF-3"), 0);
    let rows = producer_rows(&v);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["annotations"][0]["kind"], "preserve");
    assert_eq!(rows[0]["annotations"][0]["global"], false);
}
pub fn failures(run: impl Fn(&str, &str, &str) -> Value) {
    let mismatch = source(BEFORE, AFTER, "").replace(
        "configuration To { cut C;",
        "cut D { include [A,R] }\n  configuration To { cut D;",
    );
    let invalid = source(BEFORE, AFTER, "").replace("x = A.two()", "x = Unknown.two()");
    for src in [mismatch, invalid] {
        let v = run(&src, "From", "To");
        assert_ne!(v["verdict"], "satisfied");
        assert!(producer_rows(&v).is_empty());
        assert!(
            v["unchecked"]
                .as_array()
                .unwrap()
                .iter()
                .any(|s| s == "relation")
        );
        for word in ["relation_state_producers", "relation_binding_identity"] {
            assert!(!v["unchecked"].as_array().unwrap().iter().any(|s| s == word));
        }
    }
}
pub fn invoice(run: impl Fn(&str, &str, &str) -> Value, src: &str) {
    for (a, b, rows) in [
        (
            "AsIs",
            "ToBeA",
            vec![row("entry", "Clerk.enter", "Extractor.extract")],
        ),
        ("AsIs", "ToBeP", vec![]),
        (
            "ToBeP",
            "ToBeA",
            vec![row("entry", "Clerk.enter", "Extractor.extract")],
        ),
        (
            "ToBeA",
            "ToBeP",
            vec![row("entry", "Extractor.extract", "Clerk.enter")],
        ),
    ] {
        let v = run(src, a, b);
        // Existing directed ToBeA intent need not be valid in reverse; producer
        // census is nevertheless static and must not silently disappear.
        assert_eq!(producer_rows(&v), rows, "{a}->{b}: {v}");
    }
}
pub fn wire_controls() {
    let mut preserved = row("x", "A.one", "A.two");
    preserved["annotations"] =
        json!([{"kind":"preserve","selector":"Ctl","global":false,"properties":[]}]);
    for r in [
        row("x", "A.one", "A.two"),
        preserved,
        json!({"kind":"relation","target":"A.one","change":"added","reason":null,"loses":null}),
        json!({"kind":"relation","target":"A.one","change":"removed","reason":null,"loses":null,"annotations":[{"kind":"keep_fallback"}]}),
    ] {
        let decoded: relca_core::diff::Difference =
            serde_json::from_value(r.clone()).expect("valid closed relation form");
        assert_eq!(serde_json::to_value(decoded).unwrap(), r);
    }
}
pub fn wire_faults() {
    let valid = row("x", "A.one", "A.two");
    let mut faults = vec![];
    for key in ["from", "to"] {
        let mut b = valid.clone();
        b.as_object_mut().unwrap().remove(key);
        faults.push(b);
        let mut b = valid.clone();
        b[key] = Value::Null;
        faults.push(b);
    }
    for producer in ["", "A", "A.one.more"] {
        let mut b = valid.clone();
        b["from"]["producer"] = json!(producer);
        faults.push(b);
    }
    for change in ["added", "removed"] {
        let mut b = valid.clone();
        b["change"] = json!(change);
        faults.push(b);
    }
    let mut b = valid.clone();
    b["from"] = b["to"].clone();
    faults.push(b);
    let mut b = valid.clone();
    b["extra"] = json!(1);
    faults.push(b);
    let mut b = valid.clone();
    b["to"]["extra"] = json!(1);
    faults.push(b);
    let mut b = valid;
    b["target"] = json!("A.one");
    faults.push(b);
    for b in faults {
        assert!(
            serde_json::from_value::<relca_core::diff::Difference>(b.clone()).is_err(),
            "accepted {b}"
        );
    }
}
pub fn forbidden_producer_annotations() -> Vec<Value> {
    vec![
        json!({"kind":"override","subject_kind":"actor","subject":"A","verdict":"inconclusive","effects":[],"reason":null,"unchecked":[]}),
        json!({"kind":"maintain","person":"p","links":[],"reason":null,"unlinked_reason":"no duty"}),
        json!({"kind":"keep_fallback"}),
    ]
}
pub fn wire_annotations() {
    let mut accepted = vec![];
    for annotation in forbidden_producer_annotations() {
        // The annotation itself must decode; only its producer attachment fails.
        let _: relca_core::diff::Annotation = serde_json::from_value(annotation.clone()).unwrap();
        let mut b = row("x", "A.one", "A.two");
        b["annotations"] = json!([annotation]);
        if serde_json::from_value::<relca_core::diff::Difference>(b.clone()).is_ok() {
            accepted.push(b);
        }
    }
    assert!(
        accepted.is_empty(),
        "accepted producer annotations {accepted:?}"
    );
}
