# sinv-rewrite.broken-reapprove — independent hand oracle

ADR 0037; design v0.2 §11.1; design v0.2 §12.4; design v0.2 §22.3. All times/work are minutes.

Approval finishes at minute 2 with pass or fail (not probabilities). The fail guard ends returned at elapsed/work 2/2. Pass alone posts during [2,3], so posted elapsed/work is 3/3. registered is false at receipt, hence no activation there. At post finish registered=true and approval=pass. No invariant diagnostic is emitted. Second approval occupies [3,5]. It may change approval to fail at 5. The event clause saw pass at 3 and remains satisfied; the state clause becomes violated at 5.

Single edit: 'end posted' → 'again = Approver.approve(); end posted'
Fault/control oracle: {"verdict": "violated", "diagnostics": [], "bounds": {"truncated": false}, "properties": [{"id": "contract Controls #1 invariant", "verdict": "violated"}, {"id": "contract Controls #2 invariant", "verdict": "satisfied"}], "terminals": [{"end": "returned", "elapsed": {"min": 2, "max": 2}, "work": {"min": 2, "max": 2}, "result_at": null}, {"end": "posted", "elapsed": {"min": 5, "max": 5}, "work": {"min": 5, "max": 5}, "result_at": null}]}

Expected data was frozen before implementation. A failing pre-feature run is not semantic verification.
