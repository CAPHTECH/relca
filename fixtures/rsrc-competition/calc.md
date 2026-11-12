# P11: rsrc-competition

Hand oracle fixed before implementation (control). Units: minutes and anonymous units.

FIFO is Actor-local. x1[0,2),y1[2,5),x2[5,7),y2[7,10) gives case1=5. x1[0,2),x2[2,4),y1[4,7),y2[7,10) gives7. case2=10. Same Actor y queues behind x2, leaving only7. Both O10/P1/U1. Inactive duplicate capability produces one existing never_started warning in each side, not a resource warning.

Single edit: 'y = B.y()' → 'y = A.y()'.

No resource price is configured; absence is not zero cost. A RED run is not semantic verification.
