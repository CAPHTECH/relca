# sinv-sticky — independent hand oracle

ADR 0037; design v0.2 §11.1; design v0.2 §12.4; design v0.2 §22.3. All times/work are minutes.

Control calls Fix only: Bad.bad not_called, elapsed/work1. Fault Bad finishes1 with ok=false, Fix start1 repairs and ends2 with ok=true. Sticky failure survives, and task/end execution continues.

Control: {"verdict": "satisfied", "diagnostics": [{"code": "W-UNREACH-0"}], "bounds": {"truncated": false}, "properties": [{"id": "contract Controls #1 invariant", "verdict": "satisfied"}], "terminals": [{"end": "done", "elapsed": {"min": 1, "max": 1}, "work": {"min": 1, "max": 1}, "result_at": null}]}

Expected data was frozen before implementation. A failing pre-feature run is not semantic verification.
