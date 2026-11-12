# F5: obligation survives configuration removal

Hand oracle from ADR 0011, design v0.2 §12.4; units min.
AsIs only assignment (c1,a1): receipt0 → Clerk.start(c1)0/finish2 →
Approver.start(a1)2/finish3 → done3. Left={c1}, right={a1}, clause #1 satisfied;
safety/root satisfied, elapsed/work=2+1=3, result_at null.
ToBeA only named assignment Approver=a1; Extractor is anonymous:
receipt0 → Extractor.start0/finish1 → Approver.start(a1)1/finish2 → done2.
Left=∅, right={a1}, clause #1 vacuous (not satisfied), safety satisfied,
root vacuous, elapsed/work=1+1=2, result_at null. Neither path truncates.
Anonymous Extractor is not a separation event target, hence legal.

Diff keeps both property IDs, #1 from satisfied/to vacuous, root vacuous/exit5.
Selected call inventories remove Clerk and Clerk.enter, add Extractor and
Extractor.extract: exactly four differences, all matched by intent reasons;
removed targets have loses. Cut C is common and complete: no W-CUT-0,
W-DIFF-1/2, E-DIFF-0 or E-CMP-0. Agency stays Actor-level, executors unchecked;
static resource/authority differences remain unchecked.
