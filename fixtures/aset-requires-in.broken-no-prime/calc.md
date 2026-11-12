# P20: aset-requires-in.broken-no-prime

Hand oracle fixed before implementation (single edit). Unit: min.

Prime [0,1) inserts1; work starts1 and ends2. Removing prime insertion (and its writes contract) makes work refuse before start at1, refusal1/work0, total E=W1. Both reach done. Atomic refusal in P01 instead consumes a full task minute.

Single mechanism edit: 'writes keys;finish{keys+=key};' -> ''.

Static failures have no analyzed time or invented zero counts. Computational cuts are not business rejection. Atomic refusal is a normal finish, not a requires refusal. A RED checkpoint is not semantic verification.
