# 未発生の終了時刻を見ない

ADR 0014 本文・追記と ADR 0013 から独立計算。単位は分。
Guess は1でearly/lateを非決定に選ぶ。Markerは4でtrue、Randomは3/5各1/2で終了。
Random3ならmarker=false、Random5ならmarker=true。同時刻順序では変化しない。
Guess early: hit確率(1/2)×1+(1/2)×0=1/2。late: (1/2)×0+(1/2)×1=1/2。
時刻1ではRandomのage3 coinは未発生。どのschedulerも未来を知れず min=max1/2、<=1/2 satisfied。
開始時に未来の時間をstateへ持つ誤実装なら、Guessを合わせmax1、逆にしてmin0になる。
done 確率1。elapsed min max(1,4,3)=4/max max(1,4,5)=5、work min8/max10。
result_at null、安全性 satisfied、root satisfied/exit0、切断なし、W-CUT-0 のみ。
各状態のwriterは別のcapなので兄弟write競合はない。
