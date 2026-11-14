#!/usr/bin/env python3
"""ADR 0072: old invocation bytes, except the one new ledger limit field.

No capture/update mode. Golden files and the existing snapshot runner are read
only. This is regression evidence, not the hand-value oracle.
"""
import argparse
from concurrent.futures import ThreadPoolExecutor
import gzip
import json
from pathlib import Path
import re
import subprocess
import sys

import snapshots


KEY = re.compile(rb'"agency_limit_scale":')


def remove_new_field(raw, *, require_field=True):
    node = json.loads(raw)
    expected = []

    def visit(value):
        if isinstance(value, dict):
            ledger = value.get("ledger")
            if isinstance(ledger, dict):
                limits = ledger.get("execution", {}).get("limits", {})
                if isinstance(limits, dict):
                    if require_field:
                        assert "agency_limit_scale" in limits, "missing ledger limit field"
                    if "agency_limit_scale" in limits:
                        record = limits["agency_limit_scale"]
                        assert isinstance(record, dict), record
                        if record["status"] == "known":
                            assert record["value"] == 1, record
                        else:
                            assert record["status"] == "not_applicable" and record.get("value") is None, record
                        expected.append(record)
            for child in value.values():
                visit(child)
        elif isinstance(value, list):
            for child in value:
                visit(child)

    visit(node)
    matches = list(KEY.finditer(raw))
    assert len(matches) == len(expected), "a same-named non-ledger key must not be erased"
    # Decode only the new property's value; leave every other byte untouched.
    for match in reversed(matches):
        start, value_start = match.start(), match.end()
        text = raw[value_start:].decode("utf-8")
        record, end_chars = json.JSONDecoder().raw_decode(text)
        assert record in expected
        end = value_start + len(text[:end_chars].encode("utf-8"))
        if raw[end:end+1] == b",":
            end += 1
        elif raw[start-1:start] == b",":
            start -= 1
        raw = raw[:start] + raw[end:]
    json.loads(raw)
    return raw


def self_test():
    old = b'{"ledger":{"execution":{"limits":{"z":7}}},"text":"keep  spacing"}\n'
    new = old.replace(b'"z":7', b'"agency_limit_scale":{"status":"known","value":1,"reason":null},"z":7')
    assert remove_new_field(new) == old
    for bad in [new.replace(b'"z":7', b'"z":8'), new.replace(b"spacing", b"changed")]:
        assert remove_new_field(bad) != old, "unrelated stdout fault escaped"
    try:
        remove_new_field(old)
    except AssertionError:
        pass
    else:
        raise AssertionError("missing field escaped")


def trial(binary, job):
    paths = [snapshots.SAVED / (job["id"] + suffix) for suffix in (".snap", ".snap.gz")]
    paths = [p for p in paths if p.is_file()]
    assert len(paths) == 1, job["id"]
    path = paths[0]
    raw = path.read_bytes()
    saved = snapshots.unpack(gzip.decompress(raw) if path.suffix == ".gz" else raw)
    output = subprocess.run([str(binary), *job["args"]], cwd=snapshots.ROOT, capture_output=True, check=False)
    live = (remove_new_field(output.stdout), output.stderr, f"{output.returncode}\n".encode("ascii"))
    errors = snapshots.compare(job, saved, live)
    if errors:
        raise AssertionError("\n".join(errors))
    return job["id"]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path)
    parser.add_argument("--only", help="one existing snapshot job, for fault calibration")
    parser.add_argument("--legacy-control", action="store_true", help="verify original byte predicate without requiring the new field")
    options = parser.parse_args()
    self_test()
    if options.binary is None:
        print("self-test: stdout changes and missing field are caught")
        return 0
    jobs, _ = snapshots.jobs()
    assert json.loads((snapshots.SAVED / "jobs.json").read_bytes()) == jobs
    if options.only:
        jobs = [j for j in jobs if j["id"] == options.only]
        assert len(jobs) == 1
    binary = options.binary.resolve()
    if options.legacy_control:
        # Separate control test, never the new-field acceptance. It accepts the
        # old binary and must fail after a temporary production fault.
        for job in jobs:
            paths = [snapshots.SAVED / (job["id"] + s) for s in (".snap", ".snap.gz")]
            path = next(p for p in paths if p.is_file())
            raw = path.read_bytes()
            saved = snapshots.unpack(gzip.decompress(raw) if path.suffix == ".gz" else raw)
            output = subprocess.run([str(binary), *job["args"]], cwd=snapshots.ROOT, capture_output=True, check=False)
            assert not snapshots.compare(job, saved, (output.stdout, output.stderr, f"{output.returncode}\n".encode()))
        print(f"legacy control: {len(jobs)} complete stdout/stderr/exit byte comparisons")
    else:
        # Evaluate all jobs, including the two response-control L outputs. A
        # failure must not stop the census after its first missing ledger field.
        failures = []
        def check(job):
            try:
                return trial(binary, job), None
            except (AssertionError, ValueError, KeyError) as error:
                return job["id"], str(error)
        with ThreadPoolExecutor(max_workers=4) as pool:
            for name, error in pool.map(check, jobs):
                if error is not None:
                    failures.append((name, error))
        for name, error in failures:
            print(f"RED {name}: {error}")
        print(f"preservation: {len(jobs)} jobs; {len(failures)} failures")
        return int(bool(failures))
    return 0


if __name__ == "__main__":
    sys.exit(main())
