# F6 fault: missing state norm observation

Only Approver reads [Invoice] is removed. State writers/support as resp-state;
R={Invoice} still comes from Reader.enter -> entry -> ap -> Ledger.post.
H(a1)={Approver}; direct={}, indirect={Invoice}, missing={Invoice}: O missing,
W-RESP-1 once. Ledger registered write has ap all_paths/0; noncommits approve
can break P in an abstract registered=true state, so U not_derived with
noncommit_state_write, not present/missing. Keep responsibility_noncommit_writes
unchecked. Anonymous Ledger alone W-RESP-0, no W-RESP-2. Scope includes all
actors. done 3/3, returned 2/2min, result_at=null; safety/root satisfied, exit 0.
Actual flow preserves P, but does not cure missing original access.
Oracle: ADR 0018 U4, ADR 0009, ADR 0012, design v0.2 §12.6.
