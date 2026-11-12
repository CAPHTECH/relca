# H02: rsrc-release-fork.broken-parallel

Hand oracle fixed before implementation (single fault). Units: minutes and anonymous units.

Fork retains Done sibling while other task waits. Correct R2 serial E5/O10/P2; R4 overlaps E3/O10/P4. A Done-holder implementation deadlocks at2 or3, O4..6; both capabilities can start on different paths so only W-DEADLOCK-0/W-UNREACH-2, no never_started warning. Internal fault injection needs separate execution evidence.

Single edit: 'R capacity 2' → 'R capacity 4'.

No resource price is configured; absence is not zero cost. A RED run is not semantic verification.
