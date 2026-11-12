# sinv-static-binding-compare.broken-binding-compare — independent hand oracle

ADR 0037; design v0.2 §11.1; design v0.2 §12.4; design v0.2 §22.3. All times/work are minutes.

Control uses only declared state and has approval2/post1 (returned2, posted3). Single replacement is rejected statically with exactly U-SEM-0; no runtime terminal, no probability, bounds.truncated=null. Both expression sides and nested groups are tested separately.

Single edit: 'approval == pass }' → 'j == pass }'
Fault/control oracle: {"verdict": "unsupported", "diagnostics": [{"code": "U-SEM-0"}], "bounds": {"truncated": null}, "properties": [], "terminals": []}

Expected data was frozen before implementation. A failing pre-feature run is not semantic verification.
