# F3 fault: indirect is not direct

Only Approver.approve's reads [Invoice] is removed from resp-origin.
R={Invoice} still follows entry <- Reader.enter reads Invoice. H(a1)={Approver},
direct={}, indirect={Invoice}, missing={Invoice}; O missing and W-RESP-1 once.
Reader's direct access does not transfer to a1. Ledger.post@posted still has
Approver.approve@approval all_paths/0: fail ends returned, pass posts. U present.
Anonymous Ledger adds W-RESP-0 as in the positive partner; no W-RESP-2.
Scope includes all four actors, no contest. returned 2/2, done 3/3min,
result_at=null, safety/root satisfied and exit 0. Duty is normative, not a
runtime invariant. Oracle: ADR 0018 U4, design v0.2 §12.6, ADR 0009.
