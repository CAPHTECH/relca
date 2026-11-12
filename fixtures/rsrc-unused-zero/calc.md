# P15: rsrc-unused-zero

Hand oracle fixed before implementation (control). Units: minutes and anonymous units.

Unused zero capacity is legal, E1/W1/O0/P0; utilization null/zero_capacity. Adding uses1 makes deadlock0, O0/P0 and same null reason. Unused assumption suffix only on control.

Single edit: 'capability x() -> T {' → 'capability x() -> T { uses R: 1;'.

No resource price is configured; absence is not zero cost. A RED run is not semantic verification.
