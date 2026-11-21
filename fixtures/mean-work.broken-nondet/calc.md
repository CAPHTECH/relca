# 非決定の費用を一様にしない

結果だけを非決定に変える。常にpassで費用(21+41)/2=31円、常にfailで(83+103)/2=93円。
費用[31,93]、作業・経過[3,6]、per_case_relaxation。
匿名作業しか無いので人0はexact。Done条件付きは分母下界0で denominator_may_be_zero。
verify metric は未実装なので reachable done は satisfied/exit0のまま。
