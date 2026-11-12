# sinv-label — independent hand oracle

ADR 0037; design v0.2 §11.1; design v0.2 §12.4; design v0.2 §22.3. All times/work are minutes.

Both labels use exactly one shared state bank: first post completes1; other refused1; joint work1, per-case work0..1, both end1. posted never exceeds1. document→person cannot change resource ownership. A ledger third variant is also tested.

Control: {"verdict": "satisfied", "diagnostics": [], "bounds": {"truncated": false}, "properties": [{"id": "contract Controls #1 invariant", "verdict": "satisfied"}, {"id": "state_in_range", "verdict": "satisfied"}]}

Expected data was frozen before implementation. A failing pre-feature run is not semantic verification.
