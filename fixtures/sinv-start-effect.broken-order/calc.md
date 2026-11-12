# sinv-start-effect.broken-order — independent hand oracle

ADR 0037; design v0.2 §11.1; design v0.2 §12.4; design v0.2 §22.3. All times/work are minutes.

Control running0..2 has registered=false; post writes true at3 after idle. Fault post writes at1, work start1 creates running/true (bad), work finish3 repairs idle. Both elapsed/work3. Witness must be Start at1, not terminal.

Single edit: 'a = Desk.work(); b = Desk.post()' → 'b = Desk.post(); a = Desk.work()'
Fault/control oracle: {"verdict": "violated", "diagnostics": [], "bounds": {"truncated": false}, "properties": [{"id": "contract Controls #1 invariant", "verdict": "violated"}], "terminals": [{"end": "done", "elapsed": {"min": 3, "max": 3}, "work": {"min": 3, "max": 3}, "result_at": null}]}

Expected data was frozen before implementation. A failing pre-feature run is not semantic verification.
