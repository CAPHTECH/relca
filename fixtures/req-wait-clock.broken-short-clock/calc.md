# req-wait-clock.broken-short-clock

ADR 0026 / 計画の第3節の手計算。単一変更版。単位は分。

F9: short[0,3)、wait[3,7)、long[7,12)。期限6にend無しでresponse violated、end12/work8。clock6版はwake7を完了させずcut、end無し、3completionはinconclusive。期限6はclock内に解決しないので違反を保持。未完了を成功や通常失敗へ変換しない。

変更は `clock 12min` → `clock 6min` の一箇所だけ。正常 Task の宣言時間は全て正。
期待は仕様の時刻・区間和・値域から先に固定し、実装出力から採取していない。
未実装 RED は意味論の合格ではない。
