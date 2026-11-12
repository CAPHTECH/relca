# F4 single fault: B.executors q → p

Hand oracle, ADR 0011. Units min. Each Actor still has effective capacity1.
Assignment A=p/B=p; receipt0 and fork0. Exactly two work-conserving orders:
A.start0 → A.finish3/release p → B.start3 → B.finish8 → done8;
B.start0 → B.finish5/release p → A.start5 → A.finish8 → done8.
No concurrent p Tasks and no intentional idle. Elapsed=3+5=8 on both paths,
work8/result_at null. Safety/root satisfied, exit0, W-CUT-0, not truncated.
Detection is the elapsed change 5→8, not a nonzero exit or a fabricated violation.
An Actor-local person key would incorrectly retain elapsed5.
