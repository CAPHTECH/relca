# F22 aobs-integer-compare.broken-noninteger — hand oracle

ADR 0032; plan section 3. Values fixed before implementation, minutes.

1<2 true gives wait1/work0. Enum ordered comparison is unsupported even with numeric RHS. At n1 the six comparisons against1 yield true,false,false,true,false,true for ==,!=,<,<=,>,>=. Naked enum non== keeps legacy E-SYN-0.

Single input transformation: 'n: 0..2 = 1' → 'n: Mode = ok'.

Expected verdict: unsupported; command: check.
RED acceptance is not implementation verification.
