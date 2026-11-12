//! ADR 0032 boundary-review fixes on the wire.
//!
//! Requirement: models without Actor `state` keep the diagnostics of the
//! pre-change production (revision 1fcaf03, archived baseline executable) byte
//! for byte. Because the requirement is "does not change", the expected
//! stdout, stderr and exit of each `Legacy` row were copied once from that
//! binary's `relca check` output; the test never calls the old binary.
//! The rows with Actor `state` cannot be run on the old binary (it stops at
//! the `state` declaration); their expectations are written by hand from the
//! old parser rule for a qualified responsibility comparison: U-SYN-0
//! `unsupported syntax: responsibility target expression`, spanning the first
//! character of the operator, verdict `unsupported`, exit 4.
use serde_json::Value;
use std::path::PathBuf;
use std::process::Command;

const REQ: &str = "model M version \"0.2\" {\n  type T = enum { ok, no }\n  actor A { capacity 1; latent n: T = ok; capability run() -> T { requires n == ok else no; duration fixed 1min; result nondet { ok } } }\n  process P { a = A.run(); end done }\n  bounds { cases 1; live_tasks 1; clock 5min }\n  verify safety no_resource_overcommit\n}\n";
const RESP: &str = "model M version \"0.2\" {\n  type R = enum { ok, no }\n  people [p]; actor Doc { source record; latent level: R = ok }; actor Reader { executors [p]; capability read() -> R { duration fixed 1min; result nondet { ok } } }\n  process P { a = Reader.read(); end done }\n  cut C { include [Doc,Reader] }\n  bounds { cases 1; live_tasks 2; clock 20min }\n  responsible p for (Doc.level == ok)\n  verify safety no_resource_overcommit\n}\n";
const DUP: &str = "model M version \"0.2\" {\n  type T = enum { ok, no }\n  observed case { k: T = ok }\n  actor A { capacity 1; capability run() -> T { finish { z = ok }; duration fixed 1min; result nondet { ok } } }\n  process P { a = A.run(); end done }\n  bounds { cases 1; live_tasks 1; clock 5min }\n  verify safety no_resource_overcommit\n}\n";
const STATE_RESP: &str = "model M version \"0.2\" {\n  type R = enum { ok, no }\n  people [p]; actor Doc { source record; state level: 0..3 = 0 }; actor Reader { executors [p]; capability read() -> R { reads [Doc]; duration fixed 1min; result nondet { ok } } }\n  process P { a = Reader.read(); end done }\n  cut C { include [Doc,Reader] }\n  bounds { cases 1; live_tasks 2; clock 20min }\n  responsible p for (Doc.level == 1)\n  verify safety no_resource_overcommit\n}\n";
const OBSERVED_REQ: &str = "model M version \"0.2\" {\n  type T = enum { ok, no }\n  actor A { capacity 1; state n: 0..1 = 0; capability run() -> T { requires n == 0 else no; writes n; finish { n = 1 }; duration fixed 1min; result nondet { ok } } }\n  process P { a = A.run(); end done }\n  bounds { cases 1; live_tasks 1; clock 5min }\n  verify safety no_resource_overcommit\n}\n";

struct Legacy {
    name: &'static str,
    base: &'static str,
    change: Option<(&'static str, &'static str)>,
    stdout: &'static str,
    stderr: &'static str,
    exit: i32,
}

const LEGACY: &[Legacy] = &[
    Legacy {
        name: r#"fix1_requires_eq_latent"#,
        base: REQ,
        change: None,
        stdout: r#"{"actors":null,"cut_context":{"notices":["Cut 外の存在(未知の被影響者、一度だけの拒否、登録されていない当事者)が作用しないとは主張しない","この結果を運用の範囲決定に使う場合は採用経路を別に追うこと"],"reentry":[],"horizon":[]},"agency":null,"verdict":"invalid","mode":"exhaustive","unit":"min","model":{"name":"M","sha256":"71693bd78448800feab2230c6b24976a9992db5f817ff08d5d94cb3f086c0c64"},"cut":"undeclared","assumptions":[],"diagnostics":[{"code":"E-NAME-0","severity":"error","message":"requires needs an observed case variable","span":{"start":126,"end":127}},{"code":"W-CUT-0","severity":"warning","message":"Cut is undeclared: all declared Actors are included; exclusions are unknown","span":null}],"bounds":{"cases":null,"live_tasks":null,"clock":null,"truncated":null},"properties":[],"terminals":[]}
"#,
        stderr: r#"E-NAME-0: requires needs an observed case variable
W-CUT-0: Cut is undeclared: all declared Actors are included; exclusions are unknown
"#,
        exit: 3,
    },
    Legacy {
        name: r#"fix1_requires_lt_latent"#,
        base: REQ,
        change: Some((r#"requires n == ok"#, r#"requires n < 1"#)),
        stdout: r#"{"actors":null,"cut_context":{"notices":["Cut 外の存在(未知の被影響者、一度だけの拒否、登録されていない当事者)が作用しないとは主張しない","この結果を運用の範囲決定に使う場合は採用経路を別に追うこと"],"reentry":[],"horizon":[]},"agency":null,"verdict":"invalid","mode":"exhaustive","unit":"min","model":{"name":"M","sha256":"662e2d8d2b009220ef9edd5f1e966df792fe806f237baffef99fddc1b85fdcb0"},"cut":"undeclared","assumptions":[],"diagnostics":[{"code":"E-SYN-0","severity":"error","message":"expected ==","span":{"start":128,"end":129}},{"code":"W-CUT-0","severity":"warning","message":"Cut is undeclared: all declared Actors are included; exclusions are unknown","span":null}],"bounds":{"cases":null,"live_tasks":null,"clock":null,"truncated":null},"properties":[],"terminals":[]}
"#,
        stderr: r#"E-SYN-0: expected ==
W-CUT-0: Cut is undeclared: all declared Actors are included; exclusions are unknown
"#,
        exit: 3,
    },
    Legacy {
        name: r#"fix2_resp_lt"#,
        base: RESP,
        change: Some((r#"Doc.level == ok"#, r#"Doc.level < 1"#)),
        stdout: r#"{"actors":null,"cut_context":{"notices":["Cut 外の存在(未知の被影響者、一度だけの拒否、登録されていない当事者)が作用しないとは主張しない","この結果を運用の範囲決定に使う場合は採用経路を別に追うこと"],"reentry":[],"horizon":[]},"agency":null,"verdict":"unsupported","mode":"exhaustive","unit":"min","model":{"name":null,"sha256":"5bf3cbf5809dd81fa73bb57da9e5e2e41935963bad3d6cf78503793cce87a7af"},"cut":"undeclared","assumptions":[],"diagnostics":[{"code":"U-SYN-0","severity":"error","message":"unsupported syntax: responsibility target expression","span":{"start":373,"end":374}},{"code":"W-CUT-0","severity":"warning","message":"Cut is undeclared; exclusions are unknown","span":null}],"bounds":{"cases":null,"live_tasks":null,"clock":null,"truncated":null},"properties":[],"terminals":[]}
"#,
        stderr: r#"U-SYN-0: unsupported syntax: responsibility target expression
W-CUT-0: Cut is undeclared; exclusions are unknown
"#,
        exit: 4,
    },
    Legacy {
        name: r#"fix2_resp_le"#,
        base: RESP,
        change: Some((r#"Doc.level == ok"#, r#"Doc.level <= 1"#)),
        stdout: r#"{"actors":null,"cut_context":{"notices":["Cut 外の存在(未知の被影響者、一度だけの拒否、登録されていない当事者)が作用しないとは主張しない","この結果を運用の範囲決定に使う場合は採用経路を別に追うこと"],"reentry":[],"horizon":[]},"agency":null,"verdict":"unsupported","mode":"exhaustive","unit":"min","model":{"name":null,"sha256":"82b94f990a86f174a1df0aaa93e1d789d204ca53d9b1212b2c763edc7d444638"},"cut":"undeclared","assumptions":[],"diagnostics":[{"code":"U-SYN-0","severity":"error","message":"unsupported syntax: responsibility target expression","span":{"start":373,"end":374}},{"code":"W-CUT-0","severity":"warning","message":"Cut is undeclared; exclusions are unknown","span":null}],"bounds":{"cases":null,"live_tasks":null,"clock":null,"truncated":null},"properties":[],"terminals":[]}
"#,
        stderr: r#"U-SYN-0: unsupported syntax: responsibility target expression
W-CUT-0: Cut is undeclared; exclusions are unknown
"#,
        exit: 4,
    },
    Legacy {
        name: r#"fix2_resp_ne"#,
        base: RESP,
        change: Some((r#"Doc.level == ok"#, r#"Doc.level != ok"#)),
        stdout: r#"{"actors":null,"cut_context":{"notices":["Cut 外の存在(未知の被影響者、一度だけの拒否、登録されていない当事者)が作用しないとは主張しない","この結果を運用の範囲決定に使う場合は採用経路を別に追うこと"],"reentry":[],"horizon":[]},"agency":null,"verdict":"unsupported","mode":"exhaustive","unit":"min","model":{"name":null,"sha256":"77879580cebf55d440d04413b8416efd8f29527d21e76d60e9540c8cb9bcd38f"},"cut":"undeclared","assumptions":[],"diagnostics":[{"code":"U-SYN-0","severity":"error","message":"unsupported syntax: responsibility target expression","span":{"start":373,"end":374}},{"code":"W-CUT-0","severity":"warning","message":"Cut is undeclared; exclusions are unknown","span":null}],"bounds":{"cases":null,"live_tasks":null,"clock":null,"truncated":null},"properties":[],"terminals":[]}
"#,
        stderr: r#"U-SYN-0: unsupported syntax: responsibility target expression
W-CUT-0: Cut is undeclared; exclusions are unknown
"#,
        exit: 4,
    },
    Legacy {
        name: r#"fix2_resp_gt"#,
        base: RESP,
        change: Some((r#"Doc.level == ok"#, r#"Doc.level > 1"#)),
        stdout: r#"{"actors":null,"cut_context":{"notices":["Cut 外の存在(未知の被影響者、一度だけの拒否、登録されていない当事者)が作用しないとは主張しない","この結果を運用の範囲決定に使う場合は採用経路を別に追うこと"],"reentry":[],"horizon":[]},"agency":null,"verdict":"unsupported","mode":"exhaustive","unit":"min","model":{"name":null,"sha256":"20b9ee5b735cd834c47cdcee6c9fa2cc7fca42214b779a3f041cf05609f7b51c"},"cut":"undeclared","assumptions":[],"diagnostics":[{"code":"U-SYN-0","severity":"error","message":"unsupported syntax: responsibility target expression","span":{"start":373,"end":374}},{"code":"W-CUT-0","severity":"warning","message":"Cut is undeclared; exclusions are unknown","span":null}],"bounds":{"cases":null,"live_tasks":null,"clock":null,"truncated":null},"properties":[],"terminals":[]}
"#,
        stderr: r#"U-SYN-0: unsupported syntax: responsibility target expression
W-CUT-0: Cut is undeclared; exclusions are unknown
"#,
        exit: 4,
    },
    Legacy {
        name: r#"fix2_resp_ge"#,
        base: RESP,
        change: Some((r#"Doc.level == ok"#, r#"Doc.level >= 1"#)),
        stdout: r#"{"actors":null,"cut_context":{"notices":["Cut 外の存在(未知の被影響者、一度だけの拒否、登録されていない当事者)が作用しないとは主張しない","この結果を運用の範囲決定に使う場合は採用経路を別に追うこと"],"reentry":[],"horizon":[]},"agency":null,"verdict":"unsupported","mode":"exhaustive","unit":"min","model":{"name":null,"sha256":"ed649e308caa0eb60cc0c3d2fa324adfc0f74e46d62909bb93cecb3f5e91af0c"},"cut":"undeclared","assumptions":[],"diagnostics":[{"code":"U-SYN-0","severity":"error","message":"unsupported syntax: responsibility target expression","span":{"start":373,"end":374}},{"code":"W-CUT-0","severity":"warning","message":"Cut is undeclared; exclusions are unknown","span":null}],"bounds":{"cases":null,"live_tasks":null,"clock":null,"truncated":null},"properties":[],"terminals":[]}
"#,
        stderr: r#"U-SYN-0: unsupported syntax: responsibility target expression
W-CUT-0: Cut is undeclared; exclusions are unknown
"#,
        exit: 4,
    },
    Legacy {
        name: r#"fix3_finish_undefined_twice"#,
        base: DUP,
        change: Some((r#"finish { z = ok }"#, r#"finish { z = ok; z = ok }"#)),
        stdout: r#"{"actors":null,"cut_context":{"notices":["Cut 外の存在(未知の被影響者、一度だけの拒否、登録されていない当事者)が作用しないとは主張しない","この結果を運用の範囲決定に使う場合は採用経路を別に追うこと"],"reentry":[],"horizon":[]},"agency":null,"verdict":"invalid","mode":"exhaustive","unit":"min","model":{"name":"M","sha256":"ee525fb22e42737df6c11b759df92f624d1f96c3b5a1eb222cbe035563b8dd74"},"cut":"undeclared","assumptions":[],"diagnostics":[{"code":"U-SEM-0","severity":"error","message":"duplicate finish target","span":{"start":146,"end":147}},{"code":"E-NAME-0","severity":"error","message":"undefined write target","span":{"start":138,"end":139}},{"code":"E-NAME-0","severity":"error","message":"undefined write target","span":{"start":146,"end":147}},{"code":"W-CUT-0","severity":"warning","message":"Cut is undeclared: all declared Actors are included; exclusions are unknown","span":null}],"bounds":{"cases":null,"live_tasks":null,"clock":null,"truncated":null},"properties":[],"terminals":[]}
"#,
        stderr: r#"U-SEM-0: duplicate finish target
E-NAME-0: undefined write target
E-NAME-0: undefined write target
W-CUT-0: Cut is undeclared: all declared Actors are included; exclusions are unknown
"#,
        exit: 3,
    },
    Legacy {
        name: r#"fix3_start_undefined_twice"#,
        base: DUP,
        change: Some((r#"finish { z = ok }"#, r#"start { z = ok; z = ok }"#)),
        stdout: r#"{"actors":null,"cut_context":{"notices":["Cut 外の存在(未知の被影響者、一度だけの拒否、登録されていない当事者)が作用しないとは主張しない","この結果を運用の範囲決定に使う場合は採用経路を別に追うこと"],"reentry":[],"horizon":[]},"agency":null,"verdict":"invalid","mode":"exhaustive","unit":"min","model":{"name":"M","sha256":"2028fd754bdead9b212409979302c61bb87d86e1c14c52125c9b7ada1a3e16e0"},"cut":"undeclared","assumptions":[],"diagnostics":[{"code":"E-NAME-0","severity":"error","message":"undefined write target","span":{"start":137,"end":138}},{"code":"U-SEM-0","severity":"error","message":"duplicate start target","span":{"start":145,"end":146}},{"code":"E-NAME-0","severity":"error","message":"undefined write target","span":{"start":145,"end":146}},{"code":"W-CUT-0","severity":"warning","message":"Cut is undeclared: all declared Actors are included; exclusions are unknown","span":null}],"bounds":{"cases":null,"live_tasks":null,"clock":null,"truncated":null},"properties":[],"terminals":[]}
"#,
        stderr: r#"E-NAME-0: undefined write target
U-SEM-0: duplicate start target
E-NAME-0: undefined write target
W-CUT-0: Cut is undeclared: all declared Actors are included; exclusions are unknown
"#,
        exit: 3,
    },
];

#[path = "support/ledger_shape.rs"]
mod ledger_shape;
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn change(s: &str, from: &str, to: &str) -> String {
    let t = s.replace(from, to);
    assert_ne!(s, t, "replacement must affect the control: {from}");
    t
}

fn run(name: &str, source: &str) -> (String, String, i32) {
    let path = root().join("target").join(format!(
        "aobs-review-fix-{}-{name}.relca",
        std::process::id()
    ));
    std::fs::write(&path, source).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_relca"))
        .arg("check")
        .arg(&path)
        .current_dir(root())
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&path);
    (
        String::from_utf8(output.stdout).unwrap(),
        String::from_utf8(output.stderr).unwrap(),
        output.status.code().unwrap(),
    )
}

fn legacy(prefix: &str) {
    let rows: Vec<_> = LEGACY
        .iter()
        .filter(|r| r.name.starts_with(prefix))
        .collect();
    assert!(!rows.is_empty());
    for row in rows {
        let source = match row.change {
            Some((from, to)) => change(row.base, from, to),
            None => row.base.to_string(),
        };
        assert!(
            !source.contains("state "),
            "{}: legacy rows must not declare Actor state",
            row.name
        );
        let (stdout, stderr, exit) = run(row.name, &source);
        let expected = ledger_shape::extend_raw(
            row.stdout,
            &ledger_shape::Spec::prelower("exhaustive", false),
        );
        assert_eq!(stdout, expected, "{}: stdout", row.name);
        assert_eq!(stderr, row.stderr, "{}: stderr", row.name);
        assert_eq!(exit, row.exit, "{}: exit", row.name);
    }
}

fn verdict(stdout: &str) -> Value {
    serde_json::from_str(stdout).unwrap()
}

/// Fix 1: a bare requires name that is the owner's latent keeps the old
/// diagnostics (E-NAME-0 for `==`, E-SYN-0 for `<`).
#[test]
fn fix1_legacy_bare_requires_on_owner_latent_matches_old_binary() {
    legacy("fix1_");
}

/// Fix 1: the ADR 0032 form (bare name of the owner's observed state) still
/// passes.
#[test]
fn fix1_bare_requires_on_actor_observed_state_still_satisfied() {
    for (name, source) in [
        ("fix1-observed-eq", OBSERVED_REQ.to_string()),
        (
            "fix1-observed-lt",
            change(OBSERVED_REQ, "requires n == 0", "requires n < 1"),
        ),
    ] {
        let (stdout, _, exit) = run(name, &source);
        assert_eq!(exit, 0, "{name}: {stdout}");
        assert_eq!(verdict(&stdout)["verdict"], "satisfied", "{name}");
    }
}

/// Fix 2: a qualified responsibility comparison other than `==` keeps the old
/// parser diagnostic in models without Actor state.
#[test]
fn fix2_legacy_qualified_noneq_responsibility_matches_old_binary() {
    legacy("fix2_");
}

/// Fix 2: the same comparisons with Actor state get the old parser
/// diagnostic too; `==` stays accepted.
#[test]
fn fix2_state_qualified_noneq_responsibility_keeps_old_parse_diagnostic() {
    let (stdout, _, exit) = run("fix2-state-eq", STATE_RESP);
    assert_eq!(exit, 0, "{stdout}");
    assert_eq!(verdict(&stdout)["verdict"], "satisfied");
    for (i, to) in [
        "Doc.level < 1",
        "Doc.level <= 1",
        "Doc.level != 1",
        "Doc.level > 1",
        "Doc.level >= 1",
    ]
    .into_iter()
    .enumerate()
    {
        let source = change(STATE_RESP, "Doc.level == 1", to);
        let at = source.find(to).unwrap() + "Doc.level ".len();
        let (stdout, stderr, exit) = run(&format!("fix2-state-{i}"), &source);
        assert_eq!(exit, 4, "{to}: {stdout}");
        let v = verdict(&stdout);
        assert_eq!(v["verdict"], "unsupported", "{to}");
        let errors: Vec<_> = v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|d| d["severity"] == "error")
            .collect();
        assert_eq!(
            errors,
            vec![&serde_json::json!({
                "code": "U-SYN-0",
                "severity": "error",
                "message": "unsupported syntax: responsibility target expression",
                "span": {"start": at, "end": at + 1}
            })],
            "{to}"
        );
        assert!(
            stderr.starts_with("U-SYN-0: unsupported syntax: responsibility target expression\n"),
            "{to}: {stderr}"
        );
    }
}

/// Fix 3: an undefined name written twice keeps U-SEM-0 duplicate.
#[test]
fn fix3_legacy_undefined_duplicate_target_matches_old_binary() {
    legacy("fix3_");
}
