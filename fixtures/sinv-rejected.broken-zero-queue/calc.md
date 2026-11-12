# sinv-rejected.broken-zero-queue — independent hand oracle

ADR 0037; design v0.2 §11.1; design v0.2 §12.4; design v0.2 §22.3. All times/work are minutes.

Admission batches before dispatch. Queue1 admits case1 and rejects case2; case1 satisfied and ends1/work1, case2 vacuous with null times. Root vacuous5. Queue0 rejects both, so no valuation, no activation, no task/end; both rows vacuous. Task and done warnings are W-UNREACH-0 and W-UNREACH-2, exactly.

Single edit: 'queue capacity 1' → 'queue capacity 0'
Fault/control oracle: {"verdict": "vacuous", "diagnostics": [{"code": "W-UNREACH-0"}, {"code": "W-UNREACH-2"}], "bounds": {"truncated": false}, "properties": [{"id": "contract Controls #1 invariant", "verdict": "vacuous", "cases": [{"case_id": 1, "arrival": 0, "verdict": "vacuous"}, {"case_id": 2, "arrival": 0, "verdict": "vacuous"}]}], "terminals": []}

Expected data was frozen before implementation. A failing pre-feature run is not semantic verification.
