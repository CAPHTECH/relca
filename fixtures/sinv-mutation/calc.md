# sinv-mutation — independent hand oracle

ADR 0037; design v0.2 §11.1; design v0.2 §12.4; design v0.2 §22.3. All times/work are minutes.

Approval finishes at minute 2 with pass or fail (not probabilities). The fail guard ends returned at elapsed/work 2/2. Pass alone posts during [2,3], so posted elapsed/work is 3/3. registered is false at receipt, hence no activation there. At post finish registered=true and approval=pass. No invariant diagnostic is emitted. Mutation kinds2 and8 each have the same one guard-bypass site, two generated mutants in total, six unsupported kinds. Kind8 is detected by Controls #1 invariant only with that clause present. Without it, once stays satisfied and state guard gives no legacy Agency refusal check: kind8 undetected. Root remains unsupported/exit4, not violated.

Control: {"verdict": "unsupported", "diagnostics": []}

Expected data was frozen before implementation. A failing pre-feature run is not semantic verification.
