# req-after-queue.broken-close

ADR 0026 / 計画の第3節の手計算。単一変更版。単位は分。

F2: 容量1。close先行は [0,2) の後に run のガードを評価。正は真で run[2,5)。故障は偽、t2拒否し elapsed/work2。run先行は [0,3), close[3,5)、両版 accepted5/work5。queued の t0 の真値を保存してはならない。

変更は `finish { flag = true }` → `finish { flag = false }` の一箇所だけ。正常 Task の宣言時間は全て正。
期待は仕様の時刻・区間和・値域から先に固定し、実装出力から採取していない。
未実装 RED は意味論の合格ではない。
