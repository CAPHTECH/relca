# F11 中間ノードを含む最大経路 — 手導出

Judge の2分から Relay4分を経て Effect3分へ引数依存。j fail は out を停止する。
O=[j] は2分、U=[j,relay,out] は2+4+3=9分、T=max(2,9)=9。
within9 で satisfied。Reader1分はこの U 経路の始点より前なので加えない。
実行 returned=3、done=1+2+4+3=10分、作業も同じ。Relay は値コピー式でなく有限 nondet 結果を使う。
故障対は期限9→8だけ。短/長の排他二経路を足さず max9 にする補助試験も置く。
