# F2: 非決定は平均しない

ADR 0013 本文・追記から、選択 safe の後の hit/bad は 1/4、risky は 3/4。
各分布は 1/4+3/4=1。外側の非決定で min=1/4、max=3/4。
good は各々 3/4 と 1/4 なので同じ幅。min の和 1/2、max の和 3/2 は別々の scheduler の極値であり正しい。
choose は 0→1、sample は 1→2 分。全経路 elapsed=work=2、result_at=null。
容量1を直列に使うので safety satisfied。最大3/4 <= 閾値3/4、確率性質/root satisfied、exit0。
probability.scheduler=full-information を表示する。これは実現可能な方策の値でなく完全情報の緩和境界。
Cut 未宣言の W-CUT-0 だけを要求。期待は実装出力から採取していない。
