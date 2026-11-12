# F1 fault: wrong permitted person

## D2 migration

The wrong permit belongs to Approver.approve, not Clerk.recheck. Clerk
keeps default executor permission {c1}. Returned2 branches declined/closed2
and filed/recheck2..3 with pass/fail, deadline7. Runtime satisfied, static
and root violated as before. End snapshots2/2; obligations=[]; metrics absent.
Modeled occurrence/response/closure are assumptions; real-world occurrence,
response time, acceptance/remedy/route transfer remain unchecked, not verified.

Only permit a1 becomes permit c1. Permission on Approver.approve is now {c1},
not its default executors. a1 is still executor with ApprovalGrant, but fails
Permission. Route=true, reversal=false, deadline=true; missing reversal_authority,
reason permission_missing. Clause/root violated, exit 1. No dynamic permitted
clause was added: execution itself is not disabled by the permit.
Reader direct Invoice, Approver indirect Invoice; no commits/duties.
Both terminals elapsed=work=2min, result_at=null. Scope includes all actors;
the existing contest suppresses W-CONTEST-0. ADR 0018 U3/U5 and ADR 0011.
