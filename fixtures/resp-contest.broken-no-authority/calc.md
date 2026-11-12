# F1 fault: wrong capability grant

## D2 migration

Returned at 2 still branches declined/closed2 or filed/recheck2..3 on c1,
normal pass/fail, deadline7, responded/closed3. Runtime satisfied despite
the static reversal-authority violation: Clerk's runtime response is not
the authority to reverse Approver's end decision. Root remains violated.
End snapshots 2/2; obligations=[]; no metrics declaration. The modeled
occurrence/response/closure assumptions replace structural-only scope;
real-world occurrence/time, acceptance/remedy and route transfer remain unchecked.

Relative to resp-contest, only ApprovalGrant's target changes from
Approver.approve to Clerk.recheck. a1 is still Approver's executor and has
Permission {a1}; the grant for a different cap does not connect Authority.
Route and deadline true, reversal false, missing=[reversal_authority],
reason=Permission あり、Authority 未接続. Clause/root violated, exit 1.
Reader.enter directly reads Invoice; Approver receives it indirectly via entry.
No commits or duties. Both terminals remain elapsed=work=2min, result_at=null.
Scope includes every actor. Contest exists, so no W-CONTEST-0.
Oracle: ADR 0018 addendum U3/U5; ADR 0011 separates Permission and Authority.
