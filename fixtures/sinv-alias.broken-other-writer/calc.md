# sinv-alias.broken-other-writer — independent hand oracle

ADR 0037; design v0.2 §11.1; design v0.2 §12.4; design v0.2 §22.3. All times/work are minutes.

X calls Good and finishes1/work1; Other capability is not_called (W-UNREACH-0). Every approval binding in both processes writes the matching state. Removing just Other writer must reject even selection X, whose own writer is unchanged. StateInvariant name validation is model-wide, not selected-process-local. Both X and Y are tested.

Single edit: 'capability other() -> V { duration fixed 1min; result nondet { pass }; writes approval }' → 'capability other() -> V { duration fixed 1min; result nondet { pass } }'
Fault/control oracle: {"verdict": "unsupported", "diagnostics": [{"code": "U-SEM-0"}], "bounds": {"truncated": null}, "properties": [], "terminals": []}

Expected data was frozen before implementation. A failing pre-feature run is not semantic verification.
