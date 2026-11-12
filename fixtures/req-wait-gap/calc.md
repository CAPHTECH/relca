# req-wait-gap

ADR 0026 / 計画の第3節の手計算。正例。単位は分。

F8: work3→wait2→work5 は t3待機/t5起床/t10終了。work3+5=8、意図待ち2、queued待ちは各0、first0。wait0ならt3起床/end8、work8。費用両方8×2+2×3=22。wait を架空Task/Actor/開始費へ数えない。

変更は `wait 2min` → `wait 0min` の一箇所だけ。正常 Task の宣言時間は全て正。
期待は仕様の時刻・区間和・値域から先に固定し、実装出力から採取していない。
未実装 RED は意味論の合格ではない。

## wait-onlyと期限の補助受入

- wait2→wait3→endはend5/work0/intentional_wait5、Task0・Actor0・Capability0・実行者0。live_tasks=0で足りる。費目の必要集合も空なので費用0。first_start_waitはnull/no_task_started。
- wait0→endはclock0でもend0/work0。起床管理でPCが進み、架空のループやTaskを作らない。
- wait2→short3→endはend5/work3。case.received起算のresponse期限5は期限ちょうどの履行でsatisfied、期限4ならviolated。
- contextual名のwait束縛、start Capability、immediate型は新構文を使っていない。1分のcallは従来どおりend1/work1、拒否表を追加しない。
