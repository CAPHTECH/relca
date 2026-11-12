# P02: rsrc-shared.broken-missing-use

Hand oracle fixed before implementation (single fault). Units: minutes and anonymous units.

Shared R serializes independent Actor slots: E=3+5=8, O8/P1. Removing B.y use permits overlap: E5, O3/P1, U3/10. Work8 on both.

Single edit: 'capability y() -> T { uses R: 1;' → 'capability y() -> T {'.

No resource price is configured; absence is not zero cost. A RED run is not semantic verification.
