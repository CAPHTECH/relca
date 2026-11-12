# sinv-range — independent hand oracle

ADR 0037; design v0.2 §11.1; design v0.2 §12.4; design v0.2 §22.3. All times/work are minutes.

One take changes 0 to1 at1 and ends1/work1. Second take attempts1+1=2 at2; 2 is not a reachable valuation. One range witness, boolean range_violation_stops=true, truncated=false. State invariant has no bad valuation but unexamined continuation: inconclusive.

Control: {"verdict": "satisfied", "diagnostics": [], "bounds": {"truncated": false}, "properties": [{"id": "contract Controls #1 invariant", "verdict": "satisfied"}, {"id": "state_in_range", "verdict": "satisfied"}], "terminals": [{"end": "done", "elapsed": {"min": 1, "max": 1}, "work": {"min": 1, "max": 1}, "result_at": null}]}

Expected data was frozen before implementation. A failing pre-feature run is not semantic verification.
