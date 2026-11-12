# sinv-shared — independent hand oracle

ADR 0037; design v0.2 §11.1; design v0.2 §12.4; design v0.2 §22.3. All times/work are minutes.

Two cases arrive at0. One shared slot posts at1; guard rejects the next at1 without task work. Each case may work0 or1; joint total work1, refusal1, both ends1. Removing requires allows starts0/1 and finishes1/2, work1 each, total2, no refusal table. posted=2 is inside 0..2 but violates <=1 at2. Labels never create a per-case copy.

Control: {"verdict": "satisfied", "diagnostics": [], "bounds": {"truncated": false}, "properties": [{"id": "contract Controls #1 invariant", "verdict": "satisfied"}, {"id": "state_in_range", "verdict": "satisfied"}]}

Expected data was frozen before implementation. A failing pre-feature run is not semantic verification.
