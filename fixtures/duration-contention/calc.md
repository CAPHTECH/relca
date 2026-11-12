# 容量1での競合

ADR 0014 と ADR 0013 による手計算。時間・作業量は分。
left先/right先は非決定。どちらでも後者は queued の間 age と work を増やさない。
実行時間の組 (3,3),(3,5),(5,3),(5,5) は各1/4=(1/2)(1/2)、総和1。
all は和6,8,8,10。両write within6 は1/4、within5 は0。各開始順の min=max。
query1 >=1/4 satisfied、query2 <=0 satisfied。容量超過なし、root satisfied/exit0。
done 確率1、elapsed/work min6/max10、result_at null。切断なし、W-CUT-0 のみ。
