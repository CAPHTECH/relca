# H06: rsrc-queued-cancel.broken-startable

Hand oracle fixed before implementation (single fault). Units: minutes and anonymous units.

R0: b never starts and cancels queued at2, starts0/cancelled_before_start1. a consumes no R, W2/O0/P0/null zero_capacity. R1: b[0,3) with ack3, E3/W5/O3/P1. Queue is not holding.

Single edit: 'R capacity 0' → 'R capacity 1'.

No resource price is configured; absence is not zero cost. A RED run is not semantic verification.
