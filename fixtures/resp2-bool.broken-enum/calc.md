# F3 故障 — 非 bool

変数の型/初期値だけ r:bool=false → r:Flag=off。対象は (r) のまま。
組込み bool ではないため E-TYPE-0、invalid、exit3。未使用変数でも型検査を省略しない。
探索・性質・終端・静的表は出さない。optional、latent、true/false 名の enum は追加行列で別に検査する。
