# F3: 容量の二重計上

Team は contains Member だが Capability が無い。uses_capacity(Team)=false。
capacity=1 の宣言だけでは二重計上でない。Member の作業だけで終了/作業量 1 分。
W-CUT-0 のみ。contains は記録/静的検査であり容量を合算しない。
