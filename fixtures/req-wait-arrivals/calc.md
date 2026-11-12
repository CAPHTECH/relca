# req-wait-arrivals

ADR 0026 / 計画の第3節の手計算。正例。単位は分。

F14: 共通時刻でc1到着0/起床2/開始2/完了5、c2到着1/起床3/開始5/完了8。elapsed5,7/work各3、first2,4、queued0,2、intentional各2/全体4。wait0対照は開始0,3/完了3,6、elapsed3,5/first0,2/intentional0。wait中でも到着1を処理する。

変更は `wait 2min` → `wait 0min` の一箇所だけ。正常 Task の宣言時間は全て正。
期待は仕様の時刻・区間和・値域から先に固定し、実装出力から採取していない。
未実装 RED は意味論の合格ではない。
