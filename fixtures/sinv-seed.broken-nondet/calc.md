# sinv-seed.broken-nondet — independent hand oracle

ADR 0037; design v0.2 §11.1; design v0.2 §12.4; design v0.2 §22.3. All times/work are minutes.

Fixed true has one receipt valuation. Removing default creates two valuations true/false; false receipt violates (not seed). End0/work0 in both; latent nondet is not probability1/2.

Single edit: 'flag: bool = true' → 'flag: bool'
Fault/control oracle: {"verdict": "violated", "diagnostics": [], "bounds": {"truncated": false}, "properties": [{"id": "contract Controls #1 invariant", "verdict": "violated"}], "terminals": [{"end": "done", "elapsed": {"min": 0, "max": 0}, "work": {"min": 0, "max": 0}, "result_at": null}]}

Expected data was frozen before implementation. A failing pre-feature run is not semantic verification.
