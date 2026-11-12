# P05: rsrc-reuse.broken-excess-next

Hand oracle fixed before implementation (single fault). Units: minutes and anonymous units.

x[0,2) releases both units at2, y[2,5): O=2*2+2*3=10, P2, U1/2. Fault y3>capacity2 deadlocks after x; O4, U1/5, missing1.

Single edit: 'capability y() -> T { uses R: 2' → 'capability y() -> T { uses R: 3'.

No resource price is configured; absence is not zero cost. A RED run is not semantic verification.
