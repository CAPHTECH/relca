# P06: scn-sim-clock30.broken-earlier

Hand oracle, fixed before implementation (ADR0042; plan section 3). Time unit: min.

Retain the original ADR positive: D8/clock30, finishes3/6/9, third cutoff work2, totalwork8 and unknown4. The equal-clock pair is separate. Change only D8 to7: third work1, totalwork7, still unknown4. Old sampled clock cuts also observe partial occupancy through clock, so the no-horizon clock8 control has metric work8, not6.

FIFO: start_i=max(arrival_i, finish_(i-1)), finish_i=start_i+service. At a business horizon D, work_i=max(0,min(D,finish_i)-start_i). A cutoff is neither End nor rejected nor unfinished nor a computational truncation.
For four deterministic replications, three query rows and confidence 1/2, one-sided alpha=(1-1/2)/(2*3)=1/12. G(4;1/2)=F(0;1/2)=1/16<1/12; four hits satisfy >=1/2, four misses violate it, four unknowns give [0,1].
No result was copied from a new Relca implementation. Stage C RED is not semantic verification.
