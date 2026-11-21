# 作業と費用（実装前の手計算）

ADR 0078 の5.1。4経路の確率は3/8,3/8,1/8,1/8。
占有作業は2,4,5,7分、平均(6+12+5+7)/8=15/4分。
名前付きの実行者が無いので named_people_work は0人分（匿名を人へ推定しない）。
費用は21,41,83,103円、平均(63+123+83+103)/8=93/2円。
Done条件付きは(21+41)/2=31円。開始費Bの2円も、failの経路にだけ足す。
meanは検証要求ではなく reachable done が satisfied/exit0。
各故障対は結果の非決定化、開始費欠落、待ち、opaque defer の1変更。clock の対照は `crates/relca-core/tests/means.rs` の `mean_partial_work_and_cost_are_bounds_not_complete_or_clock_plus_one_work` で検査する。
