# Independent hand calculation — ADR 0041

S19 writes. External Actor write unsupported at this independent grammar entrance.

Stage C: authored oracle, not observed implementation behavior.

Boundary review m2 (ADR 0041): the U-SYN-0 of the other Actor's write does not
hide the version_check checks. writes lists [Other.x, claims], so holder, the
version_check target, is not among the declared writes: U-SEM-0 "version_check
target is not written". Both are U-, so the verdict stays unsupported (exit 4).
