# P18: rsrc-collision.broken-same-name

Hand oracle fixed before implementation (single fault). Units: minutes and anonymous units.

Control E1/W1/O1/P1/U1/10. Resource A collides with Actor A, regardless declaration order. Fault is static: no runtime numbers.

Single edit: 'resource R capacity 1' → 'resource R capacity 1; resource A capacity 1'.

No resource price is configured; absence is not zero cost. A RED run is not semantic verification.
