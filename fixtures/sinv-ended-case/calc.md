# sinv-ended-case — independent hand oracle

ADR 0037; design v0.2 §11.1; design v0.2 §12.4; design v0.2 §22.3. All times/work are minutes.

Mixed outcomes: fail case ends2 with work2; pass case writes shared posted at3 with work3. The ended fail case is violated at global3, but completed_at/elapsed/work remain2. Across histories both case IDs have a violation (not both in the same history). Both-pass control ends3/4 with work3 each. Fault upper1 stops attempted2 at4, so posted terminal only3. Both-fail remains safe. A clean upper2 companion isolates the ended-case violation.

Control: {"verdict": "satisfied", "diagnostics": [], "bounds": {"truncated": false}, "properties": [{"id": "contract Controls #1 invariant", "verdict": "satisfied"}, {"id": "state_in_range", "verdict": "satisfied"}]}

Expected data was frozen before implementation. A failing pre-feature run is not semantic verification.
