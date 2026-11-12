# 正確率候補だけを1分遅らせる

正解版の3minを4minへ変更、問いの期限3は変えない。ADR 0014・ADR 0013 による手計算。
候補1,4,100の重み0,1,0、唯一の正経路は4分。hit within3 の min=max0、>=1 violated。
done 確率1、elapsed/work min=max4、result_at null。0重み1/100は展開しない。
safety satisfied、root violated/exit1、切断なし、W-CUT-0 のみ。
