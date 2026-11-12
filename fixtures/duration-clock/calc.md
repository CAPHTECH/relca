# 時計切断を問いごとに区別する

ADR 0014 本文・追記と ADR 0013 による独立手計算。単位は分。
時間1/3各1/2。1の枝で hit=true、done1、work1。生存1/2の次候補3はclock2を超える。
query1 D2=clock: clock以下の全bundleを処理済み。生存枝の期限内hitは0確定。
四端点は全て1/2、<=1/2 satisfied。query2 D3>clock: 生存質量は未知。
下界1/2、上界1/2+1/2=1、>=3/4 は inconclusive。非決定による幅ではなく切断の幅。
root inconclusive/exit2、truncated=true。done は完了経路だけから elapsed/work min=max1、result_at null。
終端確率は期限なしなので [1/2,1] のまま。切断を成功/失敗の終端へ変換しない。
時計切断は W-BUDGET-0 を出さず、W-CUT-0 のみ。safetyは併記しない。
