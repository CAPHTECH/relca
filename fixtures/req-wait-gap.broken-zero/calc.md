# req-wait-gap.broken-zero

ADR 0026 / 計画の第3節の手計算。単一変更版。単位は分。

F8: work3→wait2→work5 は t3待機/t5起床/t10終了。work3+5=8、意図待ち2、queued待ちは各0、first0。wait0ならt3起床/end8、work8。費用両方8×2+2×3=22。wait を架空Task/Actor/開始費へ数えない。

変更は `wait 2min` → `wait 0min` の一箇所だけ。正常 Task の宣言時間は全て正。
期待は仕様の時刻・区間和・値域から先に固定し、実装出力から採取していない。
未実装 RED は意味論の合格ではない。
