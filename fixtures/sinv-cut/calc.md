# sinv-cut — independent hand oracle

ADR 0037; design v0.2 §11.1; design v0.2 §12.4; design v0.2 §22.3. All times/work are minutes.

Approval finishes at minute 2 with pass or fail (not probabilities). The fail guard ends returned at elapsed/work 2/2. Pass alone posts during [2,3], so posted elapsed/work is 3/3. registered is false at receipt, hence no activation there. At post finish registered=true and approval=pass. No invariant diagnostic is emitted. Invoice is read only on the antecedent. Its routine default makes that conjunct true. Excluding this read is E-CUT-1; the reason and loses text prevent an unrelated Cut warning.

Control: {"verdict": "satisfied", "diagnostics": [], "bounds": {"truncated": false}, "properties": [{"id": "contract Controls #1 invariant", "verdict": "satisfied"}], "terminals": [{"end": "returned", "elapsed": {"min": 2, "max": 2}, "work": {"min": 2, "max": 2}, "result_at": null}, {"end": "posted", "elapsed": {"min": 3, "max": 3}, "work": {"min": 3, "max": 3}, "result_at": null}]}

Expected data was frozen before implementation. A failing pre-feature run is not semantic verification.
