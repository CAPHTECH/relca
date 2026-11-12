# B5: relation の loses だけを削除

P3 から `loses "extraction removed"` だけを削除。actor の loses は保持。
AsIs: Office.c 0→3、Extractor.extract 3→5、done=5、elapsed/work=5/5。
ToBe: Office.c 0→3、done=3、elapsed/work=3/3。両方 result_at=null、min=max、truncated=false。
removed の厳密集合は actor Extractor と relation Extractor.extract の 2 件で不変。
relation の reason="remove step" は残り loses=null になる。W-DIFF-2/warning、親 satisfied / exit 0。
E-DIFF-0/W-DIFF-1/E-CMP-0 は不在。check 自体では W-DIFF-2 を出さない(計画 D6)。
