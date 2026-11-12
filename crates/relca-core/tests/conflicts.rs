//! ADR0041 independent acceptance. Stage C intentionally RED on old semantics.
#[path = "support/actor_observed_acceptance.rs"]
mod h;
use relca_core::{compare, engine, explain, reference, report};
use serde_json::{Value, json};

fn names() -> Vec<String> {
    serde_json::from_str(include_str!("support/conflict_names.json")).unwrap()
}
fn check(name: &str) -> Value {
    let s = h::source(name);
    let path = h::root()
        .join("fixtures")
        .join(name)
        .join("invocation.json");
    let inv: Value = if path.exists() {
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
    } else {
        Value::Null
    };
    match inv["command"].as_str() {
        Some("simulate") => json!(report::simulate(&s)),
        Some("explain") => json!(explain::explain(&s)),
        Some("compare") => json!(compare::compare(
            &s,
            &["Left", "Right"],
            if inv["mode"] == "simulation" {
                compare::Analysis::Simulation
            } else {
                compare::Analysis::Exhaustive
            }
        )),
        _ => json!(report::check(&s)),
    }
}
fn property<'a>(v: &'a Value, id: &str) -> &'a Value {
    v["properties"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == id)
        .unwrap_or_else(|| panic!("missing property {id}: {v}"))
}
fn accept(base: &str) {
    let selected: Vec<_> = names()
        .into_iter()
        .filter(|n| n == base || n.starts_with(&format!("{base}.broken-")))
        .collect();
    assert_eq!(selected.len(), 2);
    for name in selected {
        let expected = h::expected(&name);
        h::subset(&expected, &check(&name));
        if name.starts_with("cf-compare-") {
            continue;
        }
        if name.starts_with("cf-simulation") || name == "cf-same-tick.broken-simulate" {
            let m = h::model(&h::source(&name));
            let e = engine::simulate(&m);
            let r = reference::simulate(&m);
            assert_eq!(e, r.outcome, "independent simulation {name}");
            assert_eq!(
                e.completed_replications,
                if expected["verdict"] == "unsupported" {
                    0
                } else {
                    10
                }
            );
            continue;
        }
        if ["unsupported", "invalid"].contains(&expected["verdict"].as_str().unwrap()) {
            if name != "cf-same-tick.broken-simulate" {
                let ds = relca_core::syntax::parse(&h::source(&name))
                    .and_then(|a| relca_core::lower::lower(&a))
                    .expect_err("static failure, not runtime abort");
                let actual: std::collections::BTreeSet<_> = ds
                    .iter()
                    .filter(|d| d.severity == relca_core::diag::Severity::Error)
                    .map(|d| d.code.as_str())
                    .collect();
                let error_count = ds
                    .iter()
                    .filter(|d| d.severity == relca_core::diag::Severity::Error)
                    .count();
                let expected_count = if name == "cf-one-sided.broken-one-sided" {
                    2
                } else {
                    expected["diagnostics"].as_array().unwrap().len()
                };
                assert_eq!(
                    error_count, expected_count,
                    "all and only prescribed diagnostics: {name}"
                );
                let wanted: std::collections::BTreeSet<_> = expected["diagnostics"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|d| d["code"].as_str().unwrap())
                    .collect();
                assert_eq!(
                    actual, wanted,
                    "all and only prescribed error classes: {name}"
                );
            }
            continue;
        }
        let model = h::model(&h::source(&name));
        let e = engine::explore(&model);
        let r = reference::explore(&model);
        assert_eq!(
            r.stop,
            reference::StopReason::Complete,
            "no runtime exclusions: {name}"
        );
        assert_eq!(
            json!(r.verdict()),
            expected["verdict"],
            "reference oracle {name}"
        );
        for o in [&e, &r.outcome] {
            assert_eq!(
                o.truncated,
                expected["bounds"]["truncated"].as_bool().unwrap()
            );
            let p = h::projection(&model, o);
            for key in ["terminals", "case_outcomes"] {
                if let Some(v) = expected.get(key) {
                    h::subset(v, &p[key]);
                }
            }
            if name != "cf-race.broken-two-writers" {
                assert!(
                    o.state_range_violations.is_empty(),
                    "version rejection before range preflight: {name}"
                );
            }
        }
        // The engine deliberately does NOT prepend lowering diagnostics.
        assert!(r.outcome.diagnostics.starts_with(&model.diagnostics));
        assert_eq!(
            e.diagnostics,
            r.outcome.diagnostics[model.diagnostics.len()..]
        );
        assert_eq!(e.properties, r.outcome.properties, "{name}");
        assert_eq!(e.terminals, r.outcome.terminals, "{name}");
        assert_eq!(e.case_outcomes, r.outcome.case_outcomes, "{name}");
        assert_eq!(
            e.state_range_violations, r.outcome.state_range_violations,
            "{name}"
        );
        assert_eq!(
            e.capability_refusals, r.outcome.capability_refusals,
            "{name}"
        );
    }
}
macro_rules! acceptance { ($($test:ident => $base:literal),+ $(,)?) => {$(
    #[test] fn $test() { accept($base); }
)+}; }
include!("support/conflict_pair_tests.rs");

#[test]
fn closed_population_real_edits_and_no_comments() {
    let mut actual: Vec<_> = std::fs::read_dir(h::root().join("fixtures"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("cf-"))
        .collect();
    actual.sort();
    assert_eq!(actual, names());
    let pairs: Vec<Value> =
        serde_json::from_str(include_str!("support/conflict_pairs.json")).unwrap();
    assert_eq!(actual.len(), 2 * pairs.len());
    for pair in pairs {
        let s = h::source(pair["good"].as_str().unwrap());
        let t = h::change(
            &s,
            pair["old"].as_str().unwrap(),
            pair["new"].as_str().unwrap(),
        );
        assert_eq!(t, h::source(pair["bad"].as_str().unwrap()));
    }
    for name in actual {
        let s = h::source(&name);
        assert!(!s.contains("//") && !s.contains('#'));
    }
}

#[test]
fn exactly_three_existing_static_fixtures_migrate_to_hand_runtime_oracles() {
    use sha2::{Digest, Sha256};
    let rows: Vec<Value> =
        serde_json::from_str(include_str!("support/conflict_migrations.json")).unwrap();
    assert_eq!(
        rows.iter()
            .map(|r| r["name"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec![
            "aobs-capacity.broken-two-writers",
            "aset-cases.broken-capacity",
            "aset-static-capacity-with-resource.broken-capacity-with-resource"
        ]
    );
    for row in rows {
        let name = row["name"].as_str().unwrap();
        let s = h::source(name);
        use std::fmt::Write;
        let mut digest = String::new();
        for byte in Sha256::digest(s.as_bytes()) {
            write!(digest, "{byte:02x}").unwrap();
        }
        assert_eq!(digest, row["source_sha256"]);
        assert_eq!(row["original_expected"]["verdict"], "unsupported");
        assert_eq!(h::expected(name), row["replacement_expected"]);
        h::subset(&row["replacement_expected"], &json!(report::check(&s)));
    }
}

#[test]
fn old_grammar_substrates_are_source_bound_and_accepted() {
    let rows: Vec<Value> =
        serde_json::from_str(include_str!("support/conflict_legacy_controls.json")).unwrap();
    let mut failures = vec![];
    for row in rows {
        let name = row["name"].as_str().unwrap();
        assert_eq!(h::source(name), row["original"].as_str().unwrap());
        let s = row["legacy_control"].as_str().unwrap();
        if let Err(d) = relca_core::syntax::parse(s).and_then(|a| {
            if let Some(c) = row["configuration"].as_str() {
                relca_core::lower::lower_configuration(&a, c).map(|_| ())
            } else {
                relca_core::lower::lower(&a).map(|_| ())
            }
        }) {
            failures.push(format!("{name}: {d:?}\n{s}"));
        }
    }
    assert!(
        failures.is_empty(),
        "unrelated substrate defects:\n{}",
        failures.join("\n")
    );
}

#[test]
fn explicit_concurrency_assumptions_include_scope_and_value_only_commutativity() {
    let rows: Vec<Value> =
        serde_json::from_str(include_str!("support/conflict_oracles.json")).unwrap();
    for row in rows {
        let Some(expected) = row["assumptions"].as_object() else {
            continue;
        };
        let name = row["name"].as_str().unwrap();
        let v = check(name);
        let assumptions = v["assumptions"].as_array().unwrap();
        let fields: Vec<_> = assumptions
            .iter()
            .filter_map(|a| a["field"].as_str())
            .filter(|f| f.ends_with(".concurrency"))
            .collect();
        let declaration_order: Vec<_> = h::model(&h::source(name))
            .state_variables
            .iter()
            .filter_map(|state| {
                expected
                    .keys()
                    .find(|field| field.ends_with(&format!(".{}.concurrency", state.name)))
                    .cloned()
            })
            .collect();
        assert_eq!(
            fields, declaration_order,
            "declaration order and closed concurrency field set: {name}"
        );
        let last_instance = assumptions
            .iter()
            .rposition(|a| a["field"].as_str().unwrap().ends_with(".instance"));
        for (field, value) in expected {
            let (index, a) = v["assumptions"]
                .as_array()
                .unwrap()
                .iter()
                .enumerate()
                .find(|(_, a)| a["field"] == *field)
                .unwrap_or_else(|| panic!("missing assumption {field}: {v}"));
            assert_eq!(&a["value"], value);
            let text = a["affects"].as_str().unwrap();
            for fragment in ["全順序", "古い値", "列挙しない", "値", "適格性", "権限"]
            {
                assert!(text.contains(fragment), "{name}: {text}");
            }
            if value.as_str().unwrap().starts_with("lock ") {
                assert!(text.contains("Actor") && text.contains("資源"));
            }
            if ["cf-serial", "cf-silent", "cf-lww"].contains(&name) {
                assert!(text.contains("案件をまたぐ競合には作用しない"));
            }
            assert!(
                last_instance.is_none_or(|i| i < index),
                "concurrency follows instance defaults"
            );
            assert!(
                assumptions[..index].iter().all(|a| {
                    let f = a["field"].as_str().unwrap();
                    !(f == "arrivals"
                        || f.starts_with("arrivals.")
                        || f == "policy"
                        || f.starts_with("policy."))
                }),
                "concurrency precedes arrival/policy additions"
            );
        }
    }
    let v = check("cf-silent.broken-one-case");
    assert!(v["assumptions"].as_array().unwrap().iter().any(|a| {
        a["field"] == "Ledger.holder.concurrency"
            && a["affects"]
                .as_str()
                .unwrap()
                .contains("案件をまたぐ競合には作用しない")
    }));
    let v = check("cf-lww-commutative");
    let a = v["assumptions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["field"] == "Ledger.n.concurrency")
        .unwrap();
    assert_eq!(a["value"], "commutative");
    assert!(a["affects"].as_str().unwrap().contains("last_write_wins"));
    let s = h::change(
        &h::source("cf-lock"),
        "      commits",
        "      version_check [holder] else conflict\n      commits",
    );
    let v = json!(report::check(&s));
    let a = v["assumptions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["field"] == "Ledger.holder.concurrency")
        .unwrap();
    assert_eq!(a["value"], "lock ledger_lock");
    assert!(a["affects"].as_str().unwrap().contains("version_check"));
    let legacy = check("cf-serial.broken-no-check");
    assert!(
        !legacy["assumptions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a["field"].as_str().unwrap().ends_with(".concurrency"))
    );
}

#[test]
fn unreachable_warnings_are_not_suppressed_or_invented_on_clock_cuts() {
    let rows: Vec<Value> =
        serde_json::from_str(include_str!("support/conflict_oracles.json")).unwrap();
    for row in rows {
        let Some(codes) = row["warnings"].as_array() else {
            continue;
        };
        let v = check(row["name"].as_str().unwrap());
        let actual: Vec<_> = v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|d| d["code"].as_str().unwrap().starts_with("W-UNREACH-"))
            .map(|d| d["code"].clone())
            .collect();
        assert_eq!(&actual, codes, "{}", row["name"]);
    }
    let cut = check("cf-lww-clock.broken-clock");
    assert!(
        !cut["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"].as_str().unwrap().starts_with("W-UNREACH-"))
    );
}

#[test]
fn stale_finish_is_not_a_requires_refusal_and_work_is_consumed() {
    for name in ["cf-version", "cf-three", "cf-no-ghost-commit"] {
        let m = h::model(&h::source(name));
        for o in [engine::explore(&m), reference::explore(&m).outcome] {
            assert!(o.capability_refusals.is_none());
            assert!(o.state_range_violations.is_empty());
        }
    }
}

#[test]
fn unresolved_pairs_name_both_writers_and_span_the_later_capability() {
    for (name, variable, pairs) in [
        (
            "cf-version.broken-no-check",
            "holder",
            vec![("claim", "claim")],
        ),
        (
            "cf-lock.broken-wide-lock",
            "holder",
            vec![("claim", "claim")],
        ),
        (
            "cf-one-sided.broken-one-sided",
            "holder",
            vec![("a", "b"), ("b", "b")],
        ),
        ("cf-cross-pair.broken-one-sided", "holder", vec![("a", "b")]),
        (
            "cf-mixed-resolution.broken-one-sided",
            "n",
            vec![("a", "b")],
        ),
        ("cf-s14-mixed.broken-subtract", "x", vec![("a", "b")]),
        ("cf-subtract.broken-assign", "n", vec![("sub", "sub")]),
    ] {
        let s = h::source(name);
        let v = check(name);
        let ds: Vec<_> = v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|d| d["code"] == "U-SEM-0")
            .collect();
        assert_eq!(ds.len(), pairs.len(), "{name}");
        for (d, (left, right)) in ds.iter().zip(pairs) {
            assert_eq!(
                d["message"],
                format!(
                    "unresolved concurrent writes to Ledger.{variable} by Ledger.{left} and Ledger.{right} (G5)"
                )
            );
            let start = s.find(&format!("capability {right}(")).unwrap();
            let body = start + s[start..].find('{').unwrap();
            let mut depth = 0;
            let mut end = None;
            for (i, b) in s.bytes().enumerate().skip(body) {
                match b {
                    b'{' => depth += 1,
                    b'}' => {
                        depth -= 1;
                        if depth == 0 {
                            end = Some(i + 1);
                            break;
                        }
                    }
                    _ => (),
                }
            }
            assert_eq!(d["span"], json!({"start":start,"end":end.unwrap()}));
        }
    }
    let src = h::source("cf-cross-pair.broken-one-sided");
    let src = h::change(
        &src,
        "if p==left {a=Ledger.a()} else {b=Ledger.b()}",
        "a=Ledger.a()",
    );
    let v = json!(report::check(&src));
    assert_eq!(v["verdict"], "unsupported");
    assert!(
        v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["message"]
                == "unresolved concurrent writes to Ledger.holder by Ledger.a and Ledger.b (G5)"),
        "unused capability is still a writer"
    );
}

#[test]
fn static_version_failures_keep_precise_message_and_latent_dual_diagnostics() {
    for (name, message) in [
        (
            "cf-s05-unwritten.broken-invalid-item",
            "version_check target is not written",
        ),
        (
            "cf-s06-duplicate-target.broken-invalid-item",
            "duplicate version_check target",
        ),
        (
            "cf-s07-duplicate-item.broken-invalid-item",
            "duplicate version_check",
        ),
        (
            "cf-s08-overlap.broken-invalid-item",
            "version_check else value overlaps a result",
        ),
        (
            "cf-s09-start.broken-invalid-item",
            "version_check with start writes to checked state is unsupported",
        ),
        (
            "cf-s10-kernel.broken-version",
            "version_check with kernel outcomes is unsupported",
        ),
        (
            "cf-s11-case-lww.broken-suffix",
            "last_write_wins outside shared Actor state is unsupported",
        ),
        (
            "cf-s12-percase-lww.broken-suffix",
            "last_write_wins outside shared Actor state is unsupported",
        ),
        (
            "cf-s13-latent-lww.broken-suffix",
            "last_write_wins outside shared Actor state is unsupported",
        ),
    ] {
        let s = h::source(name);
        let v = check(name);
        let d = v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .find(|d| d["code"] == "U-SEM-0" && d["message"] == message)
            .unwrap_or_else(|| panic!("missing {message}: {v}"));
        let a = d["span"]["start"].as_u64().unwrap() as usize;
        let b = d["span"]["end"].as_u64().unwrap() as usize;
        assert!(a < b && b <= s.len() && s.is_char_boundary(a) && s.is_char_boundary(b));
    }
    let v = check("cf-latent.broken-latent");
    assert_eq!(v["verdict"], "invalid");
    assert!(
        v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "E-POL-LATENT")
    );
    assert!(
        v["diagnostics"].as_array().unwrap().iter().any(
            |d| d["code"] == "U-SEM-0" && d["message"] == "version_check target is not written"
        )
    );
}

#[test]
fn requires_reject_overlap_and_case_target_remain_unsupported() {
    let s = h::source("cf-version");
    let s = h::change(&s, "committed, conflict", "committed, conflict, denied");
    let s = h::change(
        &s,
        "      commits",
        "      requires holder==free else denied\n      commits",
    );
    assert_eq!(report::check(&s).verdict, relca_core::Verdict::Satisfied);
    let bad = h::change(
        &s,
        "requires holder==free else denied",
        "requires holder==free else conflict",
    );
    let v = json!(report::check(&bad));
    assert_eq!(v["verdict"], "unsupported");
    assert!(
        v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["message"] == "version_check else value overlaps a result")
    );
    let s = h::change(
        &h::source("cf-agency"),
        "  type R",
        "  observed case {flag:H=free}\n  type R",
    );
    let s = h::change(
        &s,
        "writes [holder, claims]",
        "writes [holder, claims, flag]",
    );
    let s = h::change(&s, "claims += 1 }", "claims += 1; flag=taken }");
    h::model(&s);
    let s = h::change(&s, "version_check [holder]", "version_check [flag]");
    let v = json!(report::check(&s));
    assert_eq!(v["verdict"], "unsupported");
    assert!(
        v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["message"] == "version_check outside shared Actor state is unsupported")
    );
}

#[test]
fn contextual_identifiers_and_newline_suffix_lookahead_do_not_become_reserved_words() {
    for s in [
        "model M version \"0.2\" {observed case{a:bool=false\nlast_write_wins:bool=false};process P{end done};bounds{cases 1;live_tasks 1;clock 3min};verify reachable end.done}",
        "model M version \"0.2\" {type version_check=enum{last_write_wins};actor last_write_wins{capacity 1;capability version_check()->version_check{duration fixed 1min;result nondet{last_write_wins}}};process P{last_write_wins=last_write_wins.version_check();end done};bounds{cases 1;live_tasks 1;clock 3min};verify reachable end.done}",
    ] {
        assert_eq!(report::check(s).verdict, relca_core::Verdict::Satisfied);
        h::model(s);
    }
}

#[test]
fn renamed_actor_resource_and_grain_keep_full_semantics_under_source_bound_projection() {
    use std::io::Write;
    for (good, bad, from, to, rename, grain) in [
        (
            "cf-label",
            "cf-label.broken-renamed",
            "Ledger",
            "Book",
            true,
            false,
        ),
        (
            "cf-label-lock",
            "cf-label-lock.broken-renamed",
            "ledger_lock",
            "slots",
            true,
            false,
        ),
        (
            "cf-grain",
            "cf-grain.broken-team",
            "  actor Ledger {",
            "  actor Ledger { grain team",
            false,
            true,
        ),
    ] {
        let old = check(good);
        let new = check(bad);
        assert_eq!(
            old["verdict"],
            h::expected(good)["verdict"],
            "alpha control must be accepted before comparing metadata"
        );
        assert_eq!(
            new["verdict"],
            h::expected(bad)["verdict"],
            "alpha counterpart must be accepted before comparing metadata"
        );
        let row = json!({"old_source":h::source(good),"new_source":h::source(bad),"old":old,"new":new,"from":from,"to":to,"rename":rename,"grain":grain});
        let mut c = std::process::Command::new("python3")
            .args(["-B", "crates/relca-core/tests/support/conflict_alpha.py"])
            .current_dir(h::root())
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        c.stdin
            .take()
            .unwrap()
            .write_all(serde_json::to_string(&row).unwrap().as_bytes())
            .unwrap();
        let o = c.wait_with_output().unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    }
}

#[test]
fn locking_amount_sum_is_wide_and_missing_capacity_is_never_guessed() {
    let s = h::source("cf-lock");
    let s = h::change(
        &s,
        "ledger_lock capacity 1",
        "ledger_lock capacity 18446744073709551615",
    );
    let s = h::change(
        &s,
        "uses ledger_lock: 1",
        "uses ledger_lock: 18446744073709551615",
    );
    let v = json!(report::check(&s));
    assert_eq!(v["verdict"], "satisfied");
    h::terminal(&v, 1, "claimed", (1, 1), (1, 1), (1, 1));
    h::terminal(&v, 2, "claimed", (2, 2), (2, 2), (1, 1));
    let s = h::change(&h::source("cf-subtract"), "capacity 2;", "");
    let v = json!(report::check(&s));
    assert_eq!(v["verdict"], "invalid");
    assert!(
        v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "E-CAP-0")
    );
    assert!(
        !v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["message"].as_str().unwrap().contains("G5"))
    );
    let s = h::change(&h::source("cf-version.broken-no-check"), "capacity 2", "");
    let v = json!(report::check(&s));
    assert_eq!(v["verdict"], "invalid");
    for code in ["E-CAP-0", "U-SEM-0"] {
        assert!(
            v["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["code"] == code)
        );
    }
}

#[test]
fn no_loss_safety_with_state_budget_zero_is_inconclusive_not_a_proof() {
    let m = h::model(&h::source("cf-lww"));
    let o = engine::explore_with_budget(&m, 0);
    assert!(o.truncated);
    assert!(
        o.properties
            .iter()
            .all(|p| p.verdict == relca_core::Verdict::Inconclusive)
    );
    assert_eq!(
        check("cf-lww")["properties"]
            .as_array()
            .unwrap()
            .last()
            .unwrap()["id"],
        "no_lost_update Ledger.holder"
    );
}

#[test]
fn no_lost_update_is_global_and_has_no_probability_notes_or_cases() {
    for (name, verdict) in [
        ("cf-lww", "satisfied"),
        ("cf-lww.broken-overlap", "violated"),
        ("cf-lww-clock.broken-clock", "inconclusive"),
        ("cf-lww-commutative", "satisfied"),
        ("cf-lww-commutative.broken-assign", "violated"),
    ] {
        let v = check(name);
        let id = if name.starts_with("cf-lww-commutative") {
            "no_lost_update Ledger.n"
        } else {
            "no_lost_update Ledger.holder"
        };
        assert_eq!(property(&v, id), &json!({"id":id,"verdict":verdict}));
        assert!(v.get("versions").is_none() && v.get("lost_updates").is_none());
    }
}

#[test]
fn agency_history_result_and_conditional_finish_are_conservative() {
    for name in ["cf-agency", "cf-version"] {
        let version = check(name);
        assert_eq!(
            version["agency"]["unchecked"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|x| **x == "state_guards")
                .count(),
            1
        );
        assert!(
            !version["agency"]["refusal"]
                .as_array()
                .unwrap()
                .iter()
                .any(|r| r["effect"] == "Ledger.claim")
        );
    }
    let ordinary = check("cf-agency.broken-ordinary-result");
    assert!(
        ordinary["agency"]["refusal"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["effect"] == "Ledger.claim")
    );
    let control = check("cf-agency-requires");
    assert!(
        control["agency"]["refusal"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["effect"] == "Poster.post")
    );
    let source = h::change(
        &h::source("cf-agency"),
        "  type R",
        "  observed case {gate:bool=false}\n  type R",
    );
    let source = h::change(
        &source,
        "writes [holder, claims]",
        "writes [holder, claims, gate]",
    );
    let source = h::change(&source, "claims += 1 }", "claims += 1; gate=true }");
    let source = h::change(
        &source,
        "  process P",
        "  actor Next{capacity 1;capability go()->R{commits;requires gate==true else conflict;duration fixed 1min;result nondet{committed}}}\n  process P",
    );
    let source = h::change(&source, "include [Ledger]", "include [Ledger,Next]");
    let source = h::change(
        &source,
        "if r == committed",
        "n=Next.go(); if r == committed",
    );
    let v = json!(report::check(&source));
    assert_eq!(v["verdict"], "satisfied");
    assert!(
        v["agency"]["unchecked"]
            .as_array()
            .unwrap()
            .contains(&json!("state_guards"))
    );
    assert!(
        !v["agency"]["refusal"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["effect"] == "Next.go")
    );
    let any = "model Any version \"0.2\" {type R=enum{committed,conflict};actor Ledger{capacity 1;state holder:bool=false;capability claim()->R{version_check [holder] else conflict;commits;writes holder;finish{holder=true};duration fixed 1min;result nondet{committed}}};actor Fast{capacity 1;capability go()->R{duration fixed 2min;result nondet{committed};cancel forbidden_after_start}};process P{fork f{a=Ledger.claim();b=Fast.go()};join any f where result==committed tie by [a,b] remaining drain if none ->miss;if b==committed{end done}else{end miss}};cut C{include[Ledger,Fast]};bounds{cases 1;live_tasks 2;clock 5min};verify reachable end.done}";
    let v = json!(report::check(any));
    assert_eq!(v["verdict"], "satisfied");
    assert_eq!(
        v["agency"]["unchecked"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|w| **w == "state_guards")
            .count(),
        1,
        "join-any must not turn a history-dependent result into Constant"
    );
}

#[test]
fn simulation_and_both_comparison_modes_preserve_global_property_and_assumptions() {
    let sample = h::change(
        &h::source("cf-lww"),
        "  policy fifo",
        "  simulation replications 10 seed 1 confidence 0.95\n  policy fifo",
    );
    let v = json!(report::simulate(&sample));
    assert_eq!(v["verdict"], "inconclusive");
    assert_eq!(
        property(&v, "no_lost_update Ledger.holder"),
        &json!({"id":"no_lost_update Ledger.holder","verdict":"inconclusive","basis":"sample","witness":null})
    );
    // No random scheduler is needed: Task1 runs0..2, Task2 runs1..3.
    // The first write at2 necessarily stales the second before its finish3.
    let loss = h::change(&sample, "capacity 1", "capacity 2");
    let loss = h::change(&loss, "duration fixed 1min", "duration fixed 2min");
    let loss = h::change(&loss, "[0min,0min]", "[0min,1min]");
    let v = json!(report::simulate(&loss));
    assert_eq!(v["verdict"], "violated");
    assert_eq!(v["simulation"]["completed_replications"], 10);
    assert_eq!(
        property(&v, "no_lost_update Ledger.holder"),
        &json!({"id":"no_lost_update Ledger.holder","verdict":"violated","basis":"witness","witness":{"replication":1}})
    );
    let m = h::model(&loss);
    let e = engine::simulate(&m);
    let r = reference::simulate(&m);
    assert_eq!(r.stop, reference::StopReason::Complete);
    assert_eq!(e, r.outcome);
    assert_eq!(e.completed_replications, 10);
    let configured = h::change(
        &sample,
        "  process P",
        "  configuration Left{cut C;process P}\n  configuration Right{cut C;process P}\n  process P",
    );
    for mode in [compare::Analysis::Exhaustive, compare::Analysis::Simulation] {
        let v = json!(compare::compare(&configured, &["Left", "Right"], mode));
        for plan in v["plans"].as_array().unwrap() {
            let r = &plan["report"];
            assert!(
                r["assumptions"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|a| a["field"] == "Ledger.holder.concurrency")
            );
            property(r, "no_lost_update Ledger.holder");
        }
    }
}

#[test]
fn configuration_free_explain_replays_a_stale_same_value_overwrite() {
    let v = json!(explain::explain(&h::source("cf-lww.broken-overlap")));
    assert_eq!(v["verdict"], "violated");
    let cs = v["explanation"]["counterexamples"].as_array().unwrap();
    let c = cs
        .iter()
        .find(|c| c["id"] == "no_lost_update Ledger.holder")
        .unwrap();
    assert_eq!(c["status"], "verified");
    assert_eq!(c["replay"]["status"], "verified");
    let last = c["steps"].as_array().unwrap().last().unwrap();
    assert_eq!(last["time"], 1);
    assert_eq!(last["kind"], "finish");
    assert!(c["property_location"].is_object());
}
