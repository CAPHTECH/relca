# sinv-simulation — independent hand oracle

ADR 0037; design v0.2 §11.1; design v0.2 §12.4; design v0.2 §22.3. All times/work are minutes.

Four identical deterministic paths; draws0, done count4, end/work1. Control has no witness and is only inconclusive/sample, never safety proved. False initial fault violates every replication despite start repair; first witness replication1. Arrivals[0] variant also required.

Control: {"verdict": "inconclusive", "diagnostics": [], "properties": [{"id": "contract Controls #1 invariant", "verdict": "inconclusive"}], "simulation": {"completed_replications": 4, "truncated_replications": 0}}

Expected data was frozen before implementation. A failing pre-feature run is not semantic verification.
