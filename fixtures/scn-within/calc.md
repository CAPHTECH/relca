# P17: scn-within

Hand oracle, fixed before implementation (ADR0042; plan section 3). Time unit: min.

W6 deadlines[6,8,10]<=D12: [P1,P0,P0], per-case [S,V,V]. W9 deadlines[9,11,13]: [P1,P1,P[0,1]], [S,S,I].

FIFO: start_i=max(arrival_i, finish_(i-1)), finish_i=start_i+service. At a business horizon D, work_i=max(0,min(D,finish_i)-start_i). A cutoff is neither End nor rejected nor unfinished nor a computational truncation.
For four deterministic replications, three query rows and confidence 1/2, one-sided alpha=(1-1/2)/(2*3)=1/12. G(4;1/2)=F(0;1/2)=1/16<1/12; four hits satisfy >=1/2, four misses violate it, four unknowns give [0,1].
No result was copied from a new Relca implementation. Stage C RED is not semantic verification.
