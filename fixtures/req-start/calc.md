# req-start

ADR 0026 / 計画の第3節の手計算。正例。単位は分。

F1: 真なら t0 Start で begun=true、t4 Finish で finished=true。占有 [0,4)、work4、費用4×2+7=15。偽なら t0 denied、Start/Finish/Commit/Ack0、書込みなし、work0、費用0。accepted は到達不能、refused は到達する。p は未割当。

変更は `flag: bool = true` → `flag: bool = false` の一箇所だけ。正常 Task の宣言時間は全て正。
期待は仕様の時刻・区間和・値域から先に固定し、実装出力から採取していない。
未実装 RED は意味論の合格ではない。

## 補助受入の手計算

- 拒否版の始終の観測は begun=false / finished=false。正常版は開始で begun=true、正常終了で finished=true。これを終端分岐へ入れても時間は増えない。
- case.received→A.run.finish(denied) のresponseを足すと、拒否はFinishでないのでt0の業務終端でterminal_unresolved / violated。Start invariant、commitのonceは発火0でvacuous。
- 拒否Taskのstarts/work/refused_countは0/0/1、cancelled_before_start_countは0。refusal_wait=0、task.waitはnull/refused。
- 反例のRefuseはt0に1段、result=denied、executor=null。Start/Finish/Commit/Ack段は0、観測値は両方false。公開explainのrootはcheckと同一で、独立replayがverifiedでなければ受けない。

## 異議経路の補助受入（追加裁定）

ctx-contestのRoute.respondへfalse requires / else fail / start begun=true / cancel immediateを足す。通常案件はJudge[0,1)でend.returned1/work1のまま。起票した経路だけRouteの呼出しをt1に拒否し、call=contest:1 / result=fail / at[1,1]のCapability拒否行を出す。Startなしでbegunはfalse、route_work0、Finishでないのでcontest_responseはviolated、violation_reason=refused、response_at/resultはnull。declined経路はnot_filedのまま。

flag=trueへの対照ではRoute[1,3)で正常応答3、route_work2、contest_response satisfied、拒否行なし。業務終端の時刻1は両版とも凍結する。refusedは既存のcancelledやdeadlockの意味を変えず、新しい閉じたreasonとして扱う。
