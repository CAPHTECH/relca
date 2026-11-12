# sinv-static-integer.broken-bare — independent hand oracle

ADR 0037; design v0.2 §11.1; design v0.2 §12.4; design v0.2 §22.3. All times/work are minutes.

count0→1 at1, safe upper1 and work1. A bare integer is not bool even if its value is0.

Single edit: 'invariant count <= 1' → 'invariant count'
Fault/control oracle: {"verdict": "invalid", "diagnostics": [{"code": "E-TYPE-0"}], "bounds": {"truncated": null}, "properties": [], "terminals": []}

Expected data was frozen before implementation. A failing pre-feature run is not semantic verification.
