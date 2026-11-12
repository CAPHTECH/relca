#!/usr/bin/env python3
"""ADR0058 one-transition raw delta audit, not a snapshot normalizer."""
import argparse
from collections import Counter
import gzip
import json
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[4]
SAVED = ROOT / "crates/relca-cli/tests/snapshots"
KEYS = {"schema", "producer", "backends", "derivation", "execution",
        "actor_versions", "context", "subjects"}


def require(condition, reason):
    if not condition:
        raise ValueError(reason)


def pairs(rows):
    require(len({k for k, _ in rows}) == len(rows), "duplicate JSON key")
    return dict(rows)


def owners(v):
    result = {"": v}
    if "from" in v and "to" in v:
        for side in ("from", "to"):
            result[f"/{side}/report"] = v[side]["report"]
    for n, plan in enumerate(v.get("plans", [])):
        result[f"/plans/{n}/report"] = plan["report"]
    if v.get("original") is not None:
        result["/original"] = v["original"]
    for k, kind in enumerate(v.get("mutations", [])):
        for n, mutant in enumerate(kind["mutants"]):
            if mutant.get("report") is not None:
                result[f"/mutations/{k}/mutants/{n}/report"] = mutant["report"]
    return result


def pointer(v, at):
    for key in at.lstrip("/").split("/"):
        key = key.replace("~1", "/").replace("~0", "~")
        v = v[int(key)] if isinstance(v, list) else v[key]
    return v


def check_ledgers(v):
    for at, report in owners(v).items():
        require("ledger" in report, f"LEDGER_MISSING at {at}/ledger")
        ledger = report["ledger"]
        require(set(ledger) == KEYS, f"invalid ledger keys at {at}")
        require(ledger["schema"] == "relca-conclusion-ledger/v1", "unsupported schema")
        producer = ledger["producer"]
        require(producer == {"name": "relca", "version": "0.0.0", "version_source": "cargo_package",
                            "build_identity": {"status": "not_recorded", "value": None,
                                               "reason": "package_version_only"}}, "producer mismatch")
        require(ledger["derivation"]["bundle"] == "relca-conclusions-p2-uncertainty-2", "bundle mismatch")
        require(ledger["backends"]["external"] == {"status": "not_connected", "name": None,
                                                    "version": None,
                                                    "reason": "external_backend_not_connected"},
                "external backend mismatch")
        for component in ledger["derivation"]["components"]:
            require(set(component) == {"at", "name", "version"}, "component keys")
            require(component["at"] == component["name"], "component identity")
            require(component["version"] and pointer(report, component["at"]) == component["version"],
                    "local version mismatch")
    return len(owners(v))


def erase_ledger_members(raw, report_paths):
    """Retain every pre-existing byte; parse positions, never reserialize JSON."""
    text = raw.decode("utf-8")
    decoder = json.JSONDecoder(object_pairs_hook=pairs)
    spans = []

    def space(n):
        while n < len(text) and text[n].isspace():
            n += 1
        return n

    def walk(n, path):
        n = space(n)
        if text[n] == "{":
            n = space(n + 1)
            first = True
            while text[n] != "}":
                start = n
                key, n = decoder.raw_decode(text, n)
                n = space(n)
                require(text[n] == ":", "object delimiter")
                value_start = space(n + 1)
                escaped = key.replace("~", "~0").replace("/", "~1")
                n = space(walk(value_start, path + "/" + escaped))
                comma = text[n] == ","
                if key == "ledger" and path in report_paths:
                    require(first and comma, f"ledger not first at {path}")
                    spans.append((start, n + 1))
                first = False
                if not comma:
                    break
                n = space(n + 1)
            require(text[n] == "}", "object close")
            return n + 1
        if text[n] == "[":
            n, index = space(n + 1), 0
            while text[n] != "]":
                n = space(walk(n, path + "/" + str(index)))
                index += 1
                if text[n] != ",":
                    break
                n = space(n + 1)
            require(text[n] == "]", "array close")
            return n + 1
        return decoder.raw_decode(text, n)[1]

    end = space(walk(0, ""))
    require(end == len(text), "stdout has extra content")
    require(len(spans) == len(report_paths), "missing ledger span")
    for start, end in sorted(spans, reverse=True):
        text = text[:start] + text[end:]
    return text.encode("utf-8")


def classify(before, after):
    require(before[1] == after[1], "unexpected_stderr")
    require(before[2] == after[2], "unexpected_exit")
    v = json.loads(after[0], object_pairs_hook=pairs)
    count = check_ledgers(v)
    old = json.loads(before[0], object_pairs_hook=pairs)
    if "ledger" in old:
        require(before[0] == after[0], "unexpected_stdout after current snapshot update")
        return "current_exact", count
    raw = erase_ledger_members(after[0], owners(v))
    require(raw == before[0], "unexpected_stdout outside ledger")
    return "ledger_only", count


def calibrate():
    # The host/ledger are hand-assembled contracts, not captured Relca output.
    ledger = {"schema": "relca-conclusion-ledger/v1",
              "producer": {"name": "relca", "version": "0.0.0", "version_source": "cargo_package",
                           "build_identity": {"status": "not_recorded", "value": None,
                                              "reason": "package_version_only"}},
              "backends": {"external": {"status": "not_connected", "name": None,
                                         "version": None, "reason": "external_backend_not_connected"}},
               "derivation": {"bundle": "relca-conclusions-p2-uncertainty-2", "components": []},
              "execution": {}, "actor_versions": {}, "context": {}, "subjects": []}
    payload = json.dumps(ledger, separators=(",", ":")).encode()
    old = (b'{"verdict":"satisfied","x":"ledger\\\"literal","n":8}\n', b"", b"0\n")
    new = (b'{"ledger":' + payload + b',' + old[0][1:], b"", b"0\n")
    require(classify(old, new) == ("ledger_only", 1), "valid insertion rejected")
    child = {"verdict": "satisfied", "payload": {"ledger": "not report metadata"}}
    old_parent = {"verdict": "satisfied", "from": {"report": child}, "to": {"report": child}}
    new_parent = {"ledger": ledger, "verdict": "satisfied",
                  "from": {"report": {"ledger": ledger, **child}},
                  "to": {"report": {"ledger": ledger, **child}}}
    raw = lambda v: json.dumps(v, separators=(",", ":")).encode() + b"\n"
    parent_before = (raw(old_parent), b"", b"0\n")
    parent_after = (raw(new_parent), b"", b"0\n")
    require(classify(parent_before, parent_after) == ("ledger_only", 3), "nested ownership rejected")
    faults = [(new[0].replace(b'"n":8', b'"n":9'), b"", b"0\n"),
              (new[0], b"\n", b"0\n"), (new[0], b"", b"1\n"),
              (new[0].replace(b'"version":"0.0.0"', b'"version":""'), b"", b"0\n"),
              (new[0].replace(b'"n":8', b'"n":8,"n":8'), b"", b"0\n"), old]
    for fault in faults:
        try:
            classify(old, fault)
        except ValueError:
            pass
        else:
            raise ValueError("delta fault escaped")
    del new_parent["from"]["report"]["ledger"]
    try:
        classify(parent_before, (raw(new_parent), b"", b"0\n"))
    except ValueError as error:
        require("LEDGER_MISSING" in str(error), "wrong nested-fault predicate")
    else:
        raise ValueError("missing child ledger escaped")
    print("delta calibration: positive=2 rejected_faults=7")


def unpack(path):
    raw = path.read_bytes()
    if path.suffix == ".gz":
        raw = gzip.decompress(raw)
    header, raw = raw.split(b"\n", 1)
    lengths = [int(n) for n in header.split()]
    require(len(lengths) == 3 and sum(lengths) == len(raw), "bad framing")
    a, b, c = lengths
    return raw[:a], raw[a:a+b], raw[a+b:a+b+c]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--calibrate", action="store_true")
    parser.add_argument("--binary", type=Path)
    args = parser.parse_args()
    if args.calibrate:
        calibrate()
        return
    require(args.binary is not None, "binary required")
    counts = Counter()
    for job in json.loads((SAVED / "jobs.json").read_bytes()):
        paths = [SAVED / (job["id"] + suffix) for suffix in (".snap", ".snap.gz")]
        paths = [p for p in paths if p.is_file()]
        require(len(paths) == 1, "missing/duplicate saved job")
        out = subprocess.run([str(args.binary), *job["args"]], cwd=ROOT,
                             capture_output=True, check=False)
        require(out.returncode in range(6), "abnormal termination")
        try:
            category, owners_count = classify(unpack(paths[0]),
                                              (out.stdout, out.stderr, f"{out.returncode}\n".encode()))
        except ValueError as error:
            raise ValueError(f"job={job['id']} argv={job['args']}: {error}") from error
        counts[category] += 1
        counts["owners"] += owners_count
    print(dict(counts))


if __name__ == "__main__":
    try:
        main()
    except (ValueError, KeyError) as error:
        print(error, file=sys.stderr)
        sys.exit(1)
