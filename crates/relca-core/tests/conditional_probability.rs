//! ADR 0013 public acceptance; all numeric oracles precede implementation.
use relca_core::{diff, report};
use serde_json::{Value, json};
#[path = "../../relca-cli/tests/support/ledger_shape.rs"]
mod ledger_shape;
use std::{collections::BTreeMap, fs, path::Path};
#[path = "support/diff_increment.rs"]
mod diff_increment;
#[path = "support/responsibility_defaults_oracle.rs"]
mod responsibility_oracle;

const SUM: &str = include_str!("../../../fixtures/prob-sum/model.relca");
const ROWS: &str = include_str!("../../../fixtures/prob-rows/model.relca");
const MDP: &str = include_str!("../../../fixtures/prob-mdp/model.relca");
const CUT: &str = include_str!("../../../fixtures/prob-cut.broken-short-clock/model.relca");
const FINISH: &str = include_str!("../../../fixtures/prob-finish-read/model.relca");
const CANCEL: &str = include_str!("../../../fixtures/prob-cancel/model.relca");

fn check(s: &str) -> Value {
    serde_json::to_value(report::check(s)).unwrap()
}
fn property(v: &Value, n: usize) -> &Value {
    let id = format!("probability reached #{n}");
    v["properties"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == id)
        .unwrap_or_else(|| panic!("missing {id}: {v}"))
}
fn probability(v: &Value, n: usize, min: &str, max: &str, verdict: &str) {
    assert_eq!(
        property(v, n),
        &json!({"id":format!("probability reached #{n}"),"verdict":verdict,"probability":{"min":min,"max":max}}),
        "{v}"
    );
}
fn failure(s: &str, code: &str) {
    let v = check(s);
    assert_eq!(
        v["verdict"],
        if code.starts_with("E-") {
            "invalid"
        } else {
            "unsupported"
        },
        "{s}\n{v}"
    );
    let errors: Vec<_> = v["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["severity"] == "error")
        .collect();
    assert!(!errors.is_empty(), "{v}");
    for d in errors {
        assert_eq!(d["code"], code, "one fault only: {s}\n{v}");
        let a = d["span"]["start"].as_u64().unwrap() as usize;
        let b = d["span"]["end"].as_u64().unwrap() as usize;
        assert!(a <= b && b <= s.len() && s.is_char_boundary(a) && s.is_char_boundary(b));
    }
    assert_eq!(v["properties"], json!([]));
    assert_eq!(v["terminals"], json!([]));
    assert!(v["agency"].is_null());
    for k in ["cases", "live_tasks", "clock", "truncated"] {
        assert!(v["bounds"][k].is_null(), "{v}");
    }
}
fn rows(v: &Value, key: &str, id: &str) -> BTreeMap<String, Value> {
    let a = v[key].as_array().unwrap();
    let m: BTreeMap<_, _> = a
        .iter()
        .map(|r| (r[id].as_str().unwrap().to_owned(), r.clone()))
        .collect();
    assert_eq!(m.len(), a.len());
    m
}
fn hand_report(v: &Value, h: &Value, source: &str, selection: Option<&str>) {
    let cut = h["bounds"]["truncated"] == true;
    assert_eq!(
        ledger_shape::original_field_count(v),
        if cut { 14 } else { 13 }
    );
    assert_eq!(
        v.get("unchecked"),
        cut.then(|| json!(["unreachable"])).as_ref()
    );
    for k in ["verdict", "mode", "unit", "bounds"] {
        assert_eq!(v[k], h[k], "{k}: {v}");
    }
    if h["cut"].is_string() {
        assert_eq!(v["cut"], h["cut"]);
    } else {
        assert_eq!(v["cut"]["name"], h["cut"]["name"]);
    }
    for (k, id) in [("properties", "id"), ("terminals", "end")] {
        assert_eq!(rows(v, k, id), rows(h, k, id), "{k}: {v}");
    }
    let ast = relca_core::syntax::parse(source).unwrap();
    let responsibility =
        if let Ok(model) = relca_core::lower::lower_with_selection(&ast, selection, None) {
            // ADR 0054 "範囲" / "意味": source/selection-bound complete rows,
            // never an unconditional extra code or a runtime-output warning oracle.
            responsibility_oracle::assert_report(&model, v);
            responsibility_oracle::expected(&model)
        } else {
            vec![]
        };
    let mut hand = h["diagnostics"].as_array().unwrap().clone();
    hand.retain(|d| d["code"] != "W-RESP-0");
    hand.extend(responsibility.into_iter().map(|d| json!(d)));
    let mut expected: Vec<_> = hand.iter().map(|d| (&d["code"], &d["severity"])).collect();
    let mut actual: Vec<_> = v["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| (&d["code"], &d["severity"]))
        .collect();
    expected.sort_by_key(|(code, severity)| (code.as_str().unwrap(), severity.as_str().unwrap()));
    actual.sort_by_key(|(code, severity)| (code.as_str().unwrap(), severity.as_str().unwrap()));
    assert_eq!(actual, expected, "exact diagnostics: {v}");
    for c in h["absent_diagnostics"].as_array().into_iter().flatten() {
        assert!(!actual.iter().any(|(code, _)| *code == c), "{v}");
    }
}
fn fixture(name: &str) {
    let d = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(name);
    let s = fs::read_to_string(d.join("model.relca")).unwrap();
    let mut h: Value =
        serde_json::from_str(&fs::read_to_string(d.join("expected.json")).unwrap()).unwrap();
    if name.starts_with("prob-invoice") {
        // ADR0051 hand: the broken input alone replaces the shared, consumed
        // entry binding Clerk.enter -> Extractor.extract. Its probability
        // violation (wrong posting > 0.001) and the three old missing intents stay.
        let producers = if name == "prob-invoice.broken-ai-transfer" {
            vec![
                json!({"kind":"relation","target":"entry","change":"modified","reason":null,"loses":null,
                "from":{"producer":"Clerk.enter"},"to":{"producer":"Extractor.extract"}}),
            ]
        } else {
            vec![]
        };
        h["differences"]
            .as_array_mut()
            .unwrap()
            .extend(producers.clone());
        let v = serde_json::to_value(diff::diff(&s, "AsIs", "ToBeP")).unwrap();
        assert_eq!(v["verdict"], h["verdict"]);
        assert_eq!(v["differences"], h["differences"]);
        assert!(v["mismatch"].is_null());
        for (side, selection) in [("from", "AsIs"), ("to", "ToBeP")] {
            hand_report(&v[side]["report"], &h[side]["report"], &s, Some(selection));
            let single = serde_json::to_value(report::check_configuration(&s, selection)).unwrap();
            assert_eq!(v[side]["report"], single);
        }
        for (key, id) in [("properties", "id"), ("terminals", "end")] {
            assert_eq!(rows(&v, key, id), rows(&h, key, id));
        }
        for c in h["absent_diagnostics"].as_array().unwrap() {
            assert!(
                !v["diagnostics"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|d| d["code"] == *c)
            );
        }
        // ADR 0031: only the enumerated W-DIFF-1 / W-DIFF-3 increments are new;
        // the old diagnostics keep the pre-ADR count and codes.
        let old = diff_increment::old_diagnostics(&v, &producers);
        assert_eq!(old.len(), if name.contains("broken") { 3 } else { 0 });
        for d in &old {
            assert_eq!(d["code"], "W-DIFF-1");
            assert_eq!(d["severity"], "warning");
        }
    } else {
        hand_report(&check(&s), &h, &s, None);
    }
}
macro_rules! fixtures { ($($test:ident => $name:literal),+ $(,)?) => {$(#[test] fn $test(){fixture($name);})+}; }
fixtures! {
 f1_invoice=>"prob-invoice", f1_transfer=>"prob-invoice.broken-ai-transfer",
 f2_mdp=>"prob-mdp", f2_tight=>"prob-mdp.broken-tight-upper",
 f3_cut=>"prob-cut", f3_short=>"prob-cut.broken-short-clock",
 f4_sum=>"prob-sum", f4_mass=>"prob-sum.broken-mass",
 f5_rows=>"prob-rows", f5_missing=>"prob-rows.broken-missing-difficult",
 f6_cancel=>"prob-cancel", f6_ignore=>"prob-cancel.broken-ignore-cancel",
 f7_zero=>"prob-zero", f7_positive=>"prob-zero.broken-positive-write",
 f8_finish=>"prob-finish-read", f8_late=>"prob-finish-read.broken-late-writer",
}

macro_rules! matrix { ($($name:ident: $base:ident, $from:literal => $to:literal, $code:literal;)+) => {$ (
 #[test] fn $name(){ assert_eq!($base.matches($from).count(),1); failure(&$base.replacen($from,$to,1),$code); }
)+}; }
matrix! {
 s_mass_low: SUM,"0.75: ok"=>"0.74: ok","E-PROB-0";
 s_mass_high: SUM,"0.75: ok"=>"0.85: ok","E-PROB-0";
 s_mass_float_trap: SUM,"0.25: ok with hit = true; 0.75: ok"=>"0.999999999999999999: ok with hit = true; 0.000000000000000002: ok","E-PROB-0";
 s_range_branch: SUM,"0.25: ok"=>"1.01: ok","E-PROB-0";
 s_negative_branch: SUM,"0.25: ok"=>"-0.01: ok","E-PROB-0";
 s_range_bound: SUM,"<= 0.25"=>"<= 1.1","E-PROB-0";
 s_negative_bound: SUM,"<= 0.25"=>"<= -0.1","E-PROB-0";
 s_initial_sum: ROWS,"routine: 0.5, difficult: 0.5"=>"routine: 0.5, difficult: 0.4","E-PROB-0";
 s_initial_duplicate: ROWS,"routine: 0.5, difficult: 0.5"=>"routine: 0.5, routine: 0.5","E-PROB-0";
 s_initial_missing: ROWS,"routine: 0.5, difficult: 0.5"=>"routine: 1","E-PROB-0";
 s_initial_range: ROWS,"routine: 0.5"=>"routine: 1.1","E-PROB-0";
 s_initial_type: ROWS,"routine: 0.5"=>"unknown: 0.5","E-TYPE-0";
 s_initial_default: ROWS,"difficulty: Difficulty ~"=>"difficulty: Difficulty = routine ~","U-SEM-0";
 s_initial_optional: ROWS,"difficulty: Difficulty ~"=>"difficulty: optional<Difficulty> ~","U-SEM-0";
 s_row_duplicate: ROWS,"difficult ->"=>"routine ->","E-PROB-0";
    s_row_arity: ROWS,"routine ->"=>"routine, routine ->","E-PROB-0";
    s_no_given_value_row: SUM,"* ->"=>"ok ->","E-PROB-0";
 s_row_type: ROWS,"routine ->"=>"unknown ->","E-TYPE-0";
 s_row_wildcard: ROWS,"routine ->"=>"* ->","E-PROB-0";
 s_reads_missing: ROWS,"kernel_reads latent [Invoice.difficulty]"=>"","E-PROB-0";
    s_reads_unknown: ROWS,"kernel_reads latent [Invoice.difficulty]"=>"kernel_reads latent [Invoice.missing]","E-NAME-0";
    s_reads_unknown_actor: ROWS,"kernel_reads latent [Invoice.difficulty]"=>"kernel_reads latent [Missing.difficulty]","E-NAME-0";
    s_given_unknown: ROWS,"given [Invoice.difficulty]"=>"given [Invoice.missing]","E-NAME-0";
    s_given_unknown_case: ROWS,"given [Invoice.difficulty]"=>"given [missing]","E-NAME-0";
 s_given_duplicate: ROWS,"given [Invoice.difficulty]"=>"given [Invoice.difficulty,Invoice.difficulty]","U-SEM-0";
 s_reads_duplicate: ROWS,"kernel_reads latent [Invoice.difficulty]"=>"kernel_reads latent [Invoice.difficulty,Invoice.difficulty]","U-SEM-0";
 s_reads_observed: SUM,"writes hit"=>"writes hit; kernel_reads latent [hit]","U-SEM-0";
 s_result_conflict: SUM,"outcomes kernel K"=>"outcomes kernel K; result nondet { ok }","E-PROB-0";
 s_kernel_unknown: SUM,"outcomes kernel K"=>"outcomes kernel Missing","E-NAME-0";
 s_result_type_zero: SUM,"0.75: ok"=>"0.75: unknown","E-TYPE-0";
    s_with_type: SUM,"hit = true"=>"hit = ok","E-TYPE-0";
    s_with_none: SUM,"hit = true"=>"hit = none","E-TYPE-0";
 s_with_unknown: SUM,"hit = true"=>"missing = true","E-NAME-0";
 s_with_outside_writes: SUM,"writes hit"=>"","E-NAME-0";
 s_with_duplicate: SUM,"hit = true"=>"hit = true, hit = true","U-SEM-0";
 s_finish_with_conflict: SUM,"writes hit"=>"writes hit; finish { hit = false }","U-SEM-0";
 s_outcomes_twice: SUM,"outcomes kernel K"=>"outcomes kernel K; outcomes kernel K","U-SEM-0";
 s_reads_twice: ROWS,"kernel_reads latent [Invoice.difficulty]"=>"kernel_reads latent [Invoice.difficulty]; kernel_reads latent [Invoice.difficulty]","U-SEM-0";
 s_observed_distribution: SUM,"hit: bool = false"=>"hit: bool ~ { true: 0.5, false: 0.5 }","U-SEM-0";
 s_query_binding: SUM,"reached hit == true"=>"reached s == ok","U-SEM-0";
 s_query_unknown: SUM,"reached hit == true"=>"reached unknown == true","E-NAME-0";
 s_query_type: SUM,"reached hit == true"=>"reached hit == ok","E-TYPE-0";
 s_query_duplicate: SUM,"<= 0.25"=>"<= 0.25; verify probability reached hit == true <= 0.250","U-SEM-0";
 s_empty_kernel: SUM,"* -> { 0.25: ok with hit = true; 0.75: ok with hit = false }"=>"","E-SYN-0";
 s_empty_branches: SUM,"0.25: ok with hit = true; 0.75: ok with hit = false"=>"","E-SYN-0";
 s_empty_distribution: ROWS,"routine: 0.5, difficult: 0.5"=>"","E-SYN-0";
 s_missing_colon: SUM,"0.25: ok"=>"0.25 ok","E-SYN-0";
 s_missing_arrow: SUM,"* ->"=>"*","E-SYN-0";
    s_missing_equals: SUM,"hit = true"=>"hit true","E-SYN-0";
    s_missing_with_value: SUM,"hit = true"=>"hit =","E-SYN-0";
    s_missing_branch_separator: SUM,"hit = true; 0.75"=>"hit = true 0.75","E-SYN-0";
    s_missing_given_bracket: ROWS,"given [Invoice.difficulty]"=>"given Invoice.difficulty]","E-SYN-0";
 s_missing_reached: SUM,"probability reached"=>"probability","E-SYN-0";
 s_less_than: SUM,"<= 0.25"=>"< 0.25","U-SYN-0";
  s_end_query: SUM,"hit == true <="=>"end.bad && hit == true <=","U-SYN-0";
    s_qualified_with: SUM,"hit = true"=>"A.hit = true","U-SYN-0";
    s_qualified_writes: SUM,"writes hit"=>"writes A.hit","U-SYN-0";
    s_deep_given: ROWS,"given [Invoice.difficulty]"=>"given [Invoice.difficulty.level]","U-SYN-0";
 s_expression_with: SUM,"hit = true"=>"hit = executor.id","U-SYN-0";
 s_policy: ROWS,"if hit == true"=>"if Invoice.difficulty == routine","E-POL-LATENT";
}

#[test]
fn p_zero_branches_are_still_statically_typed() {
    let zero = include_str!("../../../fixtures/prob-zero/model.relca");
    failure(
        &zero.replace(
            "0: fail with danger = true",
            "0: unknown with danger = true",
        ),
        "E-TYPE-0",
    );
    failure(
        &zero.replace(
            "0: fail with danger = true",
            "0: fail with danger = unknown",
        ),
        "E-TYPE-0",
    );
    let no_support = ROWS.replace("routine: 0.5, difficult: 0.5", "routine: 1, difficult: 0");
    probability(&check(&no_support), 1, "1/5", "1/5", "satisfied");
    failure(
        &no_support.replace(
            "    difficult -> { 0.6: ok with hit = true; 0.4: ok with hit = false }\n",
            "",
        ),
        "E-PROB-0",
    );
}

#[test]
fn p_kernel_duplicate_forward_unused_and_each_use_type_checks() {
    let k = "kernel K { * -> { 0.25: ok with hit = true; 0.75: ok with hit = false } }";
    failure(&SUM.replace(k, &format!("{k}; {k}")), "U-SEM-0");
    // Forward declaration and uncalled capabilities do not evade row validation.
    let forward = SUM
        .replace(k, "")
        .replace("bounds {", &format!("{k}; bounds {{"));
    probability(&check(&forward), 1, "1/4", "1/4", "satisfied");
    let bad = SUM.replace("0.75: ok", "0.74: ok").replace(
        "s = A.sample(); if hit == true { end bad } else { end good }",
        "end done",
    );
    failure(&bad, "E-PROB-0");
    let shared=SUM.replace("process Flow","type Other = enum { ok }; actor B { capacity 1; capability run() -> Other { duration fixed 1min; outcomes kernel K; writes hit } }; process Flow");
    probability(&check(&shared), 1, "1/4", "1/4", "satisfied");
    failure(
        &shared.replace("Other = enum { ok }", "Other = enum { different }"),
        "E-TYPE-0",
    );
}

#[test]
fn p_analysis_names_short_circuit_alias_and_latent_writer_conflict() {
    let initial = empty(
        "latent case { truth: bool = false }",
        "truth == false || missing == true",
    );
    failure(&initial, "E-NAME-0");
    let alias = ROWS
        .replace(
            "observed case { hit: bool = false }",
            "observed case { hit: bool = false }; latent case { s: bool = false }",
        )
        .replace("reached hit == true", "reached s == false");
    failure(&alias, "U-SEM-0");
    // Same-name observed/result alias requires every override to equal result.
    let alias = SUM
        .replace("hit: bool = false", "hit: T = ok")
        .replace("hit = true", "hit = ok")
        .replace("hit = false", "hit = ok")
        .replace("s = A.sample()", "hit = A.sample()")
        .replace("hit == true", "hit == ok")
        .replace("<= 0.25", "<= 1");
    probability(&check(&alias), 1, "1", "1", "satisfied");
    let broken = alias.replace("enum { ok }", "enum { ok, other }").replacen(
        "hit = ok; 0.75",
        "hit = other; 0.75",
        1,
    );
    failure(&broken, "U-SEM-0");
    let latent = SUM
        .replace(
            "observed case { hit: bool = false }",
            "latent case { hit: bool = false }",
        )
        .replace("if hit == true { end bad } else { end good }", "end done");
    let parallel = latent.replace(
        "s = A.sample()",
        "fork f { x = A.sample(); y = A.sample() }; join all f",
    );
    failure(&parallel, "U-SEM-0");
}

#[test]
fn p_exact_ten_branches_and_64_digit_boundary() {
    let ten = (0..10)
        .map(|_| "0.1: ok with hit = true")
        .collect::<Vec<_>>()
        .join("; ");
    let s = SUM
        .replace("0.25: ok with hit = true; 0.75: ok with hit = false", &ten)
        .replace("<= 0.25", "<= 1");
    probability(&check(&s), 1, "1", "1", "satisfied");
    let p = format!("0.{}1", "0".repeat(62)); // 0 + 63 fractional digits = 64.
    let q = format!("0.{}", "9".repeat(63));
    let s = SUM
        .replace(
            "0.25: ok with hit = true; 0.75: ok with hit = false",
            &format!("{p}: ok with hit = true; {q}: ok with hit = false"),
        )
        .replace("<= 0.25", "<= 1");
    let exact = format!("1/1{}", "0".repeat(63));
    probability(&check(&s), 1, &exact, &exact, "satisfied");
}

#[test]
fn p_assumptions_state_declaration_order_and_analyst_independence() {
    let s = empty(
        "latent case { x: T ~ { b: 0.6, a: 0.4 }; y: bool }; actor D { latent z: T ~ { a: 1, b: 0 } }",
        "x == a",
    );
    let v = check(&s);
    let a = v["assumptions"].as_array().unwrap();
    let rows: Vec<_> = a
        .iter()
        .filter(|r| ["latent.x", "latent.y", "D.z"].contains(&r["field"].as_str().unwrap()))
        .collect();
    assert_eq!(
        rows.iter()
            .map(|r| r["field"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["latent.x", "latent.y", "D.z"]
    );
    assert_eq!(rows[0]["value"], "dist(a=2/5, b=3/5)");
    assert_eq!(rows[1]["value"], "nondet(全値)");
    assert_eq!(rows[2]["value"], "dist(a=1, b=0)");
    for r in [rows[0], rows[2]] {
        assert_eq!(
            r["affects"],
            "initial value is drawn from the declared distribution, independently of other initial distributions; the distribution is an analyst assumption"
        );
    }
}

#[test]
fn p_execution_skeletons_are_legacy_syntax_controls_not_probability_evidence() {
    // Drop only the new declarations and replace kernels with their declared
    // result support. This validates authored calls/joins/bounds and arithmetic
    // against already-supported syntax without using execution to set oracles.
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    for (name, selection, e, w) in [
        ("prob-invoice", Some("AsIs"), 17, 17),
        ("prob-invoice", Some("ToBeP"), 14, 17),
        ("prob-invoice.broken-ai-transfer", Some("ToBeP"), 9, 12),
    ] {
        let source = fs::read_to_string(root.join(name).join("model.relca")).unwrap();
        let mut depth = 0i32;
        let mut kept = Vec::new();
        for line in source.lines() {
            if depth == 0 && line.trim_start().starts_with("kernel ") {
                depth = line.chars().filter(|c| *c == '{').count() as i32
                    - line.chars().filter(|c| *c == '}').count() as i32;
                continue;
            }
            if depth > 0 {
                depth += line.chars().filter(|c| *c == '{').count() as i32
                    - line.chars().filter(|c| *c == '}').count() as i32;
                continue;
            }
            if !line.trim_start().starts_with("verify probability") {
                kept.push(line);
            }
        }
        assert_eq!(depth, 0);
        let mut s = kept.join("\n");
        for (from, to) in [
            (
                "difficulty: Difficulty ~ { routine: 0.8, difficult: 0.2 }",
                "difficulty: Difficulty = routine",
            ),
            ("outcomes kernel human_entry", "result nondet { high }"),
            ("outcomes kernel ai_entry", "result nondet { high }"),
            ("outcomes kernel po_match", "result nondet { pass, fail }"),
            (
                "outcomes kernel receipt_match",
                "result nondet { pass, fail }",
            ),
            (
                "outcomes kernel approval_check",
                "result nondet { pass, fail }",
            ),
            ("; kernel_reads latent [Invoice.difficulty]", ""),
            (
                "; kernel_reads latent [extraction_correct,Invoice.difficulty]",
                "",
            ),
            ("writes [entry,extraction_correct]", "writes entry"),
        ] {
            s = s.replace(from, to);
        }
        let v = serde_json::to_value(report::check_configuration(&s, selection.unwrap())).unwrap();
        assert_eq!(v["verdict"], "satisfied", "{s}\n{v}");
        let t = v["terminals"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["end"] == "posted")
            .unwrap();
        assert_eq!(t["elapsed"], json!({"min":e,"max":e}));
        assert_eq!(t["work"], json!({"min":w,"max":w}));
    }
}

fn empty(items: &str, condition: &str) -> String {
    format!(
        r#"model Initial version "0.2" {{ type T = enum {{ a,b }} {items}
 process Flow {{ end done }} bounds {{ cases 1; live_tasks 2; clock 20min }}
 verify probability reached {condition} <= 1 }}"#
    )
}
#[test]
fn p_initial_product() {
    let s = empty(
        "latent case { x: bool ~ { true: 0.25, false: 0.75 } }; actor D { latent y: T ~ { a: 0.4, b: 0.6 } }",
        "x == true && D.y == a",
    );
    for (x, y, p) in [
        ("true", "a", "1/10"),
        ("true", "b", "3/20"),
        ("false", "a", "3/10"),
        ("false", "b", "9/20"),
    ] {
        let v = check(&s.replace("x == true && D.y == a", &format!("x == {x} && D.y == {y}")));
        probability(&v, 1, p, p, "satisfied");
        assert_eq!(
            v["terminals"][0]["probability"],
            json!({"min":"1","max":"1"})
        );
    }
    let s = empty(
        "latent case { x,y: T ~ { a: 0.5, b: 0.5 } }",
        "x == a && y == b",
    );
    probability(&check(&s), 1, "1/4", "1/4", "satisfied");
}
#[test]
fn p_initial_order() {
    let v = check(&empty(
        "latent case { x: bool ~ { true: 0.5, false: 0.5 }; y: bool }",
        "(x == true && y == true) || (x == false && y == false)",
    ));
    probability(&v, 1, "0", "1", "satisfied");
    assert!(
        v["assumptions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a["field"] == "latent.y" && a["value"] == "nondet(全値)")
    );
    scheduler(&v);
}
fn scheduler(v: &Value) {
    let a: Vec<_> = v["assumptions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|a| a["field"] == "probability.scheduler")
        .collect();
    assert_eq!(a.len(), 1, "{v}");
    assert_eq!(a[0]["value"], "full-information");
    assert_eq!(
        a[0]["affects"],
        "min/max は完全情報の緩和境界で、実現できる方策の値ではない"
    );
    assert_eq!(v["assumptions"].as_array().unwrap().last().unwrap(), a[0]);
}
#[test]
fn p_thresholds_mdp() {
    for (op, b, v) in [
        ("<=", "0.75", "satisfied"),
        ("<=", "0.25", "violated"),
        (">=", "0.25", "satisfied"),
        (">=", "0.5", "violated"),
    ] {
        let a = check(&MDP.replace("<= 0.75", &format!("{op} {b}")));
        probability(&a, 1, "1/4", "3/4", v);
        scheduler(&a);
    }
}
#[test]
fn p_four_cut_endpoints_and_seen_before_cut() {
    for (op, b, v) in [
        ("<=", "0.75", "inconclusive"),
        ("<=", "0.25", "violated"),
        (">=", "0.5", "satisfied"),
        ("<=", "1", "satisfied"),
    ] {
        let a = check(&CUT.replace(">= 0.75", &format!("{op} {b}")));
        probability(&a, 1, "1/2", "1", v);
        assert_eq!(a["bounds"]["truncated"], true);
        assert_eq!(
            a["verdict"],
            if v == "violated" {
                "violated"
            } else {
                "inconclusive"
            }
        );
    }
    let a = check(&CUT.replace("seen == true >= 0.75", "route == long <= 0.5"));
    probability(&a, 1, "1/2", "1/2", "satisfied");
}
#[test]
fn p_merge_and_transient_reached() {
    let s = SUM
        .replace(
            "0.25: ok with hit = true; 0.75: ok with hit = false",
            "0.2: ok with hit = true; 0.3: ok with hit = true; 0.5: ok with hit = false",
        )
        .replace("<= 0.25", "<= 0.5");
    probability(&check(&s), 1, "1/2", "1/2", "satisfied");
    let reset=s.replace("process Flow", "actor Reset { capacity 1; capability clear() -> T { duration fixed 1min; result nondet { ok }; writes hit; finish { hit = false } } }; process Flow")
  .replace("if hit == true { end bad } else { end good }","r = Reset.clear(); end done");
    let v = check(&reset);
    probability(&v, 1, "1/2", "1/2", "satisfied");
    assert_eq!(
        v["terminals"],
        json!([{"end":"done","elapsed":{"min":2,"max":2},"work":{"min":2,"max":2},"result_at":null,"probability":{"min":"1","max":"1"}}])
    );
}
#[test]
fn p_finish_order_and_conditional_product() {
    let tied = FINISH.replace("fixed 1min", "fixed 2min");
    probability(&check(&tied), 1, "0", "1/2", "violated");
    let conditional=tied.replace("result nondet { ok }; writes flag; finish { flag = true }","outcomes kernel W; writes flag")
  .replace("kernel K given", "kernel W { * -> { 0.5: ok with flag = true; 0.5: ok with flag = false } }; kernel K given");
    // Writer first: (1/2)(1/2)+(1/2)(0)=1/4; Sampler first:0.
    probability(&check(&conditional), 1, "0", "1/4", "violated");
}
#[test]
fn p_action_independent_product_no_contract() {
    let s=FINISH.replace("result nondet { ok }; writes flag; finish { flag = true }","outcomes kernel W; writes flag")
  .replace("kernel K given [flag] {\n    false -> { 0: ok with hit = true; 1: ok with hit = false }\n    true -> { 0.5: ok with hit = true; 0.5: ok with hit = false }\n  }", "kernel W { * -> { 0.5: ok with flag = true; 0.5: ok with flag = false } }; kernel K { * -> { 0.25: ok with hit = true; 0.75: ok with hit = false } }")
  .replace("fixed 1min","fixed 2min").replace("hit == true >= 0.5","flag == true && hit == true <= 0.125");
    probability(&check(&s), 1, "1/8", "1/8", "satisfied");
}
#[test]
fn p_cancel_ack_tie() {
    let a = check(&CANCEL.replace("fixed 5min", "fixed 2min"));
    probability(&a, 1, "0", "1/2", "violated");
    let b = check(
        &CANCEL
            .replace("fixed 5min", "fixed 2min")
            .replace("<= 0", ">= 0"),
    );
    probability(&b, 1, "0", "1/2", "satisfied");
}
#[test]
fn p_nonprob_query_and_initial_zero() {
    for (decl, min, max) in [
        ("latent case { x: bool = false }", "0", "0"),
        ("latent case { x: bool = true }", "1", "1"),
        ("latent case { x: bool }", "0", "1"),
        ("latent case { x: bool ~ { true: 0, false: 1 } }", "0", "0"),
    ] {
        probability(&check(&empty(decl, "x == true")), 1, min, max, "satisfied");
    }
}
#[test]
fn p_exact_assumptions() {
    let v = check(ROWS);
    let a = v["assumptions"].as_array().unwrap();
    let relevant: Vec<_> = a
        .iter()
        .filter(|a| {
            a["field"] == "Invoice.difficulty"
                || a["field"].as_str().unwrap().starts_with("kernel.")
        })
        .cloned()
        .collect();
    assert_eq!(relevant,json!([
  {"field":"Invoice.difficulty","value":"dist(routine=1/2, difficult=1/2)","affects":"initial value is drawn from the declared distribution, independently of other initial distributions; the distribution is an analyst assumption"},
  {"field":"kernel.K","value":"given[Invoice.difficulty]","affects":"outcomes and writes are drawn from the declared rows; separate draws are conditionally independent given the listed variables; the rows are an analyst assumption"}
 ]).as_array().unwrap().clone());
    assert!(
        !a.iter()
            .any(|a| a["field"] == "Invoice.difficulty" && a["value"] == "nondet(全値)")
    );
    let v = check(&SUM.replace(
        "process Flow",
        "kernel Unused { * -> { 1: untyped } }; process Flow",
    ));
    let kernels: Vec<_> = v["assumptions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|a| a["field"].as_str().unwrap().starts_with("kernel."))
        .collect();
    assert_eq!(
        kernels
            .iter()
            .map(|a| a["field"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["kernel.K", "kernel.Unused"]
    );
    for a in kernels {
        assert_eq!(a["value"], "given[]");
    }
}
#[test]
fn p_agency_kernel_union_not_observation_or_suppressed_guard() {
    let old = include_str!("../../../fixtures/state-agency/model.relca");
    let kernel=old.replace("result nondet { pass, fail }","outcomes kernel K; kernel_reads latent [Hidden.truth]")
  .replace("process Flow", "actor Hidden { source document; latent truth: bool = true }; kernel K given [Hidden.truth] { true -> { 0.5: pass; 0.5: fail }; false -> { 0.5: pass; 0.5: fail } }; process Flow");
    let a = check(old);
    let b = check(&kernel);
    assert_eq!(b["verdict"], "satisfied");
    for k in ["rules", "lineage", "lineage_groups", "refusal"] {
        assert_eq!(a["agency"][k], b["agency"][k], "{k}: {b}");
    }
    for origin in a["agency"]["origin"].as_array().unwrap() {
        assert!(b["agency"]["origin"].as_array().unwrap().contains(origin));
    }
    assert_eq!(
        b["agency"]["origin"]
            .as_array()
            .unwrap()
            .iter()
            .find(|o| o["actor"] == "Hidden")
            .unwrap(),
        &json!({"actor":"Hidden","direct":[],"indirect":[]})
    );
    let mut expected = a["agency"]["unchecked"].as_array().unwrap().clone();
    // ADR 0043 (plan C04): the no-people marker stays last in both reports.
    let people = (expected.last() == Some(&json!("responsibility_people_undeclared")))
        .then(|| expected.pop().unwrap());
    expected.push(json!("kernel_correlations"));
    expected.extend(people);
    assert_eq!(b["agency"]["unchecked"], json!(expected));
    failure(
        &kernel.replace("kernel_reads latent [Hidden.truth]", ""),
        "E-PROB-0",
    );
}
#[test]
fn p_legacy_wire_has_no_probability_and_optional_new_keys_are_local() {
    for source in [
        include_str!("../../../fixtures/serial-time/model.relca"),
        include_str!("../../../fixtures/state-invoice-write/model.relca"),
    ] {
        let v = check(source);
        assert_eq!(ledger_shape::original_field_count(&v), 13);
        for key in ["properties", "terminals"] {
            for row in v[key].as_array().unwrap() {
                assert!(row.get("probability").is_none());
            }
        }
    }
    let v = check(SUM);
    assert_eq!(ledger_shape::original_field_count(&v), 13);
    assert_eq!(
        v["properties"][0],
        json!({"id":"safety no_resource_overcommit","verdict":"satisfied"})
    );
    assert_eq!(property(&v, 1).as_object().unwrap().len(), 3);
    for t in v["terminals"].as_array().unwrap() {
        assert_eq!(t.as_object().unwrap().len(), 5);
    }
}
#[test]
fn p_lexical_all_probability_positions() {
    let positions = [
        (
            "branch",
            SUM,
            "0.25: ok with hit = true; 0.75: ok with hit = false",
        ),
        ("initial", ROWS, "routine: 0.5, difficult: 0.5"),
        ("bound", SUM, "<= 0.25"),
    ];
    for (kind, base, site) in positions {
        for (literal, code) in [
            ("1e-3", "U-SYN-0"),
            ("1/2", "U-SYN-0"),
            ("+0.5", "U-SYN-0"),
            ("-0.1", "E-PROB-0"),
            ("1.1", "E-PROB-0"),
            (".5", "E-SYN-0"),
            ("1.", "E-SYN-0"),
        ] {
            let replacement = match kind {
                "branch" => format!("{literal}: ok with hit = true; 0: ok with hit = false"),
                "initial" => format!("routine: {literal}, difficult: 0"),
                _ => format!("<= {literal}"),
            };
            failure(&base.replace(site, &replacement), code);
        }
        let too_long = format!("0.{}1", "0".repeat(63));
        let replacement = match kind {
            "branch" => format!("{too_long}: ok with hit = true; 1: ok with hit = false"),
            "initial" => format!("routine: {too_long}, difficult: 1"),
            _ => format!("<= {too_long}"),
        };
        failure(&base.replace(site, &replacement), "U-SYN-0");
    }
    for (a, b, p) in [
        ("0", "1", "0"),
        ("1.000", "0", "1"),
        ("0.10", "0.90", "1/10"),
    ] {
        let s = SUM
            .replace(
                "0.25: ok with hit = true; 0.75: ok with hit = false",
                &format!("{a}: ok with hit = true; {b}: ok with hit = false"),
            )
            .replace("<= 0.25", "<= 1.000");
        probability(&check(&s), 1, p, p, "satisfied");
    }
}
#[test]
fn p_contextual_words_and_unsupported_positions() {
    let legacy = SUM
        .replace(
            "kernel K { * -> { 0.25: ok with hit = true; 0.75: ok with hit = false } }",
            "",
        )
        .replace("outcomes kernel K; writes hit", "result nondet { ok }")
        .replace("verify probability reached hit == true <= 0.25", "");
    for word in [
        "kernel",
        "given",
        "with",
        "outcomes",
        "kernel_reads",
        "probability",
        "reached",
    ] {
        for s in [
            legacy
                .replace("actor A", &format!("actor {word}"))
                .replace("A.sample", &format!("{word}.sample")),
            legacy.replace("s =", &format!("{word} =")),
            legacy.replace("ok", word),
        ] {
            assert_eq!(check(&s)["verdict"], "satisfied", "{s}");
        }
    }
    failure(&legacy.replace("s =", "by ="), "E-SYN-0");
    for text in ["kernel K {}", "outcomes {}"] {
        failure(&legacy.replace("result nondet { ok }", text), "U-SYN-0");
    }
    failure(&legacy.replace("fixed 1min", "fixed 1.5min"), "U-SYN-0");
    failure(&legacy.replace("capacity 1", "capacity 1.5"), "E-SYN-0");
    for symbol in ["<", ">", "<=", ">=", "~", "*"] {
        failure(
            &legacy.replace(
                "bounds {",
                &format!("cut C {{ include [A]; reentry [review when x {symbol} 3] }}; bounds {{"),
            ),
            "E-SYN-0",
        );
    }
}
#[test]
fn p_boundary_unused_kernel_static_checks_and_given_empty() {
    assert_eq!(
        check(&SUM.replace("kernel K {", "kernel K given [] {"))["verdict"],
        "satisfied"
    );
    let unused = SUM.replace(
        "process Flow",
        "kernel Unused { * -> { 0.9: untyped } }; process Flow",
    );
    failure(&unused, "E-PROB-0");
    let optional = SUM
        .replace("hit: bool = false", "hit: bool = false; v: optional<T>")
        .replace("kernel K { * ->", "kernel K given [v] { ok ->");
    failure(&optional, "E-PROB-0");
    let unused_only = empty(
        "kernel Unused { * -> { 1: untyped } }; latent case { x: bool = false }",
        "x == true",
    )
    .replace(
        "verify probability reached x == true <= 1",
        "verify safety no_resource_overcommit",
    );
    let v = check(&unused_only);
    assert_eq!(v["verdict"], "satisfied");
    assert_eq!(
        v["terminals"][0]["probability"],
        json!({"min":"1","max":"1"})
    );
    assert!(v["properties"][0].get("probability").is_none());
    assert!(
        !v["agency"]["unchecked"]
            .as_array()
            .unwrap()
            .contains(&json!("kernel_correlations"))
    );
}
#[test]
fn p_latent_writes_all_branches_and_policy_global_cut() {
    let s = SUM
        .replace(
            "observed case { hit: bool = false }",
            "latent case { hit: bool = false }",
        )
        .replace("if hit == true { end bad } else { end good }", "end done");
    probability(&check(&s), 1, "1/4", "1/4", "satisfied");
    failure(
        &s.replace("; 0.75: ok with hit = false", "; 0.75: ok"),
        "E-PROB-0",
    );
    failure(
        &s.replace("outcomes kernel K", "result nondet { ok }"),
        "U-SYN-0",
    );
    failure(
        &s.replace("writes hit", "writes hit; finish { hit = false }"),
        "U-SYN-0",
    );
    let cut = ROWS.replace(
        "bounds {",
        "cut C { include [A]; exclude Invoice reason \"external\" loses \"difficulty\" }; bounds {",
    );
    failure(&cut, "E-CUT-1");
    let query = empty(
        "actor A {}; actor D { latent truth: bool = true }; cut C { include [A]; exclude D reason \"external\" loses \"truth\" }",
        "D.truth == true",
    );
    failure(&query, "E-CUT-1");
    let unchosen=s.replace("bounds {","process Other { if hit == false {} else { if hit == true {} else {} }; end done }; cut C { include [A] }; configuration Chosen { cut C; process Flow }; configuration OtherChoice { cut C; process Other }; bounds {");
    let v = serde_json::to_value(report::check_configuration(&unchosen, "Chosen")).unwrap();
    assert_eq!(v["verdict"], "invalid");
    assert!(
        v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "E-POL-LATENT")
    );
}
#[test]
fn p_source_pairs_are_single_declared_faults() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    for (base, fault, from, to) in [
        (
            "prob-invoice",
            "ai-transfer",
            "process ToBePFlow {\n    entry = Clerk.enter()",
            "process ToBePFlow {\n    entry = Extractor.extract()",
        ),
        ("prob-mdp", "tight-upper", "<= 0.75", "<= 0.5"),
        ("prob-cut", "short-clock", "clock 4min", "clock 2min"),
        ("prob-sum", "mass", "0.75: ok", "0.74: ok"),
        (
            "prob-rows",
            "missing-difficult",
            "    difficult -> { 0.6: ok with hit = true; 0.4: ok with hit = false }\n",
            "",
        ),
        (
            "prob-cancel",
            "ignore-cancel",
            "cancel cooperative ack_within 1min",
            "cancel forbidden_after_start",
        ),
        (
            "prob-zero",
            "positive-write",
            "1: pass with danger = false",
            "1: pass with danger = true",
        ),
        (
            "prob-finish-read",
            "late-writer",
            "fixed 1min",
            "fixed 3min",
        ),
    ] {
        let before = fs::read_to_string(root.join(base).join("model.relca")).unwrap();
        assert_eq!(before.matches(from).count(), 1);
        let after = fs::read_to_string(
            root.join(format!("{base}.broken-{fault}"))
                .join("model.relca"),
        )
        .unwrap();
        assert_eq!(before.replace(from, to), after);
    }
}

// Private graph/Room/numeric observers and census obligations are explicitly
// assigned to implementation-side acceptance; no private hooks here.
