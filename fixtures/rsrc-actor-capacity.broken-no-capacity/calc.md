# P23: rsrc-actor-capacity.broken-no-capacity

Hand oracle fixed before implementation (single fault). Units: minutes and anonymous units.

Control E1/W1/O1/P1/U1/10. Positive duration needs Actor capacity even when R has capacity1. Fault is static: no runtime numbers.

Single edit: 'capacity 1;  capability' → ' capability'.

No resource price is configured; absence is not zero cost. A RED run is not semantic verification.
