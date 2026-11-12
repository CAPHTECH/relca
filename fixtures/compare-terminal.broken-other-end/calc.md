# F5 single fault: Q declares other

Exactly Q end done→end other. The global IR end table is {done,other} in BOTH standalone reports;
it is not the comparison inventory. Selected AST inventories P={done}, Q={other} differ.
left_only=[done],right_only=[other], E-CMP-0 invalid/exit3, no tables. Cut mismatch null.
P reachable done satisfied, Q violated, but invalid dominates at root. Each reached terminal
has elapsed/work1/result_at null. diff is NOT compare: it must keep its done/other union,
with a null missing side, without adding E-CMP-0 for terminal differences.
