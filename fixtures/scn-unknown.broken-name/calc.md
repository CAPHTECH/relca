# P13: scn-unknown.broken-name

Hand oracle, fixed before implementation (ADR0042; plan section 3). Time unit: min.

Declared Light exists; selecting Nope is E-NAME-0, not declaration absence.
Selector-error span follows the archived PRE-B5 configuration-required check/simulate diagnostics for crates/relca-cli/tests/support/scenario_legacy.relca (whole model [0,425)); see tests/support/scenario-selector/calc.md. The original executable is recorded separately.


FIFO: start_i=max(arrival_i, finish_(i-1)), finish_i=start_i+service. At a business horizon D, work_i=max(0,min(D,finish_i)-start_i). A cutoff is neither End nor rejected nor unfinished nor a computational truncation.
For four deterministic replications, three query rows and confidence 1/2, one-sided alpha=(1-1/2)/(2*3)=1/12. G(4;1/2)=F(0;1/2)=1/16<1/12; four hits satisfy >=1/2, four misses violate it, four unknowns give [0,1].
No result was copied from a new Relca implementation. Stage C RED is not semantic verification.
