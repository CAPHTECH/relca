# Independent hand calculation — ADR 0041

A20 trace oracle (not inferred from root). Prefix long1..6, short1..2 writes taken, release2..3 writes free. At6 value equals initial free yet long MUST conflict, work5. Control release=taken distinguishes restoration but never excuses stale history. Cases can independently pick either arm.

Stage C: authored oracle, not observed implementation behavior.
