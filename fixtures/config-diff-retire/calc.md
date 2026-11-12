# P3: 退役の理由と失う能力

手計算、分。AsIs: Office.c 0→3、Extractor.extract 3→5、done=5、elapsed=work=3+2=5。
ToBe: Office.c 0→3、done=3、elapsed=work=3。result_at は両方 null。各構成の経路は 1 本。
actors {Office,Extractor}→{Office}、relations {Office.c,Extractor.extract}→{Office.c}。
removed actor Extractor: reason="remove automation", loses="automation removed"。
removed relation Extractor.extract: reason="remove step", loses="extraction removed"。
宣言 Actor は消さず C に両方 include。Cut 変更ではない。両側 reachable done/safety は satisfied、truncated=false。
全 retire に loses があるので W-DIFF-2 不在。他の diff 診断も不在。親 satisfied / exit 0。
