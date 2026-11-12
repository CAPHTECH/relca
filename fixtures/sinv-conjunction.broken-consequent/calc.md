# sinv-conjunction.broken-consequent — independent hand oracle

ADR 0037; design v0.2 §11.1; design v0.2 §12.4; design v0.2 §22.3. All times/work are minutes.

po5 and receipt3 run in parallel (elapsed5, work8); post adds1 (elapsed6, work9). Only at6 registered is true, both comparisons hold. Changing right receipt constant to none violates at6. The outer implication is weaker than the unparenthesized &&.

Single edit: '&& receipt == pass' → '&& receipt == none'
Fault/control oracle: {"verdict": "violated", "diagnostics": [], "bounds": {"truncated": false}, "properties": [{"id": "contract Controls #1 invariant", "verdict": "violated"}], "terminals": [{"end": "done", "elapsed": {"min": 6, "max": 6}, "work": {"min": 9, "max": 9}, "result_at": null}]}

Expected data was frozen before implementation. A failing pre-feature run is not semantic verification.
