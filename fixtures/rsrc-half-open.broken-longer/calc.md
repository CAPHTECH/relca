# H16: rsrc-half-open.broken-longer

Hand oracle fixed before implementation (single fault). Units: minutes and anonymous units.

[0,2)*2+[2,3)*1 yields O5/P2/U1/6, not closed-interval P3. Longer x: [0,3)*2+[3,4)*1 gives O7/P2/U7/30. Safety remains satisfied in both.

Single edit: 'duration fixed 2min' → 'duration fixed 3min'.

No resource price is configured; absence is not zero cost. A RED run is not semantic verification.
