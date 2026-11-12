#!/usr/bin/env python3
"""Current CLI golden bytes, not an oracle of semantic correctness (ADR 0055)."""

import argparse
from concurrent.futures import ThreadPoolExecutor
import gzip
import json
import os
from pathlib import Path
import shlex
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parent.parent
SAVED = ROOT / "crates/relca-cli/tests/snapshots"
WORKERS = 4  # At most four CLI processes, including during regeneration.
THRESHOLD = 30_000_000


def require(condition, message):
    if not condition:
        raise ValueError(message)


def invocation(model, data):
    data = dict(data)
    command = data.pop("command", "check")
    args = [command, model]
    allowed = {
        "check": {"configuration", "scenario"},
        "explain": {"scenario"},
        "simulate": {"configuration", "scenario"},
        "mutate": {"set", "scenario"},
        "diff": {"from", "to", "scenario"},
        "compare": {"plans", "mode", "scenario"},
    }
    require(command in allowed, f"unknown fixture command: {command}")
    require(data.keys() <= allowed[command], f"unknown invocation fields: {data}")
    for key in ("configuration", "from", "to", "plans", "mode", "set", "scenario"):
        if key in data:
            value = data[key]
            if key == "plans":
                require(isinstance(value, list), "plans must be an array")
                value = ",".join(value)
            require(isinstance(value, str) and value, f"invalid selector: {key}")
            args.extend(["--" + key, value])
    return args


def kinds(jobs):
    found = set()
    for job in jobs:
        args = job["args"]
        command = args[0]
        found.add(command)
        for flag in ("configuration", "scenario"):
            found.add(flag + (":present" if "--" + flag in args else ":absent"))
        if command == "mutate":
            found.add("mutate:set" if "--set" in args else "mutate:default")
        if command == "compare":
            found.add("compare:" + args[args.index("--mode") + 1])
    return found


REQUIRED = {
    "check", "explain", "simulate", "mutate", "diff", "compare", "verify", "conform",
    "mutate:default", "mutate:set", "compare:exhaustive", "compare:simulation",
    "configuration:present", "configuration:absent", "scenario:present", "scenario:absent",
    "sensitivity",
}


def jobs():
    result = []
    for directory in sorted((ROOT / "fixtures").iterdir()):
        if not directory.is_dir():
            continue
        require((directory / "model.relca").is_file(), f"missing model: {directory}")
        path = directory / "invocation.json"
        data = json.loads(path.read_bytes()) if path.exists() else {}
        model = (directory / "model.relca").relative_to(ROOT).as_posix()
        result.append({"id": "fixture-" + directory.name, "args": invocation(model, data)})
    for manifest in ("commands.json", "commands-phase1.json"):
        rows = json.loads((ROOT / "examples/invoice" / manifest).read_bytes())
        for row in rows:
            result.append({"id": "invoice-" + row["id"], "args": row["args"],
                           "expected_exit": row["expected_exit"]})
    # Only missing command kinds may add jobs. Never inherit historical argv.
    extra = [
        ("verify", ["verify", "fixtures/serial-time/model.relca"]),
        ("mutate:set", ["mutate", "fixtures/serial-time/model.relca", "--set", "bpr-default"]),
        ("mutate:default", ["mutate", "fixtures/serial-time/model.relca"]),
    ]
    reasons = []
    for kind, args in extra:
        if kind not in kinds(result):
            name = "coverage-" + kind.replace(":", "-")
            result.append({"id": name, "args": args})
            reasons.append(f"{name}: existing fixture/invoice invocations lack {kind}.")
    for row in json.loads((ROOT / "scripts/snapshot-extra-jobs.json").read_bytes()):
        require(set(row) <= {"id", "args", "expected_exit", "expected_verdict", "expected_error_codes", "reason"},
                "unknown extra-job contract")
        result.append({key: value for key, value in row.items() if key != "reason"})
        reasons.append(row["id"] + ": " + row["reason"])
    require(REQUIRED <= kinds(result), f"missing command kinds: {REQUIRED - kinds(result)}")
    require(len({r["id"] for r in result}) == len(result), "duplicate snapshot id")
    require(len({tuple(r["args"]) for r in result}) == len(result), "duplicate ordered argv")
    return result, reasons


def capture(binary, job):
    env = dict(os.environ, LC_ALL="C", LANG="C", CARGO_NET_OFFLINE="true")
    output = subprocess.run([str(binary), *job["args"]], cwd=ROOT, env=env,
                            capture_output=True, check=False)
    require(0 <= output.returncode <= 5, f"abnormal CLI exit: {job}: {output.returncode}")
    if "expected_exit" in job:
        require(output.returncode == job["expected_exit"], f"declared invoice exit differs: {job['id']}")
    if "expected_verdict" in job:
        value = json.loads(output.stdout)
        require(value.get("verdict") == job["expected_verdict"], f"hand verdict differs: {job['id']}")
        if "expected_error_codes" in job:
            def errors(node):
                if isinstance(node, dict):
                    if node.get("severity") == "error" and isinstance(node.get("code"), str):
                        yield node["code"]
                    for child in node.values():
                        yield from errors(child)
                elif isinstance(node, list):
                    for child in node:
                        yield from errors(child)
            require(set(errors(value)) == set(job["expected_error_codes"]),
                    f"hand error classification differs: {job['id']}")
        refusal_reason_check(job, value)
    return output.stdout, output.stderr, f"{output.returncode}\n".encode("ascii")


def refusal_reason_check(job, value):
    # Hand contracts restored from the two waits_integration refusal families,
    # not from saved stdout. Keep the golden job/argv manifest unchanged.
    if job["id"].startswith("wait-refusal-revocation-"):
        code, reason = "E-SIM-0", "explicit simulation clause"
    elif job["id"].startswith("wait-refusal-completion-"):
        code, reason = "U-SEM-0", "structural completion"
    else:
        return
    require(any(d.get("code") == code and d.get("severity") == "error"
                and reason in d.get("message", "") for d in value.get("diagnostics", [])),
            f"hand refusal reason differs: {job['id']}: {reason}")


def pack(channels):
    return (" ".join(str(len(c)) for c in channels) + "\n").encode("ascii") + b"".join(channels)


def unpack(raw):
    header, data = raw.split(b"\n", 1)
    lengths = [int(n) for n in header.split()]
    require(len(lengths) == 3 and all(n >= 0 for n in lengths), "bad snapshot header")
    require(sum(lengths) == len(data), "truncated or extended snapshot")
    result = []
    start = 0
    for length in lengths:
        result.append(data[start:start + length])
        start += length
    require(result[2] in [f"{n}\n".encode() for n in range(6)], "bad saved exit")
    return tuple(result)


def compare(job, expected, actual):
    errors = []
    for channel, saved, live in zip(("stdout", "stderr", "exit"), expected, actual):
        if saved == live:
            continue
        offset = next((i for i, (a, b) in enumerate(zip(saved, live)) if a != b),
                      min(len(saved), len(live)))
        errors.append(f"{job['id']} {channel}: byte {offset}; expected_len={len(saved)} actual_len={len(live)}\n"
                      f"  command: {shlex.join(job['args'])}\n"
                      f"  expected: {saved[max(0, offset-48):offset+96]!r}\n"
                      f"  actual:   {live[max(0, offset-48):offset+96]!r}")
    return errors


def self_test():
    job = {"id": "calibration", "args": ["check", "model.relca"]}
    original = (b"{}\n", b"", b"0\n")
    require(unpack(pack(original)) == original, "lossy framing")
    require(not compare(job, original, original), "identity rejected")
    for changed in [(b"{} ", b"", b"0\n"), (b"{}\n", b"\n", b"0\n"),
                    (b"{}\n", b"", b"1\n")]:
        require(bool(compare(job, original, changed)), "channel fault escaped")
    for raw in (pack(original)[:-1], pack(original) + b"x"):
        try:
            unpack(raw)
        except ValueError:
            pass
        else:
            raise ValueError("corrupt framing accepted")
    for family, code, reason in (
        ("revocation", "E-SIM-0", "explicit simulation clause"),
        ("completion", "U-SEM-0", "structural completion"),
    ):
        refusal = {"id": f"wait-refusal-{family}-calibration"}
        value = {"diagnostics": [{"code": code, "severity": "error", "message": reason}]}
        refusal_reason_check(refusal, value)
        value["diagnostics"][0]["message"] = "unrelated refusal with the same code"
        try:
            refusal_reason_check(refusal, value)
        except ValueError as error:
            require("hand refusal reason differs" in str(error), "wrong reason predicate")
        else:
            raise ValueError("refusal reason fault escaped")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path)
    parser.add_argument("--update", action="store_true")
    parser.add_argument("--only", help="one job for explicit local calibration")
    parser.add_argument("--show", help="write one saved job's raw stdout without execution")
    options = parser.parse_args()
    self_test()
    started = time.monotonic()
    planned, reasons = jobs()
    if options.show:
        require(not options.update and options.only is None, "show cannot update or select execution")
        require(options.show in {j['id'] for j in planned}, "unknown saved job")
        paths = [SAVED / (options.show + suffix) for suffix in ('.snap', '.snap.gz')]
        paths = [p for p in paths if p.is_file()]
        require(len(paths) == 1, "missing/duplicate saved job")
        raw = paths[0].read_bytes()
        channels = unpack(gzip.decompress(raw) if paths[0].suffix == '.gz' else raw)
        sys.stdout.buffer.write(channels[0])
        return 0
    require(options.binary is not None, "execution requires --binary")
    if options.update:
        require(options.only is None, "partial update is forbidden")
    else:
        manifest = json.loads((SAVED / "jobs.json").read_bytes())
        require(manifest == planned, "snapshot input set changed; review and regenerate")
    selected = [j for j in planned if options.only is None or j["id"] == options.only]
    require(selected, "unknown selected job")
    with ThreadPoolExecutor(max_workers=WORKERS) as pool:
        outputs = list(pool.map(lambda j: capture(options.binary.resolve(), j), selected))
    total = sum(len(c) for channels in outputs for c in channels)
    if options.update:
        compressed = total > THRESHOLD
        suffix = ".snap.gz" if compressed else ".snap"
        SAVED.mkdir(parents=True, exist_ok=True)
        expected_files = {j["id"] + suffix for j in selected}
        # Remove only obsolete golden files in this dedicated directory.
        for path in SAVED.iterdir():
            if path.name.endswith((".snap", ".snap.gz")) and path.name not in expected_files:
                path.unlink()
        for job, channels in zip(selected, outputs):
            raw = pack(channels)
            (SAVED / (job["id"] + suffix)).write_bytes(gzip.compress(raw, mtime=0) if compressed else raw)
        (SAVED / "jobs.json").write_text(json.dumps(planned, ensure_ascii=False, indent=2) + "\n")
        (SAVED / "additional-jobs.txt").write_text("\n".join(reasons) + "\n")
    else:
        all_files = {p.name for p in SAVED.iterdir() if p.name.endswith((".snap", ".snap.gz"))}
        raw_names = {j["id"] + ".snap" for j in planned}
        gzip_names = {j["id"] + ".snap.gz" for j in planned}
        require(all_files in (raw_names, gzip_names), "missing/extra/mixed-format golden files")
        suffix = ".snap" if all_files == raw_names else ".snap.gz"
        errors = []
        for job, live in zip(selected, outputs):
            raw = (SAVED / (job["id"] + suffix)).read_bytes()
            saved = unpack(gzip.decompress(raw) if suffix.endswith(".gz") else raw)
            errors.extend(compare(job, saved, live))
        if errors:
            print("\n".join(errors[:20]), file=sys.stderr)
            print(f"snapshot: {len(errors)} changed channels; review before running scripts/update-snapshots.sh",
                  file=sys.stderr)
            return 1
    stored = sum(p.stat().st_size for p in SAVED.iterdir() if p.name.endswith((".snap", ".snap.gz")))
    print(f"snapshot: jobs={len(selected)} channels={3*len(selected)} raw_bytes={total} "
          f"stored_bytes={stored} workers={WORKERS} seconds={time.monotonic()-started:.3f}")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (ValueError, OSError, KeyError, TypeError) as error:
        print(f"snapshot: {error}", file=sys.stderr)
        sys.exit(1)
