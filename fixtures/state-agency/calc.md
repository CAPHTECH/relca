# F7: pure binding Agency remains unchanged

Initial decision=none. Receipt0; Source.read starts0/finishes(entry)1, state none.
Judge starts1, finishes2 with pass or fail and writes decision to the same value
as binding j. pass: guard j==pass true, post starts2/finishes(committed)+commit3,
done3/e3/w3. fail: guard false, rejected2/e2/w2, no post. result_at=null in both.
State never changes after Judge.finish2. Safety/root satisfied/exit0, no contracts.

Hand Agency sets: Invoice/PurchaseOrder have no calls or ancestor sources.
Source direct={Invoice}, indirect={}; Judge direct={PurchaseOrder},
indirect={Invoice} via argument e. Ledger receives no argument: both sets empty.
Judgement j consumes subject {Invoice}, adds independent {PurchaseOrder}.
One lineage group: 1 judgement, 1 Actor, 1 independent source. Effect Ledger.post
at p is reachable on j=pass; the single context has a stopping j=fail value:
all_paths, bypasses0, no W-AGY. The guard is binding-only, so agency-p0-1 and
the seven old unchecked values persist despite state being declared and written.
The pair changes only j==pass to decision==pass, not any dynamic path.
