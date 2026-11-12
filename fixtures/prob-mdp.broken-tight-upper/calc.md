# F2 単一故障: 閾値だけ 3/4 → 1/2

経路・時刻・分布は base と同じ。safe 1/4、risky 3/4、従って hit/bad/good の min=1/4、max=3/4。
両終端 elapsed=work=2 分、result_at=null。safety satisfied。
最大下界3/4 > 1/2 より確率性質/root violated、exit1。切断なし。
非決定を一様にすると (1/2)(1/4)+(1/2)(3/4)=1/2 になり誤って満たすので、この故障対で検出する。
W-CUT-0 は中立。手計算のみで期待を固定。
