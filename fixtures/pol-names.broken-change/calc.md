# pol-names.broken-change

ADR 0036; design v0.2 §22.3. Hand oracle, before implementation.

P12: bijection Clerk->Zebra,Approver->Minister,c1->z1,c2->y2,a1->x1 preserves order, occupancy and work3. First idle uses z1,y2 even though lexical order differs. Swapping only Zebra executors gives y2 twice and violates separation. grain is uninterpreted.

The pair changes only 'executors [z1,y2]' to 'executors [y2,z1]'.
This is the single edit side. Expected verdict: violated; exit 1.
Not an implementation observation. New-policy parsing is expected RED at Stage C.
