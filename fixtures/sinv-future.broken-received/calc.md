# sinv-future.broken-received — independent hand oracle

ADR 0037; design v0.2 §11.1; design v0.2 §12.4; design v0.2 §22.3. All times/work are minutes.

Future arrival5 is beyond clock4: no initialized state or witness, inconclusive/truncated. At clock5 receipt false violates, end5/elapsed0/work0. Future is never initialized early.

Single edit: 'clock 4min' → 'clock 5min'
Fault/control oracle: {"verdict": "violated", "diagnostics": [], "bounds": {"truncated": false}, "properties": [{"id": "contract Controls #1 invariant", "verdict": "violated"}], "terminals": []}

Expected data was frozen before implementation. A failing pre-feature run is not semantic verification.
