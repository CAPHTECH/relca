# X-I: guarded commit

Oracle: ADR 0021 and ADR 0010; design v0.2 §19. No probability is assumed.
Initial time 0 includes received. The only committing branch has approval=pass.
queue1/start2/advance3(t1)/finish4(pass)/join5/branch6/queue7/start8/
advance9(t2)/finish10(ok,commit)/join11/end12. The fail branch ends returned
after branch6/end7 at t1. Safety and invariant are satisfied; no counterexample.
All diagnostics are empty (explicit Scope includes Approver and Ledger).

Calls: approval line7 column5 byte350, posted line9 column5 byte432.
Each call spans binding through closing parenthesis, excluding whitespace.
Property is the invariant clause on line15, not the verify on line14.
Public tests compute half-open endpoints from these literal source fragments
and independently check Unicode scalar line/column conversion.
Replay must accept every published candidate; this model publishes none.
