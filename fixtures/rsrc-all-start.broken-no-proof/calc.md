# P24: rsrc-all-start.broken-no-proof

Hand oracle fixed before implementation (single fault). Units: minutes and anonymous units.

R2 proves both start0; a finishes2, forbidden b finishes6, consume[6,7), E7/W9/O8/P2/U2/5. R1 cannot prove b started before adoption; b may be cancelled while queued, hence its later result read is unsupported, not a runtime value.

Single edit: 'R capacity 2' → 'R capacity 1'.

No resource price is configured; absence is not zero cost. A RED run is not semantic verification.
