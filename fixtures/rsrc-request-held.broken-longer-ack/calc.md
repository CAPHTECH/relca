# P08: rsrc-request-held.broken-longer-ack

Hand oracle fixed before implementation (single fault). Units: minutes and anonymous units.

At request2 b still holds1. ack3 or4; c starts only after ack, lasts1 using2. E4..5/W6..7/O7..8/U7/20..2/5. Request alone cannot free a unit.

Single edit: 'result nondet { fail }; cancel cooperative ack_within 1min' → 'result nondet { fail }; cancel cooperative ack_within 2min'.

No resource price is configured; absence is not zero cost. A RED run is not semantic verification.
