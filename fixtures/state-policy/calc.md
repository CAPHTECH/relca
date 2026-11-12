# F1: observed policy

Oracle: ADR 0012 including its addendum; authored before implementation.
Units: min. One initial tuple: (difficult_seen=false, Invoice.difficulty=routine).
Receipt occurs at 0 after initialization. The observed guard is false, so only
AI.extract starts at 0 and finishes with ok at 1. Human.enter never starts.
Neither capability writes state; both initial values remain unchanged through done1.
Elapsed=1, work=1, result_at=null. Capacity1 is never exceeded; the sole safety
property and root are satisfied, exit0. Fixed latent has no nondet assumption.
State guard implies agency-p1-state-1 / state_guards; no effect is declared.
The pair changes only the guard atom to an Actor-local latent read.
No probabilities, costs, or empirical guarantees are inferred.
