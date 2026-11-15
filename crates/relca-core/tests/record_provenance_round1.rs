//! Reviewer reproductions: lexical call positions are not binding IDs, and an
//! unrelated registration must not change observed-variable warning semantics.
use relca_core::report;
use serde_json::{Value, json};

const PVAR: &str = r#"model PVar version "0.2" {
  type V = enum { pass }
  type Ack = enum { ok }
  people [p1,q1]
  observed case { w: optional<Ack> }
  actor Doc { source document }
  actor Doc2 { source document }
  actor Know { source tacit "k" }
  actor Notes { }
  actor Intake { capacity 1; capability start()->V { reads [Doc]; duration fixed 1min; result nondet { pass } } }
  actor W { capacity 1; capability put()->Ack { reads [Doc2]; writes w; duration fixed 1min; result nondet { ok } } }
  actor Clerk { executors [p1]
    capability register() -> Ack { reads [Know]; registers [Notes]; duration fixed 1min; result nondet { ok } }
  }
  actor Reviewer { executors [q1]
    capability review(e:V) -> Ack { reads [Notes, w]; duration fixed 1min; result nondet { ok } }
  }
  cut C { include [Doc,Doc2,Know,Notes,Intake,W,Clerk,Reviewer] }
  process Main {
    s = Intake.start() response_loss sequence [delivered]
    p = W.put()
    r = Clerk.register()
    b = Reviewer.review(s)
    end done
  }
  bounds { cases 1; live_tasks 2; clock 30min }
  verify reachable end.done
}"#;

const LIN: &str = r#"model Lin version "0.2" {
  type Entry = enum { item }
  type Ack = enum { ok }
  type Verdict = enum { pass, fail }
  latent case { bad: bool = false }
  observed case { a, b: optional<Verdict>; w: optional<Ack> }
  actor Experience { source tacit "knowledge" }
  actor S { source document }
  actor R { source record }
  actor D { source document }
  actor W { capacity 1
    capability put() -> Ack { reads [D]; writes w; duration fixed 1min; result nondet { ok } }
  }
  kernel K0 { * -> { 1: item with bad = false } }
  kernel KA given [bad] { false -> { 1: pass }; true -> { 1: pass } }
  kernel KB given [bad] { false -> { 1: pass }; true -> { 1: pass } }
  actor P { capacity 1
    capability make() -> Entry { reads [S]; duration fixed 1min; outcomes kernel K0; writes bad }
  }
  actor A { capacity 1
    capability check(e:Entry) -> Verdict { reads [R]; duration fixed 1min; outcomes kernel KA; kernel_reads latent [bad]; writes a }
  }
  actor B { capacity 1
    capability check(e:Entry) -> Verdict { reads [w]; duration fixed 1min; outcomes kernel KB; kernel_reads latent [bad]; writes b }
  }
  actor Notes { }
  actor Reg { capacity 1
    capability reg() -> Ack { reads [S]; registers [Notes]; duration fixed 1min; result nondet { ok } }
  }
  actor Rd { capacity 1
    capability rd() -> Ack { reads [Notes]; duration fixed 1min; result nondet { ok } }
  }
  process Flow {
    e = P.make()
    if e == item { pw = W.put() } else {}
    aV = A.check(e)
    bV = B.check(e)
    rr = Reg.reg()
    rd = Rd.rd()
    end done
  }
  cut C { include [Experience,S,R,P,A,B,D,W,Notes,Reg,Rd] }
  bounds { cases 1; live_tasks 2; clock 30min }
  verify safety no_resource_overcommit
}"#;

fn check(source: &str) -> Value {
    let report = serde_json::to_value(report::check(source)).unwrap();
    assert_eq!(report["verdict"], "satisfied", "{report}");
    report
}

fn observation(
    source: &str,
    call: &str,
    outputs: Value,
    record: &str,
    registrar: &str,
    source_name: &str,
    people: Value,
) {
    let report = check(source);
    let obs = &report["agency"]["viewpoints"]["observation"];
    assert_eq!(obs["status"], "derived", "{obs}");
    for key in ["sources", "people", "stages"] {
        assert!(obs[key].is_array(), "{key}: {obs}");
    }
    let stage = obs["stages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["call"] == call)
        .unwrap();
    assert_eq!(stage["direct"], json!([]));
    assert_eq!(stage["via_outputs"], outputs);
    assert_eq!(stage["via_outputs_status"], "derived");
    assert_eq!(
        stage["via_records"],
        json!([{"record":record,"registrars":[{"call":registrar,"people":people}],
        "sources":[{"source":source_name,"route":"registrar_direct"}]}])
    );
}

#[test]
fn pidx_unrelated_response_without_variables_keeps_observation() {
    let source = PVAR.replace("  observed case { w: optional<Ack> }\n", "")
        .replace("  actor Doc2 { source document }\n", "")
        .replace("  actor W { capacity 1; capability put()->Ack { reads [Doc2]; writes w; duration fixed 1min; result nondet { ok } } }\n", "")
        .replace("Notes, w", "Notes").replace("Doc,Doc2,", "Doc,").replace("Intake,W,", "Intake,")
        .replace("    p = W.put()\n", "");
    observation(
        &source,
        "b",
        json!(["Doc"]),
        "Notes",
        "r",
        "Know",
        json!(["p1"]),
    );
}

#[test]
fn pvar_unrelated_response_with_variable_keeps_both_output_sources() {
    observation(
        PVAR,
        "b",
        json!(["Doc", "Doc2"]),
        "Notes",
        "r",
        "Know",
        json!(["p1"]),
    );
}

fn repeated(before: bool) -> String {
    let source = PVAR.replace(" response_loss sequence [delivered]", "")
        .replace("  actor Notes { }", "  actor Notes { }\n  actor Office { capacity 1; capability inspect()->V { duration fixed 1min; result nondet { pass } } }")
        .replace("Doc,Doc2,Know,", "Doc,Doc2,Office,Know,");
    let repeat = "    repeat rw at_most 2 { k=Office.inspect(); if k==pass { leave } else { again } } on_limit { end esc }\n";
    if before {
        source.replace(
            "  process Main {\n",
            &format!("  process Main {{\n{repeat}"),
        )
    } else {
        source.replace("    end done", &format!("{repeat}    end done"))
    }
}

#[test]
fn prep_unrelated_repeat_before_record_keeps_observation() {
    observation(
        &repeated(true),
        "b",
        json!(["Doc", "Doc2"]),
        "Notes",
        "r",
        "Know",
        json!(["p1"]),
    );
}

#[test]
fn prep2_unrelated_repeat_after_record_keeps_observation() {
    observation(
        &repeated(false),
        "b",
        json!(["Doc", "Doc2"]),
        "Notes",
        "r",
        "Know",
        json!(["p1"]),
    );
}

#[test]
fn ltreg_resp_unrelated_response_keeps_tacit_record_observation() {
    let source = LIN
        .replace("reads [w]", "reads [Notes]")
        .replace("reads [S]; registers", "reads [Experience]; registers")
        .replace(
            "e = P.make()",
            "e = P.make() response_loss sequence [delivered]",
        )
        .replace("    if e == item { pw = W.put() } else {}\n", "")
        .replace("    rr = Reg.reg()\n", "")
        .replace(
            "    aV = A.check(e)",
            "    rr = Reg.reg()\n    aV = A.check(e)",
        );
    observation(
        &source,
        "bV",
        json!(["S"]),
        "Notes",
        "rr",
        "Experience",
        json!([]),
    );
}

#[test]
fn ov_reg_and_ov_ctrl_keep_observed_variable_warning_semantics() {
    let control = LIN
        .replace("  actor Notes { }\n", "")
        .replace("; registers [Notes]", "")
        .replace("reads [Notes]", "reads [S]")
        .replace("W,Notes,Reg", "W,Reg");
    for source in [LIN, control.as_str()] {
        let report = check(source);
        let warnings: Vec<_> = report["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|d| d["code"] == "W-LIN-2")
            .collect();
        assert!(warnings.is_empty(), "{warnings:?}");
    }
}

#[test]
fn an_observed_variable_keeps_a_source_also_supplied_by_a_may_record() {
    let source = LIN
        .replace("reads [w]", "reads [w,Notes]")
        .replace("reads [S]; registers", "reads [D]; registers")
        .replace("    rr = Reg.reg()\n", "")
        .replace(
            "    aV = A.check(e)",
            "    if e == item { rr = Reg.reg() } else {}\n    aV = A.check(e)",
        );
    let report = check(&source);
    assert!(
        !report["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "W-LIN-2")
    );
    let stage = report["agency"]["viewpoints"]["observation"]["stages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["call"] == "bV")
        .unwrap();
    assert_eq!(stage["via_records"], json!([]));
    assert_eq!(stage["via_outputs"], json!(["D", "S"]));
}

#[test]
fn record_read_mutations_identify_records_without_promoting_them_to_sources() {
    let source = include_str!("record_provenance/r0.relca");
    let options = relca_core::mutation::Options::parse(
        None,
        Some("original_access_removed,evidence_missing"),
    )
    .unwrap();
    let value = serde_json::to_value(relca_core::mutation::mutate_with_options(
        source,
        "bpr-default",
        None,
        &options,
    ))
    .unwrap();
    for kind in ["original_access_removed", "evidence_missing"] {
        let row = value["mutations"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["kind"] == kind)
            .unwrap();
        let site = row["mutants"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| {
                m["id"]
                    .as_str()
                    .unwrap()
                    .ends_with("Reviewer.review:reads:Notes")
            })
            .unwrap();
        assert_eq!(site["site"]["record"], "Notes");
        assert!(site["site"]["source"].is_null());
        let source_site = row["mutants"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| {
                m["id"]
                    .as_str()
                    .unwrap()
                    .ends_with("Clerk.register:reads:Know")
            })
            .unwrap();
        assert_eq!(source_site["site"]["source"], "Know");
        assert!(source_site["site"].get("record").is_none());
    }
}

#[test]
fn decides_is_rejected_as_registration_target_behavior() {
    let source = include_str!("record_provenance/r0.relca").replace(
        "actor Notes { }",
        "actor Notes { decides flag: bool = false }",
    );
    let report = serde_json::to_value(report::check(&source)).unwrap();
    assert_eq!(report["verdict"], "invalid");
    assert!(
        report["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "E-CON-0" && d["message"] == "registration_target_has_behavior")
    );
}
