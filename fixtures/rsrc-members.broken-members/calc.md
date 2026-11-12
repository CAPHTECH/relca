# P16: rsrc-members.broken-members

Hand oracle fixed before implementation (single fault). Units: minutes and anonymous units.

Control E1/W1/O1/P1/U1/10. Reject members syntax before resolving undeclared p. Fault is static: no runtime numbers.

The static member/Actor-use side expects U-SYN-0 followed by W-CUT-0. This is existing report behavior: parse failure returns before a later explicit Cut is resolved (report.rs check_with). Both syntax positions were reproduced with the archived PRE-resource binary, exit4 and these two codes. No resource semantic warning is added.

Single edit: 'R capacity 1' → 'R capacity 1 members [p]'.

No resource price is configured; absence is not zero cost. A RED run is not semantic verification.
