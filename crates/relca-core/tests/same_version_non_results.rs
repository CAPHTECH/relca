//! Non-result inputs are not value versions. Unsupported argument syntax keeps
//! its existing parser/name diagnostic, with and without the new contract.
use relca_core::{Verdict, report};

const CLAUSE: &str = "contract V { same_version A.first(x), A.second(x) }";
const MODEL: &str = r#"model NonResults version "0.2" {
  type Doc = enum { draft }
  type R = record { flag: bool }
  actor A { capacity 2
    capability make() -> Doc { duration fixed 1min; result nondet { draft } }
    capability first(x:Doc) -> Doc { duration fixed 1min; result nondet { draft } }
    capability second(x:Doc) -> Doc { duration fixed 1min; result nondet { draft } }
  }
  process P { d = A.make(); a = A.first(d); b = A.second(d); end done }
  cut C { include [A] }
  bounds { cases 1; live_tasks 2; clock 10min }
  verify reachable end.done
  CONTRACT
}"#;

#[test]
fn case_id_is_not_a_version_on_either_side_or_in_nested_and_parallel_calls() {
    let base = MODEL.replace("x:Doc", "x:CaseId");
    for body in [
        "a = A.first(case.id); b = A.second(case.id); end done",
        "a = A.first(case.id); end done",
        "b = A.second(case.id); end done",
        "d = A.make(); if d == draft { a = A.first(case.id); b = A.second(case.id) } else {}; end done",
        "fork f { a = A.first(case.id); b = A.second(case.id) }; join all f; end done",
    ] {
        let src = base.replace(
            "d = A.make(); a = A.first(d); b = A.second(d); end done",
            body,
        );
        let old = report::check(&src.replace("CONTRACT", ""));
        assert_eq!(
            old.verdict,
            Verdict::Satisfied,
            "{body}: {:?}",
            old.diagnostics
        );
        let new = report::check(&src.replace("CONTRACT", CLAUSE));
        assert_eq!(
            new.verdict,
            Verdict::Unsupported,
            "{body}: {:?}",
            new.diagnostics
        );
        assert!(
            new.diagnostics
                .iter()
                .any(|d| d.code == "U-SEM-0" && d.message == "same_version_feature_combination")
        );
    }
}

#[test]
fn case_id_in_an_unselected_parameter_does_not_forbid_call_result_versions() {
    let src = MODEL
        .replace("x:Doc", "x:Doc, key:CaseId")
        .replace("A.first(d)", "A.first(d,case.id)")
        .replace("A.second(d)", "A.second(d,case.id)")
        .replace("CONTRACT", CLAUSE);
    assert_eq!(report::check(&src).verdict, Verdict::Satisfied);
}

#[test]
fn an_observed_case_alias_is_not_a_produced_version_even_with_a_later_writer() {
    let base = r#"model Probe version "0.2" {
      type Doc = enum { draft }
      observed case { x: Doc = draft }
      actor A { capacity 2
        capability make() -> Doc { writes x; duration fixed 1min; result nondet { draft } }
        capability first(v:Doc) -> Doc { duration fixed 1min; result nondet { draft } }
        capability second(v:Doc) -> Doc { duration fixed 1min; result nondet { draft } }
      }
      process P { a = A.first(x); b = A.second(x); x = A.make(); end done }
      cut C { include [A] }
      bounds { cases 2; live_tasks 2; clock 20min }
      arrivals fixed [0min,0min]
      verify reachable end.done
      CONTRACT
    }"#;
    for body in [
        "a = A.first(x); b = A.second(x); x = A.make(); end done",
        "x = A.make(); a = A.first(x); b = A.second(x); end done",
        "x = A.make(); ready = if x == draft { yield x } else { yield x }; a = A.first(ready); b = A.second(ready); end done",
    ] {
        let src = base.replace(
            "a = A.first(x); b = A.second(x); x = A.make(); end done",
            body,
        );
        assert_eq!(
            report::check(&src.replace("CONTRACT", "")).verdict,
            Verdict::Satisfied
        );
        let r = report::check(&src.replace(
            "CONTRACT",
            "contract V { same_version A.first(v), A.second(v) }",
        ));
        assert_eq!(
            r.verdict,
            Verdict::Unsupported,
            "{body}: {:?}",
            r.diagnostics
        );
        assert!(
            r.diagnostics
                .iter()
                .any(|d| d.code == "U-SEM-0" && d.message == "same_version_feature_combination")
        );
    }
    let pure = MODEL.replace("d = A.make(); a = A.first(d); b = A.second(d); end done", "d = A.make(); ready = if d == draft { yield d } else { yield d }; a = A.first(ready); b = A.second(ready); end done").replace("CONTRACT", CLAUSE);
    assert_eq!(report::check(&pure).verdict, Verdict::Satisfied);
}

#[test]
fn literals_constants_state_and_other_builtins_keep_existing_fail_closed_errors() {
    for (argument, code) in [
        ("0", "E-SYN-0"),
        ("-1", "E-SYN-0"),
        ("1.5", "E-SYN-0"),
        ("\"draft\"", "E-SYN-0"),
        ("true", "E-NAME-0"),
        ("false", "E-NAME-0"),
        ("none", "E-SYN-0"),
        ("draft", "E-NAME-0"),
        ("Doc.draft", "E-SYN-0"),
        ("input", "E-NAME-0"),
        ("case.other", "U-SYN-0"),
        ("case.received", "U-SYN-0"),
        ("case.admitted", "U-SYN-0"),
        ("executor.id", "E-SYN-0"),
        ("attempt.round", "E-SYN-0"),
        ("clock", "E-SYN-0"),
        ("case_id", "E-NAME-0"),
        ("p1", "E-NAME-0"),
        ("result", "E-SYN-0"),
        ("end.done", "E-SYN-0"),
        ("[d]", "E-SYN-0"),
        ("{d}", "E-SYN-0"),
        ("R { flag = true }", "U-SYN-0"),
        ("null", "E-NAME-0"),
        ("d + d", "E-SYN-0"),
        ("A.make()", "E-SYN-0"),
        ("d.field", "E-SYN-0"),
        ("A.flag", "E-SYN-0"),
        ("x=d", "U-SYN-0"),
        ("state_value", "E-NAME-0"),
        ("secret", "E-NAME-0"),
        ("A", "E-NAME-0"),
    ] {
        let src = MODEL
            .replace("A.first(d)", &format!("A.first({argument})"))
            .replace(
                "  process P",
                "  people [p1]\n  observed case { state_value: Doc = draft }\n  latent case { secret: Doc = draft }\n  process P",
            );
        let old = report::check(&src.replace("CONTRACT", ""));
        let new = report::check(&src.replace("CONTRACT", CLAUSE));
        assert!(
            matches!(old.verdict, Verdict::Invalid | Verdict::Unsupported),
            "{argument}"
        );
        assert_eq!(new.verdict, old.verdict, "{argument}");
        assert!(
            old.diagnostics.iter().any(|d| d.code == code),
            "{argument}: {:?}",
            old.diagnostics
        );
        assert_eq!(
            new.diagnostics, old.diagnostics,
            "{argument}: existing refusal must not change"
        );
    }
    let src = MODEL.replace("process P {", "process P(input:Doc) {");
    for contract in ["", CLAUSE] {
        let r = report::check(&src.replace("CONTRACT", contract));
        assert_eq!(r.verdict, Verdict::Invalid);
        assert!(
            r.diagnostics
                .iter()
                .any(|d| d.code == "E-SYN-0" && d.message.contains("expected {"))
        );
    }
}
