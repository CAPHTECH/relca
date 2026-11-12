"""Execute the real refusal factory and CLI emission with test-only input faults.

Production files are read, never edited. A source-bound copy is built offline in
the worktree. Two refusal-input insertions and a private-IR adapter are added;
none replaces validation, serialization, diagnostic handling, or the exit
expression under test.
"""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile


ROOT = Path(__file__).resolve().parents[4]
TMP = ROOT / ".tmp"
assert TMP.is_dir()
SANDBOX = Path(tempfile.mkdtemp(prefix="q3-refusal-", dir=TMP))


def insert_once(path, needle, addition):
    source = path.read_text()
    assert source.count(needle) == 1, f"test injection seam changed: {path.name}"
    path.write_text(source.replace(needle, addition))


for name in ["Cargo.toml", "Cargo.lock"]:
    shutil.copyfile(ROOT / name, SANDBOX / name)
hashes = {}
for crate in ["relca-core", "relca-cli"]:
    source = ROOT / "crates" / crate
    target = SANDBOX / "crates" / crate
    target.mkdir(parents=True)
    shutil.copyfile(source / "Cargo.toml", target / "Cargo.toml")
    shutil.copytree(source / "src", target / "src")
    for path in sorted((source / "src").rglob("*.rs")):
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        copied = target / path.relative_to(source)
        assert hashlib.sha256(copied.read_bytes()).hexdigest() == digest
        hashes[str(path.relative_to(ROOT))] = digest

# Corrupt an actual ledger before the production factory seals it. This changes
# one value, not complete/validate or the factory's handling of diagnostics.
settings = SANDBOX / "crates/relca-core/src/ledger/settings.rs"
needle = "let mut ledger = Ledger::aggregate(mode);"
insert_once(settings, needle, needle + '''
    if std::env::var_os("RELCA_Q3_TEST_BAD_LEDGER").is_some() {
        ledger.producer["version"] = serde_json::json!("");
    }
''')

# Ordinary refusal inputs are already invalid/unsupported: both old and new
# verdicts have the same exit. Supply a successful predecessor as a second,
# explicitly artificial pair to observe whether main uses the sealed verdict.
main = SANDBOX / "crates/relca-cli/src/main.rs"
needle = "let result = relca_core::ledger::refusal(mode, verdict);"
insert_once(main, needle, '''let verdict = if std::env::var_os("RELCA_Q3_TEST_PREDECESSOR").is_some() {
        Verdict::Satisfied
    } else { verdict };
    ''' + needle)

# Lower currently rejects Error diagnostics itself. Exercise analyze_model's
# existing defense-in-depth branch with an alternate lowerer, not a weakened
# public DSL. The adapter only supplies inputs and observes actual explore calls.
report = SANDBOX / "crates/relca-core/src/report.rs"
with report.open("a") as out:
    out.write('''
pub fn q3_test_preexploration(src: &str, refuse: bool) -> serde_json::Value {
    let calls = std::cell::Cell::new(0usize);
    let r = check_with(src, syntax::parse, |ast| {
        let mut m = lower::lower(ast)?;
        if refuse {
            m.verify.clear();
            m.diagnostics.push(diag::Diagnostic {
                code: diag::U_SEM_0.into(), severity: diag::Severity::Error,
                message: "injected pre-exploration refusal".into(), span: None,
            });
        }
        Ok(m)
    }, |m| { calls.set(calls.get() + 1); engine::explore(m) });
    let r = finite_driver(r, engine::DEFAULT_STATE_BUDGET);
    serde_json::json!({"explore_calls":calls.get(),"report":r})
}
''')
needle = "fn main() {"
insert_once(main, needle, needle + '''
    if let Some(source) = std::env::var_os("RELCA_Q3_TEST_PREEXPLORATION") {
        let src = std::fs::read_to_string(source).unwrap();
        let refuse = std::env::var_os("RELCA_Q3_TEST_REFUSE").is_some();
        println!("{}", report::q3_test_preexploration(&src, refuse));
        return;
    }
''')

env = dict(os.environ)
env.update(CARGO_NET_OFFLINE="true", CARGO_PROFILE_DEV_DEBUG="0",
           CARGO_PROFILE_TEST_DEBUG="0", CARGO_INCREMENTAL="0", CARGO_BUILD_JOBS="2",
           CARGO_TARGET_DIR=str(SANDBOX / "target"), TMPDIR=str(TMP))
for name in ["RELCA_Q3_TEST_BAD_LEDGER", "RELCA_Q3_TEST_PREDECESSOR", "RELCA_Q3_TEST_PREEXPLORATION", "RELCA_Q3_TEST_REFUSE", "CARGO_ENCODED_RUSTFLAGS", "RUSTFLAGS"]:
    env.pop(name, None)
with (SANDBOX / "build.log").open("wb") as log:
    build = subprocess.run(["cargo", "build", "--offline", "--locked", "--quiet",
                            "--manifest-path", str(SANDBOX / "Cargo.toml"),
                            "-p", "relca-cli", "--bin", "relca"],
                           cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT)
assert build.returncode == 0, f"probe build failed: {SANDBOX / 'build.log'}"
rows = []
for mode, args in [("verify", ["verify"]), ("cli_refusal", [])]:
    for predecessor in [False, True]:
        for corrupt in [False, True]:
            run_env = dict(env)
            if predecessor:
                run_env["RELCA_Q3_TEST_PREDECESSOR"] = "1"
            if corrupt:
                run_env["RELCA_Q3_TEST_BAD_LEDGER"] = "1"
            run = subprocess.run([str(SANDBOX / "target/debug/relca"), *args],
                                 cwd=ROOT, env=run_env, capture_output=True)
            rows.append(dict(mode=mode, predecessor=predecessor, corrupt=corrupt,
                             exit=run.returncode, stdout=run.stdout.decode(),
                             stderr=run.stderr.decode(), value=json.loads(run.stdout)))

preexploration = []
for refuse in [False, True]:
    run_env = dict(env)
    run_env["RELCA_Q3_TEST_PREEXPLORATION"] = str(ROOT / "fixtures/serial-time/model.relca")
    if refuse:
        run_env["RELCA_Q3_TEST_REFUSE"] = "1"
    run = subprocess.run([str(SANDBOX / "target/debug/relca")], cwd=ROOT,
                         env=run_env, capture_output=True)
    assert run.returncode == 0, run.stderr.decode()
    preexploration.append(dict(refuse=refuse, **json.loads(run.stdout)))

# Detect accidental changes to the originals, including during compilation.
for name, digest in hashes.items():
    assert hashlib.sha256((ROOT / name).read_bytes()).hexdigest() == digest
result = dict(sandbox=str(SANDBOX.relative_to(ROOT)), source_sha256=hashes,
              rows=rows, preexploration=preexploration)
(SANDBOX / "observations.json").write_text(json.dumps(result, indent=2) + "\n")
print(json.dumps(result))
