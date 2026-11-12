# P15: scn-inclusive.broken-earlier

Hand oracle, fixed before implementation (ADR0042; plan section 3). Time unit: min.

At D10, finish2 and start3 happen before cutoff: completed2/cutoff1, starts3, work10. D9: completed1/cutoff2, starts2, work9.

FIFO: start_i=max(arrival_i, finish_(i-1)), finish_i=start_i+service. At a business horizon D, work_i=max(0,min(D,finish_i)-start_i). A cutoff is neither End nor rejected nor unfinished nor a computational truncation.
For four deterministic replications, three query rows and confidence 1/2, one-sided alpha=(1-1/2)/(2*3)=1/12. G(4;1/2)=F(0;1/2)=1/16<1/12; four hits satisfy >=1/2, four misses violate it, four unknowns give [0,1].
No result was copied from a new Relca implementation. Stage C RED is not semantic verification.
