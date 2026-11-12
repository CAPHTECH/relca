# P23P: aset-atomic-probability

Hand oracle fixed before implementation (control). Unit: min.

No random source: deterministic dup probability[1,1] vs[0,0] when check removed. E=W2 on both. Atomic results must not become nondeterministic outcomes.

Single mechanism edit: 'atomic {\n        if key in keys { result rejected_duplicate }\n        else { keys += key; registered = true; result committed }\n      }' -> 'finish { keys += key; registered = true }; result nondet { committed }'.

Static failures have no analyzed time or invented zero counts. Computational cuts are not business rejection. Atomic refusal is a normal finish, not a requires refusal. A RED checkpoint is not semantic verification.
