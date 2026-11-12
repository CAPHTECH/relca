# sinv-clock.broken-truncated — independent hand oracle

ADR 0037; design v0.2 §11.1; design v0.2 §12.4; design v0.2 §22.3. All times/work are minutes.

Approval finishes at minute 2 with pass or fail (not probabilities). The fail guard ends returned at elapsed/work 2/2. Pass alone posts during [2,3], so posted elapsed/work is 3/3. registered is false at receipt, hence no activation there. At post finish registered=true and approval=pass. No invariant diagnostic is emitted. Clock2 permits approval finish and the returned end; post cannot finish at3. Incomplete exploration cannot report vacuous; no unreachable warnings on a cut.

Single edit: 'clock 10min' → 'clock 2min'
Fault/control oracle: {"verdict": "inconclusive", "diagnostics": [], "bounds": {"truncated": true}, "properties": [{"id": "contract Controls #1 invariant", "verdict": "inconclusive"}], "terminals": [{"end": "returned", "elapsed": {"min": 2, "max": 2}, "work": {"min": 2, "max": 2}, "result_at": null}]}

Expected data was frozen before implementation. A failing pre-feature run is not semantic verification.
