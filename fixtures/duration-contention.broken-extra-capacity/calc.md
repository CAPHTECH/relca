# 容量だけを2に変更

正解版の capacity1 を2へ単一置換。ADR 0014・ADR 0013 から計算。
両者時刻0開始、4時間組は各1/4、総和1。all は max(3,3)=3、他3組は5。
within6=1、within5=1、両方 min=max。query1 >=1/4 は satisfied のまま、query2 <=0 は violated。
安全性は satisfied、root violated/exit1。done 確率1、elapsed min3/max5、work min6/max10。
queued 待ちを占有に足さない。result_at null、切断なし、W-CUT-0 のみ。
