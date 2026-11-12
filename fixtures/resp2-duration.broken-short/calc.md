# F7 故障 — 上限に1分不足

期限10分を9分へ変えるだけ。T=max(7,7+3)=10>9、時間欄 violated。
期待値13/4やIR固定時間placeholder0で計算するとこの故障を見逃す。
O/U/root safety は不変、returned3..8/done6..11分。時間分布の出力を oracle として写していない。
