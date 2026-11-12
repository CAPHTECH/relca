# sinv-compare-exhaustive.broken-bad-plan — independent hand oracle

ADR 0037; design v0.2 §11.1; design v0.2 §12.4; design v0.2 §22.3. All times/work are minutes.

Both plans start ok=true. Good finish1 preserves; Bad finish1 clears. A/B control safe/safe; changed B safe/violated. Both end/work1. Simulation4 has zero draws and done count4 each; safe row inconclusive/sample, bad row witness replication1. AFlow and BFlow are distinct process identities, so sampled operation keys are not aliased. Exhaustive unused capability warning is one W-UNREACH-0 per plan; simulation emits none.

Single edit: 'process BFlow { r = Desk.good()' → 'process BFlow { r = Desk.bad()'
Fault/control oracle: {"verdict": "violated", "diagnostics": [], "plans": [{"configuration": "A", "report": {"diagnostics": [{"code": "W-UNREACH-0"}]}}, {"configuration": "B", "report": {"diagnostics": [{"code": "W-UNREACH-0"}]}}]}

Expected data was frozen before implementation. A failing pre-feature run is not semantic verification.
