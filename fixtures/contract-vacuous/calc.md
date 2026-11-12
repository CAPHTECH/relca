# F5: unreachable antecedents, not lexical absence

Hand sequence: received0 -> Gate.start0 -> finish(pass)1 -> else -> end.done1.
Effect has a lexical call but no reachable start, finish or commit. Thus all four
antecedents (#1 commit, #2 first commit, #3 start, #4 start) are absent on all paths.
No cut -> each clause vacuous. Safety satisfied, root vacuous/exit5, elapsed/work1 minute,
result_at null. Add fail to the Gate result set for the single-fault pair.
