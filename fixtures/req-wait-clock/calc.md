# req-wait-clock

ADR 0026 / 計画の第3節の手計算。正例。単位は分。

F9: short[0,3)、wait[3,7)、long[7,12)。期限6にend無しでresponse violated、end12/work8。clock6版はwake7を完了させずcut、end無し、3completionはinconclusive。期限6はclock内に解決しないので違反を保持。未完了を成功や通常失敗へ変換しない。

変更は `clock 12min` → `clock 6min` の一箇所だけ。正常 Task の宣言時間は全て正。
期待は仕様の時刻・区間和・値域から先に固定し、実装出力から採取していない。
未実装 RED は意味論の合格ではない。

## 切断指標と反例

clock6版へmetricsを足すと、全体/案件のintentional_waitの極値はnull/truncated。観測したprefixを完全な4分待ちへ補完しない。waitだけなのでCapability拒否表は付けない。正常clock12版のresponse反例にはt3のwait段があり、期限6の反例を独立replayできる必要がある。起床7や終端12を期限前へ丸めない。

wait5→endだけをclock4/live_tasks0へ置く補助入力では、wake5が地平外なので終端なし、reachableと3completionは全てinconclusive、truncated=true。wakeをmin(wake,clock)へ丸める故障は偽のend4を作る。後続の長いTaskが無いこの入力で、丸めを別のclock切断に隠させない。
