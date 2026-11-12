# rec-start-finish

T18: start atomically installs high at0, finish installs low at1; result is Ack but explicit constants have Entry type. Removing start assignment alone violates start invariant at0; finish still low, done1/work1.

Expected values are hand derivations, not captured implementation output. Units: minutes. New acceptance stops RED before implementation.
