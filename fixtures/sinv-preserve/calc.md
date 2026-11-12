# sinv-preserve — independent hand oracle

ADR 0037; design v0.2 §11.1; design v0.2 §12.4; design v0.2 §22.3. All times/work are minutes.

Approval finishes at minute 2 with pass or fail (not probabilities). The fail guard ends returned at elapsed/work 2/2. Pass alone posts during [2,3], so posted elapsed/work is 3/3. registered is false at receipt, hence no activation there. At post finish registered=true and approval=pass. No invariant diagnostic is emitted. Preserve matches the invariant suffix and Controls ID. Identical plans summary satisfied1/vacuous0. No-post ToFlow remains registered=false: summary satisfied0/vacuous1, root vacuous5. To returned2/work2, From returned2/posted3. Removing the sole Ledger call removes four independently computed projections: actor Ledger, relation Ledger.post, meaning end.posted, resource Ledger. Preserve annotates but does not justify these changes: exactly four root W-DIFF-1. To has Ledger.post not_called W-UNREACH-0. Both process names are referenced by configurations; no W-CONF-0.

Control: {"verdict": "satisfied", "diagnostics": [], "from": {"report": {"diagnostics": []}}, "to": {"report": {"diagnostics": []}}}

Expected data was frozen before implementation. A failing pre-feature run is not semantic verification.
