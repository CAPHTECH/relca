# pol-tie-actor.broken-change

ADR 0036; design v0.2 §22.3. Hand oracle, before implementation.

P03: x wins at3; y is queued-cancelled (finish3,work3) or starts at3 before adoption and acknowledges at4 (finish4,work4). Reversed fork starts y, so result5 and completion/work5..6. Deadline4 holds only in positive. Tie list remains [a,b].

The pair changes only 'a = A.x(); b = A.y()' to 'b = A.y(); a = A.x()'.
This is the single edit side. Expected verdict: violated; exit 1.
Not an implementation observation. New-policy parsing is expected RED at Stage C.
