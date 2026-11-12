# B3: relation の理由だけを削除

P2 から唯一 `add Extractor.extract reason "extract step"` を削除。
AsIs: Office.c 0→3、done=3、elapsed/work=3/3。ToBe: Office.c 0→3、Extractor.extract 3→5、done=5、elapsed/work=5/5。
result_at は両方 null。actors/relations の追加集合は P2 と同じ 2 件。
actor Extractor の reason="automate" は保持。relation Extractor.extract の reason/loses=null。
Actor の intent は relation に流用できないため W-DIFF-1/warning が必要。
両 report と親は satisfied、exit 0。E-DIFF-0/E-CMP-0/W-DIFF-2 は不在。
