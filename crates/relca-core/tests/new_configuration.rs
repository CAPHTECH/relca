//! Unlock boundaries through public report entry points. No internal spies.
use relca_core::{diff, report};
use serde_json::{Value, json};
#[path = "../../relca-cli/tests/support/ledger_shape.rs"]
mod ledger_shape;
use std::{fs, path::PathBuf};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}
fn src(name: &str) -> String {
    fs::read_to_string(root().join(name).join("model.relca")).unwrap()
}
fn value(s: &str, name: &str) -> Value {
    serde_json::to_value(report::check_configuration(s, name)).unwrap()
}
fn error(v: &Value, code: &str) {
    let errors: Vec<_> = v["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["severity"] == "error")
        .map(|d| d["code"].as_str().unwrap())
        .collect();
    assert!(!errors.is_empty());
    assert!(
        errors.iter().all(|c| *c == code),
        "unexpected earlier error: {errors:?}"
    );
}
fn add(s: &str, clause: &str) -> String {
    let i = s.rfind('}').unwrap();
    format!("{}\n{clause}\n{}", &s[..i], &s[i..])
}

const CANCEL: &str = r#"model CancelRead version "0.2" {
  type Result = enum { pass, ok }
  actor F { capacity 2; capability run() -> Result { duration fixed 1min; result nondet { pass }; cancel forbidden_after_start } }
  actor S { capacity 1; capability run() -> Result { duration fixed 3min; result nondet { ok }; cancel forbidden_after_start } }
  actor H { capacity 2; capability use(x: Result) -> Result { duration fixed 1min; result nondet { ok } } }
  process Flow {
    fork f { fast = F.run(); slow = S.run() }
    join any f where result == pass tie by [fast,slow] remaining cancel_then_drain if none -> rejected
    if slow == ok { h = H.use(slow); end done } else { end rejected }
  }
  cut K { include [F,S,H] }
  configuration Cplan { cut K; process Flow }
  arrivals fixed [0min]
  bounds { cases 1; live_tasks 2; clock 20min }
  verify safety no_resource_overcommit
}"#;

/// Every generated static-boundary source is enumerated here for preflight as well as acceptance.
fn inputs() -> Vec<(String, String, Option<&'static str>)> {
    let base = src("compare-unlock");
    let mut out = Vec::new();
    for (label, clause) in [
        ("arrivals", "arrivals fixed [0min]"),
        ("policy", "policy fifo tie_by case_id"),
        ("queue", "queue capacity 1 on_overflow reject"),
    ] {
        out.push((
            format!("route-{label}"),
            base.replace("arrivals fixed [0min]", clause),
            None,
        ));
    }
    let multi = src("compare-arrivals");
    for (label, declaration) in [
        ("fixed", "latent flag:bool = true"),
        ("unset", "latent flag:bool"),
        ("distribution", "latent flag:bool ~ {true:0.5,false:0.5}"),
        (
            "write",
            "latent flag:bool = true; capacity 1; capability mutate() -> T { duration fixed 1min; writes flag; outcomes kernel Unused }",
        ),
    ] {
        let kernel = if label == "write" {
            "kernel Unused { * -> {1:ok with flag = false} }"
        } else {
            ""
        };
        let mut s = multi
            .replace(
                "  actor A",
                &format!("  {kernel}\n  actor Shared {{ {declaration} }}\n  actor A"),
            )
            .replace("include [A]", "include [A,Shared]");
        if label == "fixed" {
            s = add(&s, "verify probability reached Shared.flag == true >= 1");
        }
        out.push((
            format!("actor-multi-{label}"),
            s.clone(),
            if label == "fixed" {
                None
            } else {
                Some("U-SEM-0")
            },
        ));
        let one = s
            .replace("cases 2", "cases 1")
            .replace("[0min,0min]", "[0min]");
        out.push((format!("actor-one-{label}"), one, None));
    }
    let q_unknown = base.replace(
        "process Q { a = A.run(); end done }",
        "process Q { a = Missing.run(); end done }",
    );
    out.push(("unselected-unknown".into(), q_unknown, None));
    let latent = base
        .replace(
            "  actor A",
            "  latent case { secret:bool = true }\n  actor A",
        )
        .replace(
            "process Q { a = A.run(); end done }",
            "process Q { if secret == true { end done } else { end done } }",
        );
    out.push((
        "unselected-latent-policy".into(),
        latent,
        Some("E-POL-LATENT"),
    ));
    let duplicate = add(&base, "configuration Aplan { cut C; process P }");
    out.push(("duplicate-configuration".into(), duplicate, Some("U-SEM-0")));
    for (label, read) in [
        (
            "query",
            "verify probability reached Hidden.flag == true >= 0",
        ),
        (
            "invariant",
            "contract X { invariant A.run.finish -> Hidden.flag == true }",
        ),
    ] {
        let s = base
            .replace(
                "  actor A",
                "  actor Hidden { latent flag:bool = true }\n  actor A",
            )
            .replace(
                "include [A]",
                "include [A]; exclude Hidden reason \"external\" loses \"truth\"",
            );
        out.push((format!("cut-{label}"), add(&s, read), Some("E-CUT-1")));
    }
    let kernel = base.replace("  actor A", "  actor Hidden { latent flag:bool = true }\n  kernel Ext given [Hidden.flag] { true -> {1:ok}; false -> {1:ok} }\n  actor A")
        .replace("result nondet { ok }", "outcomes kernel Ext; kernel_reads latent [Hidden.flag]")
        .replace("include [A]", "include [A]; exclude Hidden reason \"external\" loses \"truth\"");
    out.push(("cut-kernel".into(), kernel, Some("E-CUT-1")));
    out.push(("cancel-one".into(), CANCEL.into(), None));
    let multi = CANCEL
        .replace("cases 1", "cases 2")
        .replace("[0min]", "[0min,0min]");
    out.push(("cancel-if".into(), multi.clone(), Some("U-SEM-0")));
    let argument = multi.replace(
        "if slow == ok { h = H.use(slow); end done } else { end rejected }",
        "h = H.use(slow); end done; end rejected",
    );
    out.push(("cancel-argument".into(), argument.clone(), Some("U-SEM-0")));
    let invariant = argument
        .replace("H.use(slow)", "H.use()")
        .replace("use(x: Result)", "use()");
    out.push((
        "cancel-invariant".into(),
        add(
            &invariant,
            "contract X { invariant H.use.finish -> slow == ok }",
        ),
        Some("U-SEM-0"),
    ));
    let observed = multi
        .replace(
            "  actor F",
            "  observed case { slow:Result = ok }\n  actor F",
        )
        .replace("duration fixed 3min;", "writes slow; duration fixed 3min;");
    out.push(("cancel-observed".into(), observed, None));
    out
}

#[test]
fn c_new_route_boundaries_do_not_unlock_actor_mutability_cancel_or_latent_policy() {
    for (label, s, code) in inputs() {
        let name = if label.starts_with("cancel-") {
            "Cplan"
        } else {
            "Aplan"
        };
        let v = value(&s, name);
        if let Some(code) = code {
            error(&v, code);
        } else {
            assert_eq!(v["verdict"], "satisfied", "{label}: {v}");
        }
    }
}

#[test]
fn c_selector_misuse_and_unknown_name_are_not_blanket_unsupported() {
    for name in ["compare-unlock", "compare-arrivals"] {
        let s = src(name);
        error(
            &serde_json::to_value(report::check(&s)).unwrap(),
            "E-CONF-0",
        );
        error(&value(&s, "Unknown"), "E-NAME-0");
    }
    let s = src("arrivals-count");
    error(&value(&s, "Unknown"), "E-CONF-0");
    let s = src("compare-unlock").replace(
        "process Q { a = A.run(); end done }",
        "process Q { a = Missing.run(); end done }",
    );
    error(&value(&s, "Bplan"), "E-NAME-0");
}

fn without_hash_and_spans(v: &mut Value) {
    if let Some(o) = v.as_object_mut() {
        if o.contains_key("sha256") {
            o.insert("sha256".into(), Value::Null);
        }
        if o.contains_key("span") {
            o.insert("span".into(), Value::Null);
        }
        for x in o.values_mut() {
            without_hash_and_spans(x);
        }
    } else if let Some(a) = v.as_array_mut() {
        for x in a {
            without_hash_and_spans(x);
        }
    }
}

#[test]
fn c_twenty_arrivals_inputs_configuration_selection_is_the_only_semantic_delta() {
    let names = [
        "arrivals-contracts",
        "arrivals-contracts.broken-short-response",
        "arrivals-initial-product",
        "arrivals-initial-product.broken-fixed-initial",
        "arrivals-reject-mass",
        "arrivals-reject-mass.broken-larger-queue",
        "arrivals-relative-within",
        "arrivals-relative-within.broken-tight-deadline",
        "arrivals-same-time-slot",
        "arrivals-same-time-slot.broken-later-arrival",
        "arrivals-shared-fixed",
        "arrivals-simultaneous",
        "arrivals-simultaneous.broken-no-policy",
        "arrivals-staggered",
        "arrivals-staggered.broken-extra-capacity",
        "arrivals-zero-queue",
        "arrivals-zero-queue.broken-positive-queue",
        "arrivals-count",
        "arrivals-order",
        "arrivals-required",
    ];
    assert_eq!(names.len(), 20);
    for name in names {
        let s = src(name);
        let ast = relca_core::syntax::parse(&s).unwrap();
        let process = ast
            .items
            .iter()
            .find_map(|i| {
                if let relca_core::ast::Item::Process(p) = i {
                    Some(p.name.value.as_str())
                } else {
                    None
                }
            })
            .unwrap();
        let actors: Vec<_> = ast
            .items
            .iter()
            .filter_map(|i| {
                if let relca_core::ast::Item::Actor(a) = i {
                    Some(a.name.value.as_str())
                } else {
                    None
                }
            })
            .collect();
        let added = add(
            &s,
            &format!(
                "cut SelectedCut {{ include [{}] }}\nconfiguration Selected {{ cut SelectedCut; process {process} }}",
                actors.join(",")
            ),
        );
        let mut before =
            ledger_shape::original_report(&serde_json::to_value(report::check(&s)).unwrap());
        let mut after = ledger_shape::original_report(&value(&added, "Selected"));
        // Declared Cut display and its former undeclared warning are the only metadata changes.
        before["cut"] = Value::Null;
        after["cut"] = Value::Null;
        for v in [&mut before, &mut after] {
            v["diagnostics"]
                .as_array_mut()
                .unwrap()
                .retain(|d| d["code"] != "W-CUT-0");
            without_hash_and_spans(v);
        }
        assert_eq!(
            before, after,
            "{name}: all properties, admission, Agency and times preserved"
        );
    }
}

#[test]
fn c_agency_selection_and_multi_case_annotation_are_preserved() {
    for (name, n) in [("compare-legacy", 1), ("compare-arrivals", 2)] {
        let s = src(name);
        let plans = if n == 1 {
            ["AsIs", "ToBeP"]
        } else {
            ["Aplan", "Bplan"]
        };
        for plan in plans {
            let v = value(&s, plan);
            let standalone = standalone_source(&s, plan);
            let original = serde_json::to_value(report::check(&standalone)).unwrap();
            assert_eq!(
                v["agency"], original["agency"],
                "{name}/{plan}: selected template origin/refusal/lineage and annotation"
            );
            let count = v["agency"]["unchecked"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|x| **x == "multi_case_interference")
                .count();
            assert_eq!(count, usize::from(n > 1));
            if n > 1 {
                // ADR 0043 (plan C04) appends responsibility_people_undeclared
                // after the existing markers when the model declares no people.
                assert_eq!(
                    v["agency"]["unchecked"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .rev()
                        .find(|x| *x != "responsibility_people_undeclared")
                        .unwrap(),
                    "multi_case_interference"
                );
            }
            assert!(v["agency"]["origin"].is_array());
        }
    }
}

fn standalone_source(source: &str, plan: &str) -> String {
    use relca_core::ast::Item;
    let ast = relca_core::syntax::parse(source).unwrap();
    let selected = ast
        .items
        .iter()
        .find_map(|i| {
            if let Item::Configuration(c) = i {
                if c.name.value == plan { Some(c) } else { None }
            } else {
                None
            }
        })
        .unwrap();
    let mut spans: Vec<_> = ast
        .items
        .iter()
        .filter_map(|i| match i {
            Item::Configuration(c) => Some(c.span),
            Item::Process(p) if p.name.value != selected.process.value => Some(p.span),
            Item::Cut(c) if c.name.value != selected.cut.value => Some(c.span),
            _ => None,
        })
        .collect();
    spans.sort_by_key(|s| std::cmp::Reverse(s.start));
    let mut result = source.to_owned();
    for span in spans {
        result.replace_range(span.start..span.end, "");
    }
    result
}

#[test]
fn c_legacy_check_diff_simulate_root_key_sets_are_frozen() {
    let s = src("compare-legacy");
    let check = serde_json::to_value(report::check_configuration(&s, "AsIs")).unwrap();
    let d = serde_json::to_value(diff::diff(&s, "AsIs", "ToBeP")).unwrap();
    let sim = serde_json::to_value(report::simulate_configuration(
        &add(&s, "simulation replications 4 seed 0 confidence 0.5"),
        "AsIs",
    ))
    .unwrap();
    let keys = |v: &Value| v.as_object().unwrap().keys().cloned().collect::<Vec<_>>();
    assert_eq!(
        keys(&check),
        [
            "actors",
            "agency",
            "assumptions",
            "bounds",
            "cut",
            "cut_context",
            "diagnostics",
            "ledger",
            "mode",
            "model",
            "properties",
            "terminals",
            "uncertainty",
            "unit",
            "verdict"
        ]
    );
    assert_eq!(
        keys(&d),
        [
            "agency",
            "checked",
            "diagnostics",
            "differences",
            "from",
            "ledger",
            "mismatch",
            "properties",
            "terminals",
            "to",
            "uncertainty",
            "unchecked",
            "verdict"
        ]
    );
    assert_eq!(
        keys(&sim),
        [
            "actors",
            "admission",
            "agency",
            "assumptions",
            "bounds",
            "case_outcomes",
            "cut",
            "cut_context",
            "diagnostics",
            "ledger",
            "mode",
            "model",
            "properties",
            "simulation",
            "terminals",
            "uncertainty",
            "unchecked",
            "unit",
            "verdict"
        ]
    );
    assert_eq!(check["mode"], "exhaustive");
    assert_eq!(sim["mode"], "simulation");
    assert_eq!(sim["unchecked"], json!(["unreachable"]));
}

#[test]
fn c_preflight_inventory_export() {
    // Explicit opt-in only, in-worktree artifacts, never rewrites acceptance expectations.
    if std::env::var_os("RELCA_EXPORT_PREFLIGHT").is_none() {
        return;
    }
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/preflight-inputs");
    fs::create_dir_all(&dir).unwrap();
    let mut manifest = Vec::new();
    for (label, s, code) in inputs() {
        fs::write(dir.join(format!("{label}.relca")), s).unwrap();
        manifest
            .push(json!({"label":label,"allowed_errors":code.into_iter().collect::<Vec<_>>() }));
    }
    for (name, plans) in [
        ("compare-legacy", ["AsIs", "ToBeP"]),
        ("compare-arrivals", ["Aplan", "Bplan"]),
    ] {
        for plan in plans {
            let label = format!("agency-standalone-{name}-{plan}");
            fs::write(
                dir.join(format!("{label}.relca")),
                standalone_source(&src(name), plan),
            )
            .unwrap();
            manifest.push(json!({"label":label,"allowed_errors":[]}));
        }
    }
    fs::write(
        dir.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
}
