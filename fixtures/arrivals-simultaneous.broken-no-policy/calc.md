# F1 単一故障: policy 行だけを削除

基底 calc.md の4つの独立積(各1/4、和1)を、case1先とcase2先の各非決定で使う。
先行案件の within1=1/2、within2=1/2、後行案件は0と1/4。
各案件を別々に最適化して within1 min0/max1/2、within2 min1/4/max1/2。
開始順を確率1/2で平均しない。各閾値は全順で満たしroot satisfiedのまま。
終了・elapsed は各1..6、work1..3、result_at null、End確率1、拒否0。
容量1の共有を保つので safety satisfied、truncated false。
