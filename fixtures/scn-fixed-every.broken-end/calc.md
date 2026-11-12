# P01: scn-fixed-every.broken-end

Hand oracle, fixed before implementation (ADR0042; plan section 3). Time unit: min.

n=1+(12-0-1)//4=3: [0,4,8], finishes [3,7,11]. Changing b to13 yields [0,4,8,12], four arrivals for cases3.

FIFO: start_i=max(arrival_i, finish_(i-1)), finish_i=start_i+service. At a business horizon D, work_i=max(0,min(D,finish_i)-start_i). A cutoff is neither End nor rejected nor unfinished nor a computational truncation.
For four deterministic replications, three query rows and confidence 1/2, one-sided alpha=(1-1/2)/(2*3)=1/12. G(4;1/2)=F(0;1/2)=1/16<1/12; four hits satisfy >=1/2, four misses violate it, four unknowns give [0,1].
No result was copied from a new Relca implementation. Stage C RED is not semantic verification.
