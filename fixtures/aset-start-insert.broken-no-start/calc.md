# P14: aset-start-insert.broken-no-start

Hand oracle fixed before implementation (single edit). Unit: min.

Start0 inserts1, Fast completes1, ack2 cancels post without undoing start. E2/W(2+1)=3/result1. Removing start leaves empty and absent. No at_most_once is declared, so vacuity cannot mask reachability.

Single mechanism edit: 'start{keys+=key}' -> ''.

Static failures have no analyzed time or invented zero counts. Computational cuts are not business rejection. Atomic refusal is a normal finish, not a requires refusal. A RED checkpoint is not semantic verification.
