# F7 到着列と件数

ADR 0015からの手計算。cases2=列長2、昇順で合法。
唯一の占有経路[0,2),[2,4)、completed2/4、elapsed2/3、work各2、result_at null。
確率未入力。各範囲最小=最大。全案件done reachable satisfied、占有≤1でsafety satisfied。
受付上限無し、拒否0、clock20内で切断無し。root satisfied。
