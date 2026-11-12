# F6: discharged obligation followed by computational cut

received0 creates deadline2 -> Worker.start0 -> Worker.finish1 discharges #1 ->
Slow.start1; next finish is6, beyond clock3. No terminal contributes metrics.
Even though the observed obligation was discharged, ADR 0010 addendum makes all
nonviolated clauses inconclusive if any path is cut. #1/safety/root inconclusive,
exit2, truncated true, terminals empty. Pair changes only target to Slow.finish.
