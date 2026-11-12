# req-observed.broken-latent

ADR 0026 / 計画の第3節の手計算。単一変更版。単位は分。

F3: 観測 true を読むと開始し4分で終了。宣言種だけ latent に変えると静的 E-POL-LATENT / invalid、探索なし。定数に見えても真値読取りは禁止。

変更は `observed case` → `latent case` の一箇所だけ。正常 Task の宣言時間は全て正。
期待は仕様の時刻・区間和・値域から先に固定し、実装出力から採取していない。
未実装 RED は意味論の合格ではない。
