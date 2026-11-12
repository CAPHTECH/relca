# ADR 0043 hand derivation

T13g. Sequential writer then reader: one edge, no commits, legal. Fork siblings have ambiguous start snapshot in either lexical order. Reader does not write x; this must fail read variable has a concurrent writer, not write/write conflict.
Single edit: 'w = W.write(); r = R.read();' -> 'fork f { r = R.read(); w = W.write() }; join all f;'.


The expected values above are algebraic/static hand oracles, not captured output. Stage C RED is not implemented semantics, calibrated mutation detection, real-world legitimacy, skill retention, or all-schedule safety.
