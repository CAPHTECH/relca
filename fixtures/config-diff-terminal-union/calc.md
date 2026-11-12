# P5: 終端の和集合と片側 null (計画 D3)

手計算、分。Old/AsIs: Office.c 0→3、end old=3、elapsed=work=3、result_at=null。
New/ToBe: Office.c 0→3、end new=3、elapsed=work=3、result_at=null。各構成 1 経路。
model-wide の終端 inventory は {old,new}。AsIs は reachable old=satisfied/new=violated、ToBe は逆。
safety は両側 satisfied、各 report と親は violated / exit 1、truncated=false。
親 terminals は old.from=3/3, old.to=null と new.from=null, new.to=3/3。存在しない側に 0 を作らない。
静的 actors={Office}、relations={Office.c} は同一。終端・性質の変更には intent 不要、W-DIFF-1 は不在。
