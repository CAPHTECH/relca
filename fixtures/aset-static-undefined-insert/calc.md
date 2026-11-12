# P11: aset-static-undefined-insert

Hand oracle fixed before implementation (control). Unit: min.

Positive control: Two one-minute tasks run serially (or serialize at capacity1): elapsed=1+1=2 and work=1+1=2. Single misuse undefined-insert must be statically refused with E-NAME-0, exit3. No numeric exploration after static failure.

Single mechanism edit: 'keys += key' -> 'keys += missing'.

Static failures have no analyzed time or invented zero counts. Computational cuts are not business rejection. Atomic refusal is a normal finish, not a requires refusal. A RED checkpoint is not semantic verification.
