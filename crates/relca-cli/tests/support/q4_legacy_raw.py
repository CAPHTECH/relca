"""Reuse six committed current goldens. Never capture new expected bytes."""
import gzip
import importlib.util
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[4]
spec = importlib.util.spec_from_file_location("q4_current_snapshots", ROOT / "scripts/snapshots.py")
assert spec is not None and spec.loader is not None
snapshots = importlib.util.module_from_spec(spec)
spec.loader.exec_module(snapshots)

names = ("serial-time", "parallel-time", "wait-intentional", "resource-contention",
         "sim-statistical", "serial-time.broken-missing-capacity")
saved = ROOT / "crates/relca-cli/tests/snapshots"
jobs = json.loads((saved / "jobs.json").read_text())
errors = []
for name in names:
    rows = [j for j in jobs if j["id"] == "fixture-" + name]
    assert len(rows) == 1, name
    job = rows[0]
    path = saved / (job["id"] + ".snap.gz")
    expected = snapshots.unpack(gzip.decompress(path.read_bytes()))
    actual = snapshots.capture(Path(sys.argv[1]).resolve(), job)
    errors.extend(snapshots.compare(job, expected, actual))
if errors:
    raise AssertionError("\n".join(errors))
print("Q4 legacy raw controls: 6 jobs, stdout/stderr/exit unchanged")
