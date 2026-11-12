# P16: aset-atomic-rollback.broken-initial-one

Hand oracle fixed before implementation (single edit). Unit: min.

Control: first bundle inserts key, sets registered and n1; second rejects without evaluating n+1. Fault attempts n2 at t1: no insertion, no result/commit, registered false, n remains1. state_in_range V, reachable/contract I, not computational truncation.

Single mechanism edit: 'n:0..1=0' -> 'n:0..1=1'.

Static failures have no analyzed time or invented zero counts. Computational cuts are not business rejection. Atomic refusal is a normal finish, not a requires refusal. A RED checkpoint is not semantic verification.
