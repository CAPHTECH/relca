# P06b: aset-cut-read

Hand oracle fixed before implementation (control). Unit: min.

No call or evidence read touches Ledger. At1 keys empty so false OR true is true. Exclusion must still reject the membership leaf; no short-circuit Cut exemption.

Single mechanism edit: 'include [Ledger,Probe]' -> 'include [Probe];exclude Ledger reason "outside" loses "keys"'.

Static failures have no analyzed time or invented zero counts. Computational cuts are not business rejection. Atomic refusal is a normal finish, not a requires refusal. A RED checkpoint is not semantic verification.
