# P11: aset-static-not-in.broken-not-in

Hand oracle fixed before implementation (single edit). Unit: min.

Positive control: Two one-minute tasks run serially (or serialize at capacity1): elapsed=1+1=2 and work=1+1=2. Single misuse not-in must be statically refused with E-SYN-0, exit3. No numeric exploration after static failure.

Single mechanism edit: 'if b == rejected_duplicate' -> 'if case.id not in Ledger.keys'.

Static failures have no analyzed time or invented zero counts. Computational cuts are not business rejection. Atomic refusal is a normal finish, not a requires refusal. A RED checkpoint is not semantic verification.
