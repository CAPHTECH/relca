# P25: scn-mutate.broken-no-selector

Hand oracle, fixed before implementation (ADR0042; plan section 3). Time unit: min.

No reads, terminal guards, named authority_basis, shared producer, declared response fault sites or cohort denominators: candidates_total=0.
Round 2: unsupported retains precedence with zero executed mutants (exit4, reason null), original S. Removing the required scenario selector still gives original E-CONF-0/root invalid.

FIFO: start_i=max(arrival_i, finish_(i-1)), finish_i=start_i+service. At a business horizon D, work_i=max(0,min(D,finish_i)-start_i). A cutoff is neither End nor rejected nor unfinished nor a computational truncation.
For four deterministic replications, three query rows and confidence 1/2, one-sided alpha=(1-1/2)/(2*3)=1/12. G(4;1/2)=F(0;1/2)=1/16<1/12; four hits satisfy >=1/2, four misses violate it, four unknowns give [0,1].
No result was copied from a new Relca implementation. Stage C RED is not semantic verification.
