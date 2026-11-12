# B4: 別名同内容の Cut

P2 から ToBe の `cut C` 参照だけを `cut D` に変更。C/D の本文は同一でどちらも参照済み。
各側の経路は AsIs: Office.c 0→3、done=3、elapsed/work=3/3、ToBe: Office.c 0→3、Extractor.extract 3→5、done=5、elapsed/work=5/5。
result_at=null、truncated=false、側 report は satisfied。
参照 Cut の名前が違うので E-CMP-0/error、親 invalid / exit 3。内容差は無く mismatch.differences=[]。
actor/relation/intent 比較は未実施。root differences/terminals/properties=[]、checked は comparison_mismatch のみ。
未実施 4 項目を固定 unchecked の末尾に足す。E-DIFF-0/W-DIFF-1/W-DIFF-2 は不在。
