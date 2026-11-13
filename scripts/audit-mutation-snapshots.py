#!/usr/bin/env python3
"""ADR 0068: permit only candidate-count additions to current golden bytes."""
import argparse
import gzip
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parent.parent
SAVED = "crates/relca-cli/tests/snapshots/"


def git(*args):
    return subprocess.check_output(["git", *args], cwd=ROOT)


def unpack(raw):
    header, data = raw.split(b"\n", 1)
    lengths = list(map(int, header.split()))
    assert len(lengths) == 3 and sum(lengths) == len(data)
    a, b, _ = lengths
    return data[:a], data[a:a + b], data[a + b:]


def encode(value):
    return (json.dumps(value, ensure_ascii=False, separators=(",", ":")) + "\n").encode()


def compare(old, new):
    assert old[1:] == new[1:], "stderr or exit changed"
    before = json.loads(old[0])
    after = json.loads(new[0])
    assert before["mode"] == after["mode"] == "mutation", "non-mutate snapshot changed"
    assert encode(before) == old[0], "JSON roundtrip must preserve the original bytes"
    assert "candidates_total" not in before["run"]
    value = after["run"].pop("candidates_total")
    assert value is None or type(value) is int and value >= 0
    assert len(before["mutations"]) == len(after["mutations"]) == 10
    for row in after["mutations"]:
        value = row.pop("candidates")
        assert value is None or type(value) is int and value >= 0
    assert encode(after) == old[0], "an existing stdout byte/value changed"


def self_test():
    old = {"mode": "mutation", "run": {"completion": "complete"},
           "mutations": [{"status": "generated"} for _ in range(10)]}
    new = json.loads(json.dumps(old))
    new["run"]["candidates_total"] = 10
    for row in new["mutations"]:
        row["candidates"] = 1
    channels = (encode(old), b"diagnostic\n", b"4\n")
    live = (encode(new), channels[1], channels[2])
    compare(channels, live)
    bad = json.loads(json.dumps(new))
    bad["mutations"][0]["status"] = "satisfied"
    for fault in [(encode(bad), live[1], live[2]),
                  (live[0], b"", live[2]), (live[0], live[1], b"0\n")]:
        try:
            compare(channels, fault)
        except AssertionError:
            pass
        else:
            raise AssertionError("audit failed to detect injected existing-value/channel fault")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base", default="HEAD")
    args = parser.parse_args()
    self_test()
    # All unchanged golden files are byte-identical by git; inspect every changed
    # file, including manifest/non-mutate changes (which must be rejected).
    changed = git("diff", "--name-only", args.base, "--", SAVED).decode().splitlines()
    assert changed, "no updated snapshots to audit"
    for name in changed:
        assert name.endswith((".snap", ".snap.gz")), "snapshot manifest changed"
        old = git("show", f"{args.base}:{name}")
        new = (ROOT / name).read_bytes()
        if name.endswith(".gz"):
            old, new = gzip.decompress(old), gzip.decompress(new)
        compare(unpack(old), unpack(new))
    print(f"PASS: {len(changed)} changed mutate snapshots: only 11 count fields added; "
          "existing stdout bytes, stderr and exit identical. All other golden files unchanged.")
    print("Calibration: existing status, stderr and exit faults all rejected.")


if __name__ == "__main__":
    main()
