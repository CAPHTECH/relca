# ADR 0030 F15 正例

分布も既定値もない2値潜在は非決定。2案件のtuple4組、各Dはmin0/max1であって1/2でない。simulateはU-SEM-0で統計なし。故障routine固定はD=0、初期draw0、simulateはこの初期値で拒否されない。

単一変更: latent difficulty: Difficulty → latent difficulty: Difficulty = routine。

期待値は仕様から先に固定した手計算で、実装出力の採取ではない。
時間の単位は分。確率は受付案件当たりであり、拒否を再正規化しない。
RED は未実装の観測であり、意味論の検証完了ではない。
