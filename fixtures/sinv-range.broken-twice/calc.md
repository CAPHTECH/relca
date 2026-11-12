# sinv-range.broken-twice — independent hand oracle

ADR 0037; design v0.2 §11.1; design v0.2 §12.4; design v0.2 §22.3. All times/work are minutes.

One take changes 0 to1 at1 and ends1/work1. Second take attempts1+1=2 at2; 2 is not a reachable valuation. One range witness, boolean range_violation_stops=true, truncated=false. State invariant has no bad valuation but unexamined continuation: inconclusive.

Single edit: 'end done' → 's = Desk.take(); end done'
Fault/control oracle: {"verdict": "violated", "diagnostics": [], "bounds": {"truncated": false, "range_violation_stops": true}, "properties": [{"id": "contract Controls #1 invariant", "verdict": "inconclusive"}, {"id": "state_in_range", "verdict": "violated"}], "terminals": [], "state_range_violations": [{"time": 2, "before": "1", "attempted": "2", "lower": "0", "upper": "1"}]}

Expected data was frozen before implementation. A failing pre-feature run is not semantic verification.
