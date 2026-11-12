# sinv-bool-type — independent hand oracle

ADR 0037; design v0.2 §11.1; design v0.2 §12.4; design v0.2 §22.3. All times/work are minutes.

Nonoptional builtin bool lowers to ==true. Explicit true does not make optional<bool> nonoptional. Bare optional atom is E-TYPE-0 before execution; comparison remains legal.

Control: {"verdict": "satisfied", "diagnostics": [], "bounds": {"truncated": false}, "properties": [{"id": "contract Controls #1 invariant", "verdict": "satisfied"}], "terminals": [{"end": "done", "elapsed": {"min": 1, "max": 1}, "work": {"min": 1, "max": 1}, "result_at": null}]}

Expected data was frozen before implementation. A failing pre-feature run is not semantic verification.
