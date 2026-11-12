# X-I: removed rejection

Only change: remove end returned from the else block. ADR 0010 invariant is
checked after finish/commit together; no event is split into two transitions.
Hand path: initial0(t0,received), queue1 approval, start2 approval,
advance3(t1), finish4 approval=fail, join5, branch6 else, queue7 posted,
start8 posted, advance9(t2), finish10 posted=ok plus commit. Count10.
At this last bundle approval=fail, so the invariant first fails. Earlier
states cannot violate a commit-triggered clause; pass is not a counterexample.
There is only one failing result at each relevant choice, so no tie remains.
Actors/capabilities: approval Approver.check; posted Ledger.post. Anonymous
executors are null. All observed/latent arrays are empty; result only appears
on finish. Management resource is null. Stage call locations are approval
line7 col5 byte350 and posted line9 col5 byte419. Clause line15 starts after
contract Controls opening, not at verify safety. Tests assert complete
stage order, call/capability/clause source fragments and independent replay.
Expected replay verified, root violated/1, complete/model, diagnostics [].
