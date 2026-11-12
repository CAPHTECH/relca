# Independent hand calculation — ADR 0041

A19 public supplement. A short writer touches ONLY y at2, while long writer runs1..6. V[x,y] rejects long. Dropping y is legal because y has LWW, but then stale long overwrites y at6 and no_lost_update is violated. An x-only check cannot pass this control.

Stage C: authored oracle, not observed implementation behavior.
