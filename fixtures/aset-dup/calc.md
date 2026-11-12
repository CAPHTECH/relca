# P01: aset-dup

Hand oracle fixed before implementation (control). Unit: min.

First finish t1 inserts {1}, registered=true, commit1. Second finish t2 sees1 and rejects: finish2/commit1, keys={1}, E=W=2, dup only. Without check both finish committed: commit2, twice only, contract and reachable dup violated. Notes absent/present respectively. Both sides have W-UNREACH-1 and W-UNREACH-2.

Single mechanism edit: 'atomic {\n        if key in keys { result rejected_duplicate }\n        else { keys += key; registered = true; result committed }\n      }' -> 'finish { keys += key; registered = true }; result nondet { committed }'.

Static failures have no analyzed time or invented zero counts. Computational cuts are not business rejection. Atomic refusal is a normal finish, not a requires refusal. A RED checkpoint is not semantic verification.
