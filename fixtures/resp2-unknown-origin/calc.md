# F12 O と U は独立 — 手導出

Bad の対象効果は Bad.set/out 一つ。引数祖先 Inspector.inspect が読む Record を必要とする。
p は Inspector の実行者で直接 reads Record、O present。inspect の両結果で Bad が必ず動くので U missing。
既知の停止なしを W-RESP-2 一つで残す。時間句なし unchecked。done=2+3=5分、safety satisfied。
故障対は inspect の reads 一句だけを削る。O が不明でも U の既知欠落を隠さない。
