# Slow の取消不可だけを変更

正解版の Slow.cancel cooperative ack_within1 を forbidden_after_start へ置換。
ADR 0014・ADR 0013 の独立性から、Slow は2/4各1/2、それぞれ kernel true1/2。
P(changed within4)=1/2×1/2+1/2×1/2=1/2、全非決定で同じ min=max1/2。
query <=1/4 violated。Fast 採用1、Slow 正常完了2/4より elapsed min2/max4、work1+2/1+4からmin3/max5。
result_at1、done 確率1、rejected 行なし。safety satisfied、root violated/exit1。
切断なし、W-CUT-0 のみ。実装の出力ではなく手計算。
