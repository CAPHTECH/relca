# sinv-static-integer — independent hand oracle

ADR 0037; design v0.2 §11.1; design v0.2 §12.4; design v0.2 §22.3. All times/work are minutes.

count0→1 at1, safe upper1 and work1. A bare integer is not bool even if its value is0.

Control: {"verdict": "satisfied", "diagnostics": [], "bounds": {"truncated": false}, "properties": [{"id": "contract Controls #1 invariant", "verdict": "satisfied"}, {"id": "state_in_range", "verdict": "satisfied"}], "terminals": [{"end": "done", "elapsed": {"min": 1, "max": 1}, "work": {"min": 1, "max": 1}, "result_at": null}]}

Expected data was frozen before implementation. A failing pre-feature run is not semantic verification.
