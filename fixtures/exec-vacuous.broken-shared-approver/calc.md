# F5 single fault: add c1 to Approver.executors

Hand oracle from ADR 0011; units min. AsIs assignments (c1,a1)/(c1,c1):
receipt0 → Clerk.start(c1)0/finish2 → Approver.start(person)2/finish3 → done3.
Both events activate #1: a1 satisfies ({c1}∩{a1}=∅); c1 violates at3.
AsIs clause/root violated, safety satisfied; e3/w3/result_at null.
ToBeA assignments Approver=a1/c1, Extractor anonymous: receipt0 → extract0..1
→ approve(person)1..2 → done2. Left=∅ on both paths; #1/root remain vacuous,
safety satisfied, e2/w2/result_at null. No truncation or static error.
Diff #1 from violated/to vacuous, root violated/exit1. Same four differences,
reasons and loses as base (executor change is not a new supported diff kind).
Cut C remains complete; no W-CUT-0/W-DIFF-1/2/E-DIFF-0/E-CMP-0.
This detects treating an inactive constraint as satisfied and Actor-name separation.
