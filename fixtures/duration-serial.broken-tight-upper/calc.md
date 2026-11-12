# 上限だけを狭める単一故障

正解版から within6 の <=0.25 だけを <=0.2 に変更。ADR 0014 の意味は不変。
独立な (3,3),(3,5),(5,3),(5,5) は各 (1/2)(1/2)=1/4、時刻6,8,8,10。
within6 の min=max1/4 は新上限1/5を超え query1 violated。query2 は3/4>=3/4、query3 は1>=1で satisfied。
容量違反なし、done 確率1、elapsed/work min6/max10、result_at null。切断なし。
root violated/exit1。W-CUT-0 のみ。期待は手計算で、binary 出力の転記ではない。
