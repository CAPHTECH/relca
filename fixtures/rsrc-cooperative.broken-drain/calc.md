# P06: rsrc-cooperative.broken-drain

Hand oracle fixed before implementation (single fault). Units: minutes and anonymous units.

a[0,2), b[0,3) ack3, c[3,4) uses2: W2+3+1=6, O2+3+2=7, U7/20. Forbidden b[0,6), c[6,7): W9/O10/U1/2. Result adopted at2 in both.

Single edit: 'result nondet { fail }; cancel cooperative ack_within 1min' → 'result nondet { fail }; cancel forbidden_after_start'.

No resource price is configured; absence is not zero cost. A RED run is not semantic verification.
