# P11: aset-static-after-result

Hand oracle fixed before implementation (control). Unit: min.

Positive control: Two one-minute tasks run serially (or serialize at capacity1): elapsed=1+1=2 and work=1+1=2. Single misuse after-result must be statically refused with U-SYN-0, exit4. No numeric exploration after static failure.

Single mechanism edit: 'result rejected_duplicate }' -> 'result rejected_duplicate;registered=false }'.

Static failures have no analyzed time or invented zero counts. Computational cuts are not business rejection. Atomic refusal is a normal finish, not a requires refusal. A RED checkpoint is not semantic verification.
