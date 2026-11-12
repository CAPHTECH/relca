# req-start.broken-false

ADR 0026 / 計画の第3節の手計算。単一変更版。単位は分。

F1: 真なら t0 Start で begun=true、t4 Finish で finished=true。占有 [0,4)、work4、費用4×2+7=15。偽なら t0 denied、Start/Finish/Commit/Ack0、書込みなし、work0、費用0。accepted は到達不能、refused は到達する。p は未割当。

変更は `flag: bool = true` → `flag: bool = false` の一箇所だけ。正常 Task の宣言時間は全て正。
期待は仕様の時刻・区間和・値域から先に固定し、実装出力から採取していない。
未実装 RED は意味論の合格ではない。
