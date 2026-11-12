# P11: aset-static-builtin-person

Hand oracle fixed before implementation (control). Unit: min.

Positive control: Two one-minute tasks run serially (or serialize at capacity1): elapsed=1+1=2 and work=1+1=2. Single misuse builtin-person must be statically refused with U-SEM-0, exit4. No numeric exploration after static failure.

Single mechanism edit: 'type T = enum' -> 'type PersonId=enum{c1};type T = enum'.

Static failures have no analyzed time or invented zero counts. Computational cuts are not business rejection. Atomic refusal is a normal finish, not a requires refusal. A RED checkpoint is not semantic verification.
