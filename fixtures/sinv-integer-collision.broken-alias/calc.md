# sinv-integer-collision.broken-alias — independent hand oracle

ADR 0037; design v0.2 §11.1; design v0.2 §12.4; design v0.2 §22.3. All times/work are minutes.

Approval finishes at minute 2 with pass or fail (not probabilities). The fail guard ends returned at elapsed/work 2/2. Pass alone posts during [2,3], so posted elapsed/work is 3/3. registered is false at receipt, hence no activation there. At post finish registered=true and approval=pass. No invariant diagnostic is emitted. Latent bounded j stays0 and <=1 is true. Renaming only the approve binding to j creates a latent/binding collision, U-SEM-0 even on the integer comparison arm. It cannot silently select one namespace.

Single edit: 'approvalV = Approver' → 'j = Approver'
Fault/control oracle: {"verdict": "unsupported", "diagnostics": [{"code": "U-SEM-0"}], "bounds": {"truncated": null}, "properties": [], "terminals": []}

Expected data was frozen before implementation. A failing pre-feature run is not semantic verification.
