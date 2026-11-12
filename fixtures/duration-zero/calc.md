# 確率0の時間候補を展開しない

ADR 0014・ADR 0013 による手計算。時間1,3,100の重みは0,1,0、和1。
唯一の正経路は3分で hit。within3=1、min=max1、>=1 satisfied。
done 確率1、elapsed/work min=max3、result_at null。1/100を時間極値へ入れない。
clock120で切断なし。safety satisfied、root satisfied/exit0、W-CUT-0 のみ。
仮定の行にはゼロ重み候補も時間昇順で残す。正確率0分とは別の試験。
