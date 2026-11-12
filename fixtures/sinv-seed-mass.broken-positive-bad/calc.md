# sinv-seed-mass.broken-positive-bad — independent hand oracle

ADR 0037; design v0.2 §11.1; design v0.2 §12.4; design v0.2 §22.3. All times/work are minutes.

Only positive-mass good initializes in control; placeholder bad(ValueId0) is not a state. Fault has bad mass1/2 and good mass1/2, so bad receipt violates. No probability field is added to the invariant. Both end0/work0.

Single edit: '{ bad: 0, good: 1 }' → '{ bad: 0.5, good: 0.5 }'
Fault/control oracle: {"verdict": "violated", "diagnostics": [], "bounds": {"truncated": false}, "properties": [{"id": "contract Controls #1 invariant", "verdict": "violated"}], "terminals": [{"end": "done", "elapsed": {"min": 0, "max": 0}, "work": {"min": 0, "max": 0}, "result_at": null}]}

Expected data was frozen before implementation. A failing pre-feature run is not semantic verification.
