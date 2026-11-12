# sinv-vacuous.broken-register — independent hand oracle

ADR 0037; design v0.2 §11.1; design v0.2 §12.4; design v0.2 §22.3. All times/work are minutes.

registered remains false throughout the complete control. Ledger.post is not_called (one W-UNREACH-0). Inserting post activates at 3 and the fail choice violates. No lexical posted end exists.

Single edit: 'end returned' → 'p = Ledger.post(); end returned'
Fault/control oracle: {"verdict": "violated", "diagnostics": [], "bounds": {"truncated": false}, "properties": [{"id": "contract Controls #1 invariant", "verdict": "violated"}], "terminals": [{"end": "returned", "elapsed": {"min": 3, "max": 3}, "work": {"min": 3, "max": 3}, "result_at": null}]}

Expected data was frozen before implementation. A failing pre-feature run is not semantic verification.
