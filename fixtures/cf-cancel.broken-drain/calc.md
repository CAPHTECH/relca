# Independent hand calculation — ADR 0041

A21 trace oracle: long1..6; slow1..4 cancelled at2 by Fast1..2. Immediate ACK must NOT write/notify, hence long committed6. Drain writes at4, hence long conflicts6. Start effects, if separately added, are not rolled back.

Stage C: authored oracle, not observed implementation behavior.
