# P04: rsrc-atomic.broken-excess-demand

Hand oracle fixed before implementation (single fault). Units: minutes and anonymous units.

Atomic all-or-none acquisition, despite reversed order: E5/W5/O5/P1 for each. Fault: only y[0,2) can run. At2 x needs R2, available1, missing1. No terminal. O2 for both, safety satisfied; deadlock violated.

Single edit: 'uses R: 1, S: 1' → 'uses R: 2, S: 1'.

No resource price is configured; absence is not zero cost. A RED run is not semantic verification.
