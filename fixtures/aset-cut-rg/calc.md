# P03: aset-cut-rg

Hand oracle fixed before implementation (control). Unit: min.

Both one-call cases commit once, E1/E2, total W2. Guarding the wrong empty set leaves seen={} and keys={1,2}, but cannot certify a mechanism: notes required only on faulty side.

Single mechanism edit: 'if key in keys' -> 'if key in seen'.

Static failures have no analyzed time or invented zero counts. Computational cuts are not business rejection. Atomic refusal is a normal finish, not a requires refusal. A RED checkpoint is not semantic verification.
