# P07: rsrc-immediate

Hand oracle fixed before implementation (control). Units: minutes and anonymous units.

Request2 retains b until distinct ack2. Then c[2,3). W2+2+1=5/O2+2+2=6/P2/U3/10. Cooperative ack3 gives E4/W6/O7.

Single edit: 'result nondet { fail }; cancel immediate' → 'result nondet { fail }; cancel cooperative ack_within 1min'.

No resource price is configured; absence is not zero cost. A RED run is not semantic verification.
