//! ADR 0037 review fixes: a parenthesized consequent followed by `.`, `+`, `-`,
//! `*` or `/` keeps the old U-SYN-0 classification. Check current refusal bytes.
use serde_json::Value;
#[path = "support/ledger_shape.rs"]
mod ledger_shape;
#[path = "support/syntax_cut_expectations.rs"]
mod syntax_cut_expectations;
#[path = "support/test_tmp.rs"]
mod test_tmp;
use std::{
    io::Write,
    path::PathBuf,
    process::{Command, Output, Stdio},
};

const TEMPLATE: &str = r#"model M version "0.2" {
  type R = enum { ok, bad }
  observed case { x: R = ok }
  actor A { capacity 1; state s: R = ok; capability b() -> R { duration fixed 1min; result nondet { ok }; writes x; finish { x = ok } } }
  process P { r = A.b(); end done }
  cut C { include [A] }
  bounds { cases 1; live_tasks 1; clock 3min }
  contract Controls { invariant FORM }
}
"#;

fn sha(bytes: &[u8]) -> String {
    let mut c = Command::new("sha256sum")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    c.stdin.take().unwrap().write_all(bytes).unwrap();
    let o = c.wait_with_output().unwrap();
    assert!(o.status.success());
    String::from_utf8(o.stdout)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .into()
}

fn change(s: &str, a: &str, b: &str) -> String {
    let t = s.replace(a, b);
    assert_ne!(s, t, "replacement must occur: {a}");
    t
}

fn check(name: &str, src: &str) -> Output {
    let dir = test_tmp::root().join(format!("relca-sinv-review-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path: PathBuf = dir.join("model.relca");
    std::fs::write(&path, src).unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_relca"))
        .arg("check")
        .arg(&path)
        .output()
        .unwrap();
    std::fs::remove_file(&path).unwrap();
    std::fs::remove_dir(&dir).unwrap();
    o
}

const OLD_USYN: &str = "U-SYN-0: unsupported syntax: comparison expression\nW-CUT-0: Cut is undeclared; exclusions are unknown\n";
const OLD_ESYN: &str = "E-SYN-0: expected contract clause separator\nW-CUT-0: Cut is undeclared; exclusions are unknown\n";

#[test]
fn event_invariant_group_followed_by_arithmetic_has_current_refusal_bytes() {
    let mut calibrated = false;
    // (actor state declared, consequent, old stdout sha256, old stderr, old exit)
    for (actor_state, form, stdout, stderr, exit) in [
        (
            true,
            "(x == ok) + 1",
            "172835703a056a5b6a6e9ecd6c71109d82c2e23ef629d56db1836c63ce9e731b",
            OLD_USYN,
            4,
        ),
        (
            true,
            "(x == ok) * 2",
            "b48d3f1f12082f3d7233e20ea1f0104a149c0dea222eef7a6b708df311df2838",
            OLD_USYN,
            4,
        ),
        (
            true,
            "(x == ok) - 1",
            "42b4f1f69cb759fea1cff360065dc714fd0c9eb0eef1c666ef692e822ed83a3a",
            OLD_USYN,
            4,
        ),
        (
            true,
            "(x == ok) / 2",
            "c194a64b08c63fd2e033de9f4fd870ec716c6ab1ca9dc8ab67c1235b712c3b54",
            OLD_USYN,
            4,
        ),
        (
            true,
            "(x == ok).y",
            "36adc902ebc1890cba1d78f2e37dc33378855e2a6af98deefac50130c48eb88a",
            OLD_USYN,
            4,
        ),
        (
            true,
            "(x == ok && x == ok) + 1",
            "e6845753040e6a92a2f574816b4f47b562a55ad4d4fd67db8772a4f67cc2bb49",
            OLD_USYN,
            4,
        ),
        (
            true,
            "((x == ok)) + 1",
            "4f67db439222b8b5b760cb29548c78b1e669763732739c1f6b369ebfef1debbe",
            OLD_USYN,
            4,
        ),
        (
            true,
            "x == ok || (x == ok) + 1",
            "78641e37170f78456f055881d3ff44bfdedd5f1448b4166b7cc3e3ec0f122953",
            OLD_USYN,
            4,
        ),
        (
            false,
            "(x == ok) + 1",
            "997a6490af413cee51aa150962a41cda56a0b448852657c7484f69b065c7fdeb",
            OLD_ESYN,
            3,
        ),
    ] {
        let mut src = change(TEMPLATE, "FORM", &format!("A.b.finish -> {form}"));
        if !actor_state {
            src = change(&src, "state s: R = ok; ", "");
        }
        let o = check(&format!("event-{}", sha(src.as_bytes())), &src);
        let spec = ledger_shape::Spec::prelower("exhaustive", false);
        let original = ledger_shape::remove_expected_prefix(&o.stdout, &spec);
        if !calibrated {
            let mut fault = o.stdout.clone();
            let key = b"\"verdict\":\"";
            let at = fault.windows(key.len()).position(|s| s == key).unwrap() + key.len();
            fault[at] = b'v';
            let bad = ledger_shape::remove_expected_prefix(&fault, &spec);
            assert_ne!(
                sha(&bad),
                stdout,
                "one non-ledger byte must break the old digest"
            );
            calibrated = true;
        }
        assert_eq!(
            (
                sha(&original),
                String::from_utf8_lossy(&o.stderr).into_owned(),
                o.status.code()
            ),
            (
                stdout.to_string(),
                stderr.replace("W-CUT-0: Cut is undeclared; exclusions are unknown\n", ""),
                Some(exit)
            ),
            "old event invariant {form} (actor state {actor_state})"
        );
    }
}

#[test]
fn state_invariant_group_followed_by_arithmetic_is_the_same_unsupported_comparison() {
    let base = change(TEMPLATE, "x: R = ok }", "x: R = ok; f: bool = true }");
    // (clause, operator token that the unsupported diagnostic points at)
    for (form, token) in [
        ("(x == ok) + 1", "+ 1"),
        ("x == ok -> (x == ok) * 2", "* 2"),
        ("(x == ok).y -> x == ok", ".y"),
        ("(f) - 1", "- 1"),
        ("x == ok && (f) / 2", "/ 2"),
    ] {
        let src = change(&base, "FORM", form);
        let o = check(&format!("state-{}", sha(src.as_bytes())), &src);
        assert_eq!(o.status.code(), Some(4), "{form}");
        assert_eq!(
            String::from_utf8_lossy(&o.stderr),
            "U-SYN-0: unsupported syntax: comparison expression\n",
            "{form}: same code and message as the old comparison check"
        );
        let v: Value = serde_json::from_slice(&o.stdout).unwrap();
        assert_eq!(v["verdict"], "unsupported");
        let d = &v["diagnostics"][0];
        let a = d["span"]["start"].as_u64().unwrap() as usize;
        let b = d["span"]["end"].as_u64().unwrap() as usize;
        let at = src.rfind(token).unwrap();
        assert_eq!((a, b), (at, at + 1), "{form}: span is the operator token");
    }
}
