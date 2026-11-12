# req-wait-admission.broken-small-queue

ADR 0026 / 計画の第3節の手計算。単一変更版。単位は分。

F15: queue2は両受付でF14と同じ。queue1ではt1到着時c1はwait中で未Start、受付待ち1のまま。c2は受付拒否しcase.received/Task無し、四時間欄null。c1 end5/work3は保持。受付拒否はCapability拒否でない。

変更は `queue capacity 2` → `queue capacity 1` の一箇所だけ。正常 Task の宣言時間は全て正。
期待は仕様の時刻・区間和・値域から先に固定し、実装出力から採取していない。
未実装 RED は意味論の合格ではない。
