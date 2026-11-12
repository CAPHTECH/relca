# Independent hand calculation — ADR 0041

A03. FIFO: case1 runs0..1, case2 starts AFTER first write and runs1..2. Both commit, claims2. retry_needed unreachable for BOTH, root violated1. Arrival-time stale incorrectly rejects case2; root alone cannot detect it.

Stage C: authored oracle, not observed implementation behavior.
