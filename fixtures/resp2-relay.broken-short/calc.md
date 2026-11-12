# F11 故障 — 中間を落とすと見逃す境界

within9→8 の一箇所。経路の所要時間2+4+3=9>8なので violated。
Relay を落として5、最後の二ノードだけで7、最短経路だけで5とすると誤って合格する。
root safety satisfied、O/U は正例と同じ。実行は returned3/done10分のまま。
