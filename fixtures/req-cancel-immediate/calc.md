# req-cancel-immediate

ADR 0026 / 計画の第3節の手計算。正例。単位は分。

F7: Fast占有[0,3)、Slow[0,3)で総work6。t3要求がack3を予約し独立の同時刻遷移で解放、end3。cooperative1はack4、Slow work4、総7/end4。要求時までの作業を消さず、宣言10分も丸ごと加えない。

変更は `cancel immediate` → `cancel cooperative ack_within 1min` の一箇所だけ。正常 Task の宣言時間は全て正。
期待は仕様の時刻・区間和・値域から先に固定し、実装出力から採取していない。
未実装 RED は意味論の合格ではない。

## 名前付き実行者と反例の補助受入

Fastをq、Slowをpへ割り当て、join後のNext.run1分もpへ割り当てる。即時版はSlowをt3に解放しNext[3,4)、end4/work7。協調1版はt4解放、Next[4,5)、end5/work8。採用時刻はどちらも3。

Slow.start→Slow.finish期限10分を足すと、取消は履行でなく終端未解決。反例はt3の要求(ack_at=3)とその後の独立のCancelAck段を別indexで含む。間で時計やworkを増やさない。
