# P08: aset-cancel

Hand oracle fixed before implementation (control). Unit: min.

Fast [0,1); first post [0,2) cancelled at ack2 has no atomic effect. Next post [2,5) commits, E5/W(1+2+3)=6/result1. Forbidden first finishes3 and inserts, second [3,6) rejects: E6/W7/result1, commit1 still.

Single mechanism edit: 'cancel cooperative ack_within 1min' -> 'cancel forbidden_after_start'.

Static failures have no analyzed time or invented zero counts. Computational cuts are not business rejection. Atomic refusal is a normal finish, not a requires refusal. A RED checkpoint is not semantic verification.
