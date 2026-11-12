# P11: aset-static-duplicate-atomic.broken-duplicate-atomic

Hand oracle fixed before implementation (single edit). Unit: min.

Positive control: Two one-minute tasks run serially (or serialize at capacity1): elapsed=1+1=2 and work=1+1=2. Single misuse duplicate-atomic must be statically refused with U-SEM-0, exit4. No numeric exploration after static failure.

Single mechanism edit: 'atomic {\n        if key in keys { result rejected_duplicate }\n        else { keys += key; registered = true; result committed }\n      }' -> 'atomic {\n        if key in keys { result rejected_duplicate }\n        else { keys += key; registered = true; result committed }\n      };atomic {\n        if key in keys { result rejected_duplicate }\n        else { keys += key; registered = true; result committed }\n      }'.

Static failures have no analyzed time or invented zero counts. Computational cuts are not business rejection. Atomic refusal is a normal finish, not a requires refusal. A RED checkpoint is not semantic verification.
