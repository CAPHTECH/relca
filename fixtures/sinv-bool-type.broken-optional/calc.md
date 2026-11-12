# sinv-bool-type.broken-optional — independent hand oracle

ADR 0037; design v0.2 §11.1; design v0.2 §12.4; design v0.2 §22.3. All times/work are minutes.

Nonoptional builtin bool lowers to ==true. Explicit true does not make optional<bool> nonoptional. Bare optional atom is E-TYPE-0 before execution; comparison remains legal.

Single edit: 'ok: bool = true' → 'ok: optional<bool> = true'
Fault/control oracle: {"verdict": "invalid", "diagnostics": [{"code": "E-TYPE-0"}], "bounds": {"truncated": null}, "properties": [], "terminals": []}

Expected data was frozen before implementation. A failing pre-feature run is not semantic verification.
