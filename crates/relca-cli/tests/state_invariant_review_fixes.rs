//! ADR 0037 review fixes: a parenthesized consequent followed by `.`, `+`, `-`,
//! `*` or `/` keeps the old U-SYN-0 classification. Expectations for the old
//! event form were captured from the pre-ADR-0037 binary (7f8ee1e), never from
//! this implementation.
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
fn old_event_invariant_group_followed_by_arithmetic_is_byte_identical_to_pre_adr0037_binary() {
    let mut calibrated = false;
    // (actor state declared, consequent, old stdout sha256, old stderr, old exit)
    for (actor_state, form, stdout, stderr, exit) in [
        (
            true,
            "(x == ok) + 1",
            "8465a5a1206c3f512cf754d013498ff38b5292fa3546c07014469a1be1752cad",
            OLD_USYN,
            4,
        ),
        (
            true,
            "(x == ok) * 2",
            "2e005cf73dfbcfd66b5484f4e633b501e5ae2ac4fa3764599ddaae874df8c381",
            OLD_USYN,
            4,
        ),
        (
            true,
            "(x == ok) - 1",
            "ace82783a0778a6c7b845c683677546c167230c4da7955386ca8c2b6f7bf359c",
            OLD_USYN,
            4,
        ),
        (
            true,
            "(x == ok) / 2",
            "99afeac415e12b4aafb0359e54b832e482e4db5860c22efeec169ddb35d6af03",
            OLD_USYN,
            4,
        ),
        (
            true,
            "(x == ok).y",
            "b2418247a6689bb408fa42a613a6c3eecf4a7d98ea59ce9d464c99f7450d2f8e",
            OLD_USYN,
            4,
        ),
        (
            true,
            "(x == ok && x == ok) + 1",
            "2f239d73f5955df52d1ed72c1be87390bc1941767fb034bd1472170dae89d98b",
            OLD_USYN,
            4,
        ),
        (
            true,
            "((x == ok)) + 1",
            "270b6b62bdc28ecd3dc0b0c05fa275af096dc407415515de5e9b5652a0acc92b",
            OLD_USYN,
            4,
        ),
        (
            true,
            "x == ok || (x == ok) + 1",
            "206fd9014719ca2b2b77b8fb1f8479c938ab7e8b54c681d170eaf18c5ef3edde",
            OLD_USYN,
            4,
        ),
        (
            false,
            "(x == ok) + 1",
            "b89abdaab110612c3ea747bf977119d891b53c5ae2b40a70b63de26af607dc6d",
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
        let original = syntax_cut_expectations::archived_projection(
            &ledger_shape::remove_expected_prefix(&o.stdout, &spec),
        );
        if !calibrated {
            let mut fault = o.stdout.clone();
            let key = b"\"verdict\":\"";
            let at = fault.windows(key.len()).position(|s| s == key).unwrap() + key.len();
            fault[at] = b'v';
            let bad = syntax_cut_expectations::archived_projection(
                &ledger_shape::remove_expected_prefix(&fault, &spec),
            );
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
