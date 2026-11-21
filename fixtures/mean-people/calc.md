# 名前付きの人だけの占有（実装前）

Aの2分はp1かp2が占有し、人の選択は非決定だが量を動かさない。Bの3分は匿名。
named_people_work=2人分、work=2+3=5分。cost=2×10+1+3×20+2=83円。
どの選択でも同じなので全行exact。satisfied/exit0。
故障対はBのcapacity 1をexecutors [p3]に置換するだけ。
