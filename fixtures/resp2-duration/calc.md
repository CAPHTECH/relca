# F7 分布は上限 — 手導出

Judge の時間は2分が3/4、7分が1/4。正の質量の最大は7分で、期待値13/4分ではない。
O 経路[j]=7、U 経路[j,out]=7+3=10、T=max(7,10)=10分。
within10 ちょうどで satisfied。required_duration=duration_upper=10、witness=[j,out]。
実行の returned=1+{2,7}=3..8、done=1+{2,7}+3=6..11分、作業も同じ。
故障対は within10→9 一箇所。補助試験は99分:0を足しても T=10 を要求する。
