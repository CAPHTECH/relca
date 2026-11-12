# 並列の上限を狭める

正解版の <=0.25 のみ <=0.2 へ変更。各時間3/5の4組は各1/4、和1。
両3の場合だけ within3、P=1/2×1/2=1/4。全順で min=max1/4>1/5、query violated。
安全性は satisfied、root violated/exit1。done 確率1、elapsed min3/max5、work min6/max10。
result_at null、切断なし、W-CUT-0 のみ。ADR 0014 と ADR 0013 による手計算。
