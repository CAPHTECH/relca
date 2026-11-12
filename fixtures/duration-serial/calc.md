# 直列の独立時間分布

ADR 0014 本文・追記と ADR 0013 による独立手計算。実装出力は用いない。単位は分。
first/second はともに 3 が 1/2、5 が 1/2。4 経路 (3,3),(3,5),(5,3),(5,5) は
各 (1/2)(1/2)=1/4、和 1。second は first の終了後に開始し、done は second 終了時だけ true。
終了時刻は 6,8,8,10。within6 は 1/4、within8 は 1/4+1/4+1/4=3/4、within10 は 1。
全開始順・終了順で同じなので各確率の min=max。それぞれ <=1/4、>=3/4、>=1 を満たす。
占有は直列の実作業のみで elapsed/work とも min6/max10、result_at は null。
done 終端は全経路、確率 1。容量 1 を超えず safety satisfied、root satisfied、exit0。
Cut 未宣言の W-CUT-0 以外の診断なし。切断なし。仮定は first,second の宣言順で各1行。
