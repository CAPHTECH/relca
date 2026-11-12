# P19: aset-replay-display.broken-shorter-wait

Hand oracle fixed before implementation (single edit). Unit: min.

Ledger finishes1/2 and shares keys[1,2]; waits3 then Probe1 finishes5/6. Earliest invariant counterexample case1/t5, one shortest trace, verified replay, value array[1,2]. Fault wait2 yields4/5. Replay tampering [2,1],[1,1],[0,2],[1,3] is separately API-negative, never normalized.

Single mechanism edit: 'wait 3min' -> 'wait 2min'.

Static failures have no analyzed time or invented zero counts. Computational cuts are not business rejection. Atomic refusal is a normal finish, not a requires refusal. A RED checkpoint is not semantic verification.
