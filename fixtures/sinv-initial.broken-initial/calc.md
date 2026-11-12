# sinv-initial.broken-initial — independent hand oracle

ADR 0037; design v0.2 §11.1; design v0.2 §12.4; design v0.2 §22.3. All times/work are minutes.

Receipt0 sees true in control and false in fault. Fix start0 repairs immediately, finish/end1 gives elapsed/work1 in both. Fault shortest path is receipt only: transition_count0, step0, time0. The violation must not prune or be erased by repair.

Single edit: 'ok: bool = true' → 'ok: bool = false'
Fault/control oracle: {"verdict": "violated", "diagnostics": [], "bounds": {"truncated": false}, "properties": [{"id": "contract Controls #1 invariant", "verdict": "violated"}], "terminals": [{"end": "done", "elapsed": {"min": 1, "max": 1}, "work": {"min": 1, "max": 1}, "result_at": null}]}

Expected data was frozen before implementation. A failing pre-feature run is not semantic verification.
