# P14: rsrc-over-demand.broken-too-much

Hand oracle fixed before implementation (single fault). Units: minutes and anonymous units.

Control x[0,1): E1/W1/O1/P1/U1/10. Demand2>capacity1 is legal lower, but no start: deadlock0, safety satisfied, no terminal. O0/P0/U0, required2/available1/missing1.

Single edit: 'uses R: 1' → 'uses R: 2'.

No resource price is configured; absence is not zero cost. A RED run is not semantic verification.
