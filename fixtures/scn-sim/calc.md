# P06: scn-sim

Hand oracle, fixed before implementation (ADR0042; plan section 3). Time unit: min.

Four identical paths: finish3,6,9; D8 cuts third at work2. hits [4,4,0], unknown [0,0,4]. cutoff reps4 versus clock truncated reps4. Totalwork8 each, observed total32. Without D the clock8 cut makes truncated_replications4, so metrics.status is truncated as for a model without the scenario shell; a sampled metric with no sample has reason no_samples (ADR 0019), with or without a scenario.

FIFO: start_i=max(arrival_i, finish_(i-1)), finish_i=start_i+service. At a business horizon D, work_i=max(0,min(D,finish_i)-start_i). A cutoff is neither End nor rejected nor unfinished nor a computational truncation.
For four deterministic replications, three query rows and confidence 1/2, one-sided alpha=(1-1/2)/(2*3)=1/12. G(4;1/2)=F(0;1/2)=1/16<1/12; four hits satisfy >=1/2, four misses violate it, four unknowns give [0,1].
No result was copied from a new Relca implementation. Stage C RED is not semantic verification.
