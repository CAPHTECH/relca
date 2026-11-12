# req-start-kept.broken-forbidden

ADR 0026 / 計画の第3節の手計算。単一変更版。単位は分。

F5: 両開始t0。Fast[0,1)、Slow immediate は[0,1)で work2、adopt/request/ack全てt1、begun=true は残り finished=false。取消不可版はSlow[0,5)正常終了し work6/end5、finished=true。startだけの begun を finish の自動結果writerへ加えない。

変更は `cancel immediate` → `cancel forbidden_after_start` の一箇所だけ。正常 Task の宣言時間は全て正。
期待は仕様の時刻・区間和・値域から先に固定し、実装出力から採取していない。
未実装 RED は意味論の合格ではない。
