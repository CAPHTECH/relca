# sinv-latent.broken-policy — independent hand oracle

ADR 0037; design v0.2 §11.1; design v0.2 §12.4; design v0.2 §22.3. All times/work are minutes.

Approval finishes at minute 2 with pass or fail (not probabilities). The fail guard ends returned at elapsed/work 2/2. Pass alone posts during [2,3], so posted elapsed/work is 3/3. registered is false at receipt, hence no activation there. At post finish registered=true and approval=pass. No invariant diagnostic is emitted. Latent bool has two nondeterministic receipt valuations, not a uniform distribution. false/pass reaches a bad registered state. Reading truth in the process guard is E-POL-LATENT.

Single edit: 'if approval == fail' → 'if extraction_correct == true'
Fault/control oracle: {"verdict": "invalid", "diagnostics": [{"code": "E-POL-LATENT"}], "bounds": {"truncated": null}, "properties": [], "terminals": []}

Expected data was frozen before implementation. A failing pre-feature run is not semantic verification.
