# 終了と ack の同時刻非決定

ADR 0014 本文・追記と ADR 0013 の取消・kernel の意味から独立計算。単位は分。
Fast は1で pass を採用、result_at=1。その時 Slow に取消要求、ack_within1 より ack=2。
時刻2の Slow 完了coinは正常終了候補あり1/2、生存1/2。これを ack/normal の非決定より先に決める。
候補あり側で normal を選ぶと changed の kernel true が1/2、積1/2×1/2=1/4。
候補あり側で ack を選べば0、生存側も ack のみで0。従って min0/max1/4、<=1/4 satisfied。
どちらも Slow は2で解放、elapsed2/work Fast1+Slow2=3。done 確率1、rejected は到達せず行なし。
safety satisfied、root satisfied/exit0、切断なし、W-CUT-0 のみ。
ack は kernel を引かず finish/commit/write も起こさない。独立性の仮定は kernel 後・scheduler 前。
