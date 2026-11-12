# F27 — different stopping reasons

Clock1 includes finish attempt at1: 1+1=2 > upper1, S violated,
truncated=false/range_violation_stops=true. Clock0 instead cuts before finish,
S inconclusive, truncated=true and range flag absent. Neither has a terminal.
The base intentionally violates: this pair compares reasons, not pass/fail labels.
