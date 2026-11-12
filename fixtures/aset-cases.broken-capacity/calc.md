# P02: aset-cases.broken-capacity

Hand oracle fixed before implementation (single edit). Unit: min.

FIFO capacity1: case1 [0,1), case2 [1,2). Each W1 and one commit; final shared keys={1,2}. Capacity2 is a static shared-writer refusal; no exploration.

Single mechanism edit: 'capacity 1' -> 'capacity 2'.

Static failures have no analyzed time or invented zero counts. Computational cuts are not business rejection. Atomic refusal is a normal finish, not a requires refusal. A RED checkpoint is not semantic verification.
