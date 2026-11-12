# P15: aset-person-start.broken-finish-only

Hand oracle fixed before implementation (single edit). Unit: min.

Slow starts with c1 at0 and records c1; cancellation ack2 does not roll back. Finish-only write never occurs and remains none. E2/W3/result1 in both. ID source and effect time are distinct obligations.

Single mechanism edit: 'start{input_by=executor.id}' -> 'writes input_by;finish{input_by=executor.id}'.

Static failures have no analyzed time or invented zero counts. Computational cuts are not business rejection. Atomic refusal is a normal finish, not a requires refusal. A RED checkpoint is not semantic verification.
