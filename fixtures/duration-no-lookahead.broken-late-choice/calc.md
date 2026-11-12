# 現時刻の偶然の後なら due を見られる

正解版のGuess fixed1だけをfixed3へ変更。ADR 0014・ADR 0013 の独立手計算。
age3のRandom完了/生存は各1/2、Guessの結果非決定より先に決まる。
完了側はGuess earlyを先に書いてhit、生存側はlateで時刻5にhit。最大=(1/2)×1+(1/2)×1=1。
逆の選択と順序で両側nonhit、最小=(1/2)×0+(1/2)×0=0。従って [0,1]。
<=1/2 violated、安全性 satisfied、root violated/exit1。done確率1。
elapsed min max(3,4,3)=4/max max(3,4,5)=5、work min3+4+3=10/max3+4+5=12。
result_at null、切断なし、W-CUT-0 のみ。これは未来の先読みではなく既に起きたcoinの観測。
