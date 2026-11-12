# req-no-capacity.broken-zero

ADR 0026 / 計画の第3節の手計算。単一変更版。単位は分。

F16: capacity1なら開始候補の後にfalseを読みt0 denied/end refused/work0。capacity0なら候補無し、queuedから動かずdeadlock、truncated=false、安全性satisfied、reachable refused violated、W-DEADLOCK-0。ガードで資源不足を迂回しない。

変更は `capacity 1` → `capacity 0` の一箇所だけ。正常 Task の宣言時間は全て正。
期待は仕様の時刻・区間和・値域から先に固定し、実装出力から採取していない。
未実装 RED は意味論の合格ではない。
