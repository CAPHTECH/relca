# Independent hand calculation — ADR 0041

S21. A5 preserves observed pre-C5 classification: qualified reads is E-SYN-0 expected closing bracket, exit3. C5 owns later U-SYN-0 repair; this is NOT a new reads feature.

Stage C: authored oracle, not observed implementation behavior.

ADR 0043 integration (the C5 repair named above): `reads [Actor.x]` moves from E-SYN-0 to U-SYN-0 `Actor state reads are unsupported` (ADR 0043 decision and U11), exit4. The input is unchanged; only the classification migrates. The pre-C5 bytes stay frozen in the older manifests and the C5 manifest records this job as an explicit migration.
