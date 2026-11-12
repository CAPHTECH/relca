# pol-no-person-reservation.broken-change

ADR 0036; design v0.2 §22.3. Hand oracle, before implementation.

P17: H[0,3) blocks A on S, but does not reserve p. B takes p[0,1), A[3,5): finish5, work3+2+1=6. Make B4: B[0,4),A[4,6): finish6,work9. Reserving p would incorrectly defer B until5.

The pair changes only 'duration fixed 1min' to 'duration fixed 4min'.
This is the single edit side. Expected verdict: satisfied; exit 0.
Not an implementation observation. New-policy parsing is expected RED at Stage C.
