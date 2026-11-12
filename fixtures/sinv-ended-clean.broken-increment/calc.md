# sinv-ended-clean.broken-increment — independent hand oracle

ADR 0037; design v0.2 §11.1; design v0.2 §12.4; design v0.2 §22.3. All times/work are minutes.

As sinv-ended-case, but upper2 permits both-pass ends3/4 and no range stop. Mixed histories still violate the already ended fail case at3, preserving its end2/work2.

Single edit: 'posted += 0' → 'posted += 1'
Fault/control oracle: {"verdict": "violated", "diagnostics": [], "bounds": {"truncated": false}, "properties": [{"id": "contract Controls #1 invariant", "verdict": "violated"}, {"id": "state_in_range", "verdict": "satisfied"}]}

Expected data was frozen before implementation. A failing pre-feature run is not semantic verification.
