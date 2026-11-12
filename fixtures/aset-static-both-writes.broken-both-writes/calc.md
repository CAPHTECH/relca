# P11: aset-static-both-writes.broken-both-writes

Hand oracle fixed before implementation (single edit). Unit: min.

Positive control: Two one-minute tasks run serially (or serialize at capacity1): elapsed=1+1=2 and work=1+1=2. Single misuse both-writes must be statically refused with U-SEM-0, exit4. No numeric exploration after static failure.

Single mechanism edit: 'if key in keys { result rejected_duplicate }' -> 'if key in keys { registered=false;result rejected_duplicate }'.

Static failures have no analyzed time or invented zero counts. Computational cuts are not business rejection. Atomic refusal is a normal finish, not a requires refusal. A RED checkpoint is not semantic verification.
