# eval-hard.broken-quantile — ADR 0047 T01

Hand oracle, authored before implementation. Stage C RED is not verified semantics.

T13:quantile. Single local edit: the objective names quantile.Median, but no
quantile metric is defined. The fault in model.relca is unchanged.

ADR 0049, the syntax obligation 1 for intake after ADR 0047, replaces the old
U-SYN-0 expectation: quantile objectives are now parsed. An undefined quantile
name is an E-NAME-0 static error, not unsupported syntax. Both Check and Fast
must report invalid with unknown quantile metric: Median; the root is invalid
and the CLI exits 3. There is no evaluation or Agency partial result and no
runtime numeric oracle is changed. This expectation was written before running
the updated real CLI, not copied from its output.
