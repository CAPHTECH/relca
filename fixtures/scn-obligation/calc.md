# P04: scn-obligation

Hand oracle, fixed before implementation (ADR0042; plan section 3). Time unit: min.

by9 deadlines [9,11,13]: cases1/2 resolved, case3 unknown. by7 deadlines [7,9,11]: cases2/3 violated, never excused by cutoff.
No obligation ledger: an anonymous response of a multi-case model has none under the old rule, and the scenario shell does not add one (boundary review). By9 case3 has deadline13>D12 and stays inconclusive; by7/by8 deadlines already due at D are expiry violations.


FIFO: start_i=max(arrival_i, finish_(i-1)), finish_i=start_i+service. At a business horizon D, work_i=max(0,min(D,finish_i)-start_i). A cutoff is neither End nor rejected nor unfinished nor a computational truncation.
For four deterministic replications, three query rows and confidence 1/2, one-sided alpha=(1-1/2)/(2*3)=1/12. G(4;1/2)=F(0;1/2)=1/16<1/12; four hits satisfy >=1/2, four misses violate it, four unknowns give [0,1].
No result was copied from a new Relca implementation. Stage C RED is not semantic verification.
