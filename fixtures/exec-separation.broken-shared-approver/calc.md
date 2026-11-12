# F1 single fault: add c1 to Approver.executors

Hand oracle from ADR 0011. Units min. There is no explicit capacity to conflict.
Assignments: (Clerk,Approver)=(c1,a1) or (c1,c1), exhaustively.
Both have receipt0 → Clerk.start(c1)0 → finish(c1,ok)1 → Approver.start(person)1
→ finish(person,ok)2 → end.done2. Clerk releases c1 at1, so both starts are legal.
Controls #1: a1 path has {c1}∩{a1}=∅ and satisfies; c1 path has
{c1}∩{c1}={c1}, hence violates at2. Aggregate separation/root violated, exit1;
safety satisfied, elapsed2/work2/result_at null on both paths. No truncation.
W-CUT-0 only; runtime violation must not become E-CON-0. Different Actor names
do not imply different people. This targets lost history and first-executor-only mutations.
