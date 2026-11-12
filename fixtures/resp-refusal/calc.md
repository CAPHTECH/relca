# F4: refusal positive control

Same flow as resp-origin. R={Invoice} from Reader.enter and Approver reads;
H(a1)={Approver}, direct=indirect={Invoice}: O present. Ledger.post@posted is
the sole effect. Approver.approve@approval reads a real argument and guards
post: fail terminates, pass continues. Empty context, all_paths/0 -> U present.
No duty warning on Approver; anonymous Ledger has exactly one W-RESP-0.
No W-RESP-1/2 or W-AGY-0/1. All four actors included in Scope; no contest.
returned elapsed/work=2/2min, done=3/3min, result_at=null. Safety/root
satisfied, exit 0. The broken partner removes only the else end, keeping
approval as post's argument so a judgement still exists. ADR 0009/0018 U4.
