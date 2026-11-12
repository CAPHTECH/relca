# pol-tie-person.broken-change

ADR 0036; design v0.2 §22.3. Hand oracle, before implementation.

P04: same arithmetic as P03. Separate Actor slots do not create a second p. Occupancy is global across actors. x first gives3..4; y first gives5..6.

The pair changes only 'a = A.x(); b = B.y()' to 'b = B.y(); a = A.x()'.
This is the single edit side. Expected verdict: violated; exit 1.
Not an implementation observation. New-policy parsing is expected RED at Stage C.
