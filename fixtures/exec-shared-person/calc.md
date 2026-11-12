# F4: independent people without contracts

Hand oracle, ADR 0011. Units min. Fixed A=p/B=q. Receipt0, fork creates both
queued Tasks; start orders A→B or B→A both at0. Work-conserving means no idle
alternative. A.finish(p,ok)3 → B.finish(q,ok)5 → join all → done5.
Elapsed=max(3,5)=5, work=3+5=8, result_at null. Distinct people and Actor capacities
are never overcommitted: safety/root satisfied, exit0, no truncation, W-CUT-0.
No clauses: resource effects must not be gated on the presence of executor contracts.
