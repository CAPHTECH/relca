# F5: legal result alias

Initial state approval=none; no binding result yet. Receipt0/start0 preserve none.
A.approve.finish(pass)1 publishes binding approval=pass and writes state
approval=pass before C1. C1 is true. The following if reads current observed
approval=pass, takes the empty true arm and reaches done1 (never rejected).
The only same-name call is a Result writer, so alias is statically legal.
Elapsed1/work1/result_at=null; safety/C1/root satisfied/exit0.
The pair removes writes approval only; it is rejected before any execution.
Extra tests read none before the call and fail after another writer overwrites
the state, so they do not confuse this coincident value with binding precedence.
