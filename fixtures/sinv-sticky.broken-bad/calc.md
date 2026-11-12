# sinv-sticky.broken-bad — independent hand oracle

ADR 0037; design v0.2 §11.1; design v0.2 §12.4; design v0.2 §22.3. All times/work are minutes.

Control calls Fix only: Bad.bad not_called, elapsed/work1. Fault Bad finishes1 with ok=false, Fix start1 repairs and ends2 with ok=true. Sticky failure survives, and task/end execution continues.

Single edit: 'r = Fix.fix()' → 'b = Bad.bad(); r = Fix.fix()'
Fault/control oracle: {"verdict": "violated", "diagnostics": [], "bounds": {"truncated": false}, "properties": [{"id": "contract Controls #1 invariant", "verdict": "violated"}], "terminals": [{"end": "done", "elapsed": {"min": 2, "max": 2}, "work": {"min": 2, "max": 2}, "result_at": null}]}

Expected data was frozen before implementation. A failing pre-feature run is not semantic verification.
