# H-REJECT: scn-rejected-not-cutoff.broken-admit

Hand oracle, fixed before implementation (ADR0042; plan section 3). Time unit: min.

Queue0 rejects every arrival[0,2,4]. All three rejected terminal masses remain1 and query probabilities0 at D12; actual all-terminal stop4, cutoff0. Queue3 admits all: End5,10 and cutoff work2, third query unknown.

FIFO: start_i=max(arrival_i, finish_(i-1)), finish_i=start_i+service. At a business horizon D, work_i=max(0,min(D,finish_i)-start_i). A cutoff is neither End nor rejected nor unfinished nor a computational truncation.
For four deterministic replications, three query rows and confidence 1/2, one-sided alpha=(1-1/2)/(2*3)=1/12. G(4;1/2)=F(0;1/2)=1/16<1/12; four hits satisfy >=1/2, four misses violate it, four unknowns give [0,1].
No result was copied from a new Relca implementation. Stage C RED is not semantic verification.
