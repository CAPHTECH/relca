# F3 素の bool — 手導出

観測 r:bool=false に対する (r) は (r == true) と同義。型 Flag は未使用だが両側に予め宣言する。
writer がなく原本を識別できないので O not_derived/no_source_identified、U not_derived/no_reachable_effect。
担い手 p の Actor はゼロ、direct/indirect/required/missing は空。時間句なしは unchecked の一鍵のみ。
process は end done だけ。経過・作業とも0分、safety satisfied。未使用でも素atomの型は検査する。
故障対は変数宣言を r:Flag=off へ変える一箇所だけ。
