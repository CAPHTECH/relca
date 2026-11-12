# P11: aset-static-binding-to-case.broken-binding-to-case

Hand oracle fixed before implementation (single edit). Unit: min.

Positive control: Two one-minute tasks run serially (or serialize at capacity1): elapsed=1+1=2 and work=1+1=2. Single misuse binding-to-case must be statically refused with E-TYPE-0, exit3. No numeric exploration after static failure.

Single mechanism edit: 'Ledger.take(case.id)' -> 'Ledger.take(a)'.

Static failures have no analyzed time or invented zero counts. Computational cuts are not business rejection. Atomic refusal is a normal finish, not a requires refusal. A RED checkpoint is not semantic verification.
