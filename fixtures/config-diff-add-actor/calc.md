# P2: Actor と relation の追加を別々に説明

手計算、単位は分。各 Capability は pass しか返さず、逐次の経路は各 1 本。
AsIs: Office.c 0→3、done=3、elapsed=work=3、result_at=null。
ToBe: Office.c 0→3、Extractor.extract 3→5、done=5、elapsed=work=3+2=5、result_at=null。
actors: {Office}→{Office,Extractor}。relations: {Office.c}→{Office.c,Extractor.extract}。
追加は actor Extractor (reason="automate") と relation Extractor.extract (reason="extract step") の厳密な 2 件、loses=null。
どちらも Cut C、全 Actor を含む。別名同内容 D は Hold が参照し、Before/After も参照済みなので W-CONF-0 無し。
reachable done / safety は両側 satisfied、truncated=false。親 satisfied / exit 0。
root E-DIFF-0/E-CMP-0/W-DIFF-1/W-DIFF-2、各側 W-CUT-0/W-DEADLOCK-0 は不在。
