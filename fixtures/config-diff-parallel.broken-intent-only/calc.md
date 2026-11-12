# B1: 計算に無い add

P1 からの唯一の変更は ToBeP.intent に `add Office reason "unused claim"` を追加。
Serial: short 0→3、long 3→8、done=8、work=3+5=8。
Parallel: short 0→3、long 0→5、join/done=5、work=3+5=8。全開始順で同値。result_at は両方 null。
actors={Office}、relations={Office.short,Office.long} は両側同じ。計算差分は依然空。
余剰主張は E-DIFF-0/error。両 report は satisfied のまま、親だけ invalid / exit 3。
比較は可能なので terminals と properties を空にしてはいけない。W-DIFF-1/W-DIFF-2/E-CMP-0 は不在。
