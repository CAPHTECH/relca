# P09: aset-agency.broken-scalar-requires

Hand oracle fixed before implementation (single edit). Unit: min.

Seed1 + Writer1 + prime1 + post1 =4, always membership true at runtime. Contains is Unknown statically, so no refusal proof. Control decision==ok is rewritten to unique Writer result, producing pass4/fail3 and a Writer all_paths judgement; do not claim equal timing.

Single mechanism edit: 'requires key in Ledger.keys' -> 'requires decision==ok'.

Static failures have no analyzed time or invented zero counts. Computational cuts are not business rejection. Atomic refusal is a normal finish, not a requires refusal. A RED checkpoint is not semantic verification.
