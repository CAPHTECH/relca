# sinv-no-prune.broken-second — independent hand oracle

ADR 0037; design v0.2 §11.1; design v0.2 §12.4; design v0.2 §22.3. All times/work are minutes.

Receipt violates at0 in both inputs. Fix repairs at start but sticky failure remains; one/two positive tasks still finish and end with elapsed/work1 or2. Violation never prunes.

Single edit: 'end done' → 's = Fix.fix(); end done'
Fault/control oracle: {"verdict": "violated", "diagnostics": [], "bounds": {"truncated": false}, "properties": [{"id": "contract Controls #1 invariant", "verdict": "violated"}], "terminals": [{"end": "done", "elapsed": {"min": 2, "max": 2}, "work": {"min": 2, "max": 2}, "result_at": null}]}

Expected data was frozen before implementation. A failing pre-feature run is not semantic verification.
