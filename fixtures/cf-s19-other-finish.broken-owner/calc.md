# Independent hand calculation — ADR 0041

S19 finish. External Actor write unsupported at this independent grammar entrance.

Stage C: authored oracle, not observed implementation behavior.

Boundary review M1 (ADR 0041): version_check does not hide diagnostics of the
legacy path. finish writes Other.x instead of holder, so holder in writes stays a
result writer: R (committed/conflict) into H (free/taken) is E-TYPE-0
"result writer has different base type", next to U-SYN-0 for the other Actor's
write. E- outranks U-, so the verdict is invalid (exit 3). Without version_check
the pre-change binary (56d4d0b) gives exactly these two diagnostics, invalid,
exit 3; conflict_review_fixes.rs keeps that pair.
