# F6 single fault: late target behind clock

Only target Worker.finish -> Slow.finish. received0 creates deadline2;
Worker.start0 -> finish1 (not target) -> Slow.start1. No event between1 and clock3;
next event6 lies later than deadline2, and 2 <= clock3, so #1 violated in this prefix.
Do not look ahead to Slow.finish6 to fulfill the obligation; do not erase the proven
violation at cut. #1/root violated/exit1, safety inconclusive, truncated true,
terminals empty (no partial elapsed/work summaries). Independent ADR 0010 hand oracle.
