# P23: aset-atomic-sampling.broken-no-check

Hand oracle fixed before implementation (single edit). Unit: min.

Atomic makes deterministic dup, four replications, each E=W2, draws empty. Finish-only always twice; no dup witness means inconclusive sample reachability, not violated. Corresponding exhaustive probability1 vs0 is separately asserted.

Single mechanism edit: 'atomic {\n        if key in keys { result rejected_duplicate }\n        else { keys += key; registered = true; result committed }\n      }' -> 'finish { keys += key; registered = true }; result nondet { committed }'.

Static failures have no analyzed time or invented zero counts. Computational cuts are not business rejection. Atomic refusal is a normal finish, not a requires refusal. A RED checkpoint is not semantic verification.
