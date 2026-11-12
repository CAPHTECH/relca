# P12: rsrc-global.broken-two-units

Hand oracle fixed before implementation (single fault). Units: minutes and anonymous units.

FIFO x1[0,2),x2[2,4): cases2/4, O4/P1/U2/5. R2 permits both[0,2): cases2/2,O4/P2/U1/5. No resource copying per case.

Single edit: 'R capacity 1' → 'R capacity 2'.

No resource price is configured; absence is not zero cost. A RED run is not semantic verification.
