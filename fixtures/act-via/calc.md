# F5: 出力を読む辺だけを追う

e,r,j,k は各 1 分で 4 分、j=pass のとき z の 1 分を足し done=5 分。
j=fail なら rejected=4 分。k の結果は止めない。非決定を確率にしない。
起点は via model を明示した Extract の e だけ。
e→j が U(z,all_paths)、e→j→z が E の 2 列。guard 自体は辺でない。
r は同じ Invoice を読むが、e から r への読取り辺は無い。k は U ではない。
W-CUT-0 のみ。Actor 名から AI を推測せず、変容の認定は未検査。
