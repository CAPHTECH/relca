# P09: rsrc-refusal.broken-enabled

Hand oracle fixed before implementation (single fault). Units: minutes and anonymous units.

False gate refuses x at0: starts0/work0/no occupancy; y[0,2) uses2 yields O4/P2/U1/5. True gate: x[0,3),y[3,5), O10/W5. No branch or unused end is added.

Single edit: 'flag: bool = false' → 'flag: bool = true'.

No resource price is configured; absence is not zero cost. A RED run is not semantic verification.
