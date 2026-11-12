# F2: Cartesian latent initialization

Oracle: ADR 0012. Before receipt the nondet declarations have four combinations:

| truth | Invoice.difficulty | C1 at finish1 |
|---|---|---|
| true | routine | (true or false) and (true or false) = true |
| true | difficult | (true or false) and (false or true) = true |
| false | routine | (false or true) and (true or false) = true |
| false | difficult | (false or true) and (false or true) = true |

Each tuple initializes before its single receipt0; Worker starts0, finishes(ok)1,
then done1. There are no state writes, so each tuple is constant for the whole path.
All four paths have elapsed1/work1/result_at=null. Safety and C1 are satisfied;
root satisfied/exit0. Latent reads are legal in an invariant, not a business guard.
Assumptions latent.truth and Invoice.difficulty each have value nondet(全値),
and affects must explain all-value nondeterminism without any uniform distribution.
Latent initialization cannot alter time in this fragment; no kernel exists.
The single-fault pair narrows only the second conjunct. Extra four exclusion
invariants and internal tuple observers are needed to prove the whole product.
