# req-same-time.broken-no-close

ADR 0026 / 計画の第3節の手計算。単一変更版。単位は分。

F13: Reader先なら[0,1)、Gate[1,3)、Writer[0,2)でaccepted3/work5。Gate先ならt2解放の後、Reader-start→Writer-finishはaccepted3/work5、Writer-finish→Refuseはrefused2/work4。このGate先の二順序を全終了一括更新で消してはならない。

変更は `finish { flag = false }` → `finish { flag = true }` の一箇所だけ。正常 Task の宣言時間は全て正。
期待は仕様の時刻・区間和・値域から先に固定し、実装出力から採取していない。
未実装 RED は意味論の合格ではない。
