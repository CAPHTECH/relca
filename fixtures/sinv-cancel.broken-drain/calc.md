# sinv-cancel.broken-drain — independent hand oracle

ADR 0037; design v0.2 §11.1; design v0.2 §12.4; design v0.2 §22.3. All times/work are minutes.

Fast finishes1, chosen result_at1. Slow ack/end2 suppresses finish write, work=1+2=3. Forbidden cancellation drains to5 and writes true, work=1+5=6. Rejected lexical end is unreachable (W-UNREACH-2). Immediate ack variant ends1/work2, separately tested.

Single edit: 'cancel cooperative ack_within 1min' → 'cancel forbidden_after_start'
Fault/control oracle: {"verdict": "violated", "diagnostics": [{"code": "W-UNREACH-2"}], "bounds": {"truncated": false}, "properties": [{"id": "contract Controls #1 invariant", "verdict": "violated"}], "terminals": [{"end": "done", "elapsed": {"min": 5, "max": 5}, "work": {"min": 6, "max": 6}, "result_at": {"min": 1, "max": 1}}]}

Expected data was frozen before implementation. A failing pre-feature run is not semantic verification.
