# 時間分布の正しい和

ADR 0014 と ADR 0013 により手計算。3分1/2、5分1/2の和は1、値は一意。
hit は正常終了時true。within3 の正経路は3分だけで1/2、全非決定で min=max1/2。
query <=1/2 satisfied、safety satisfied、root satisfied/exit0。
done 確率1/2+1/2=1、elapsed/work min3/max5、result_at null。
切断なし、W-CUT-0 のみ。capacity1 を明示。
