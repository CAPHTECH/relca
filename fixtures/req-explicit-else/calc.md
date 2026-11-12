# req-explicit-else

ADR 0026 / 計画の第3節の手計算。正例。単位は分。

F4: 明示 else は同型の拒否値。else を削ると U-SYN-0 / unsupported。E-SYN-0 や暗黙 denied を正解にしない。

変更は ` else denied` → `` の一箇所だけ。正常 Task の宣言時間は全て正。
期待は仕様の時刻・区間和・値域から先に固定し、実装出力から採取していない。
未実装 RED は意味論の合格ではない。
