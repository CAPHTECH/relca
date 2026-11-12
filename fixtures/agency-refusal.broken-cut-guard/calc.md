# F2 単一故障: 内側の j 条件を恒真へ

../agency-refusal/calc.md の After の内側条件式だけ j.pass OR j.fail に置換。
Before は不変(all_paths/0、done4/4、rejected3/3)。
After Reach=(x.pass AND (j.pass OR j.fail)) OR x.fail=true。
簡約前の条件support j,x は失わない。
j の文脈は {x=pass},{x=fail}、両方全 j で到達、none/bypasses2。
x の文脈は {j=pass},{j=fail}、両方全 x で到達、none/bypasses2。
W-AGY-0 が必要、W-AGY-1 不在。After rejected は消え、全4組 done4/4。
result_at=null、truncated=false、satisfied/exit0。

系譜: S(entry)=S(j)=S(x)={Invoice}、After J=j,x、各対象 {Invoice}、独立源空。
群は2判定2Actorで源0。Before は1判定1Actor。
O は両側 Reader.direct={Invoice}、Judge/Flag/Ledger.indirect={Invoice}、他は空。
実装結果は期待値の根拠として使用していない。
