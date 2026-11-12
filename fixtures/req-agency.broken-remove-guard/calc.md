# req-agency.broken-remove-guard

ADR 0026 / 計画の第3節の手計算。単一変更版。単位は分。

F10: Producer1+Judge1、passならPost1でend3/work3、failならt2拒否でend2/work2。approvalは直列Judge結果writerへ置換。Judgeのpass→failは全てPost効果を止めるのでall_paths/0。guard除去では両値Post開始、J祖先は残りnone/1、W-AGY-0。

変更は `requires approval == pass else denied; ` → `` の一箇所だけ。正常 Task の宣言時間は全て正。
期待は仕様の時刻・区間和・値域から先に固定し、実装出力から採取していない。
未実装 RED は意味論の合格ではない。
