# F1: guarded commit (ADR 0010)

Independent hand oracle, fixed before implementation. Units: minutes; no probabilities.
Both paths begin received(0), Approver.start(0), Approver.finish(1).
pass: Ledger.start(1), Ledger.finish(ok)+commit(2), end.done(2).
Clause #1 reads approval=pass at commit, hence true; work=1+1=2.
fail: end.rejected(1), work=1; no commit, so no activation on this path.
Across all paths activation exists and there is no violation/cut: #1 satisfied.
Safety satisfied (one serial task), root satisfied, exit 0. No any adoption: result_at null.
Cut undeclared gives W-CUT-0, not a contract failure. Pair removes only the rejection guard.
