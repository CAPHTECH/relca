# Independent hand calculation — ADR 0041

Comparison preserves each side property/assumption contract; no ranks, no uniform scheduler. Both same declared configuration choices.

Stage C: authored oracle, not observed implementation behavior.

The ranged claims state is omitted so that the sample compare tests only the global no_lost_update row (simulation compare rejects state_in_range with nonglobal property lacks cases under the existing rule). Expected verdict unchanged. With the old binary the legacy control without last_write_wins is satisfied for both plans; with last_write_wins the global no_lost_update row adds a sampled safety property without a violation, hence inconclusive.
