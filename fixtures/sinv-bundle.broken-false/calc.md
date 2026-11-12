# sinv-bundle.broken-false — independent hand oracle

ADR 0037; design v0.2 §11.1; design v0.2 §12.4; design v0.2 §22.3. All times/work are minutes.

Both assignments form one finish bundle at1. Before (false,false) inactive; after (true,true) safe. Never inspect (true,false) midpoint. Fault final (true,false) violates at1. Work/end1 unchanged.

Single edit: 'b = true' → 'b = false'
Fault/control oracle: {"verdict": "violated", "diagnostics": [], "bounds": {"truncated": false}, "properties": [{"id": "contract Controls #1 invariant", "verdict": "violated"}], "terminals": [{"end": "done", "elapsed": {"min": 1, "max": 1}, "work": {"min": 1, "max": 1}, "result_at": null}]}

Expected data was frozen before implementation. A failing pre-feature run is not semantic verification.
