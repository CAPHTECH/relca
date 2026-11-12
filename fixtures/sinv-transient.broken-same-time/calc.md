# sinv-transient.broken-same-time — independent hand oracle

ADR 0037; design v0.2 §11.1; design v0.2 §12.4; design v0.2 §22.3. All times/work are minutes.

Control receipt finishes at3, po at5: max(5,3)=5 elapsed and 5+3=8 work. At equal duration3, po-first finish exposes (pass,none) at3 between bundles, then receipt repairs. Both orders must be explored; elapsed3/work6 still completes.

Single edit: 'fixed 5min' → 'fixed 3min'
Fault/control oracle: {"verdict": "violated", "diagnostics": [], "bounds": {"truncated": false}, "properties": [{"id": "contract Controls #1 invariant", "verdict": "violated"}], "terminals": [{"end": "done", "elapsed": {"min": 3, "max": 3}, "work": {"min": 6, "max": 6}, "result_at": null}]}

Expected data was frozen before implementation. A failing pre-feature run is not semantic verification.
