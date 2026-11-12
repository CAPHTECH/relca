# Independent hand calculation — ADR 0041

A01/A15. Minutes: both start0 and finish1. First writes holder=taken and claims1; all other running Tasks become stale. Second finishes conflict with work1 but NO write/Commit. Reverse order gives each case both ends. claims never attempts2. Without V the holder self-pair is unresolved G5.

Stage C: authored oracle, not observed implementation behavior.
