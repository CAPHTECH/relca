# sinv-static-unknown.broken-unknown — independent hand oracle

ADR 0037; design v0.2 §11.1; design v0.2 §12.4; design v0.2 §22.3. All times/work are minutes.

Control uses only declared state and has approval2/post1 (returned2, posted3). Single replacement is rejected statically with exactly E-NAME-0; no runtime terminal, no probability, bounds.truncated=null. Both expression sides and nested groups are tested separately.

Single edit: 'invariant registered' → 'invariant absent_name'
Fault/control oracle: {"verdict": "invalid", "diagnostics": [{"code": "E-NAME-0"}], "bounds": {"truncated": null}, "properties": [], "terminals": []}

Expected data was frozen before implementation. A failing pre-feature run is not semantic verification.
