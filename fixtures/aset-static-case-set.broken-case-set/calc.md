# P11: aset-static-case-set.broken-case-set

Hand oracle fixed before implementation (single edit). Unit: min.

Positive control: No task is invoked: end at0, elapsed0 and work0. Single misuse case-set must be statically refused with U-SYN-0, exit4. No numeric exploration after static failure.

Single mechanism edit: 'actor Ledger{state keys:set<CaseId>}' -> 'observed case{keys:set<CaseId>};actor Ledger{}'.

Static failures have no analyzed time or invented zero counts. Computational cuts are not business rejection. Atomic refusal is a normal finish, not a requires refusal. A RED checkpoint is not semantic verification.
