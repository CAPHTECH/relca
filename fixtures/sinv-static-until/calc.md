# sinv-static-until — independent hand oracle

ADR 0037; design v0.2 §11.1; design v0.2 §12.4; design v0.2 §22.3. All times/work are minutes.

Control uses only declared state and has Fix1, end/work1. Single replacement is rejected statically with exactly U-SYN-0; no runtime terminal, no probability, bounds.truncated=null. Both expression sides and nested groups are tested separately.

Control: {"verdict": "satisfied", "diagnostics": [], "bounds": {"truncated": false}, "properties": [{"id": "contract Controls #1 invariant", "verdict": "satisfied"}], "terminals": [{"end": "done", "elapsed": {"min": 1, "max": 1}, "work": {"min": 1, "max": 1}, "result_at": null}]}

Expected data was frozen before implementation. A failing pre-feature run is not semantic verification.
