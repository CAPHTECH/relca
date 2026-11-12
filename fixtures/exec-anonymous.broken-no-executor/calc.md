# F6 single fault: delete A.executors only

Hand oracle from ADR 0011. A.capacity1 remains, so this is NOT E-CAP-0.
A is now anonymous: #1 separation reads its finish, #2 permitted reads its
start, #3 authority_basis reads its normal finish; each independently requires
named executors and must fail statically with U-SEM-0 even if A is uncalled.
No runtime path, assignment, event time or terminal metric is reported.
Root unsupported/exit4, properties=[]/terminals=[], truncated=null (not false).
The grants declaration alone remains legal; it cannot supply a Task identity.
No E-CON-0, E-CAP-0, W-DEADLOCK-0 or W-BUDGET-0. Separate single-clause tests
are required so one detected anonymous target cannot hide two unchecked clauses.
