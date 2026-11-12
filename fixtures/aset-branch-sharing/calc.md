# P21: aset-branch-sharing

Hand oracle fixed before implementation (control). Unit: min.

Each case begins false/empty own key and posts once; keys{1,2}, case completion1/2, W1 each. Replacing guard with per-case false registered control leaves runtime equal. Private transition seam, not this monotone fixture, must distinguish due shared finish vs Branch ordering.

Single mechanism edit: 'if case.id in Ledger.keys' -> 'if registered==true'.

Static failures have no analyzed time or invented zero counts. Computational cuts are not business rejection. Atomic refusal is a normal finish, not a requires refusal. A RED checkpoint is not semantic verification.

Both cases arrive at 0 and the guard reads the shared Ledger.keys, so each case's Branch is a nondeterministic candidate like start and finish. Case 2 may pass its Branch and enqueue before case 1; FIFO orders only the tasks already queued. Hence each case completes at 1 or 2: completed_at 1..2, elapsed 1..2 (arrival 0), work 1..1. The broken scalar guard reads a per-case variable, its Branch is not a shared candidate, FIFO by case_id fixes case 1 = 1 and case 2 = 2; this difference is what the pair discriminates.
