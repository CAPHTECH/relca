# F3: direct original and effective refusal

ADR 0018 U4, ADR 0009. P=Approver.approve at approval. Required sources R={Invoice}
from its reads and entry's Reader.enter ancestry. H(a1)={Approver}; origin
direct={Invoice}, indirect={Invoice}. R subset direct -> O present, no W-RESP-1.
E(P)={Ledger.post at posted}. Approval is an argument ancestor and guard:
pass continues, fail terminates. The sole empty context has no bypass;
Approver.approve@approval all_paths/0 -> U present, no W-RESP-2.
Ledger is an anonymous reachable E actor with no duty: exactly one W-RESP-0
for Ledger; Approver has a duty. No Authority/permit/contest needed for O/U.
Scope includes all four actors. Reader 1 + Approver 1 -> returned 2min;
add Ledger 1 -> done 3min. Work equals elapsed, result_at=null. Safety/root
satisfied, exit 0. Partner removes only Approver reads [Invoice].
