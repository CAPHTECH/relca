# F4 単一故障: a の結果集合だけ singleton へ

基準 ../agency-any/calc.md。JA.check の result {pass,fail} だけを {pass} に変更。
enum Verdict 自体は2値のまま。期待を enum 全体から計算しない。
Reach=a.pass OR b.pass。a の文脈は {b=pass},{b=fail}。
a の許された値は pass のみで、どちらの文脈でも止められず none/2。
b の充足可能文脈は {a=pass} のみ、全 b で到達するので none/1。
W-AGY-0、W-AGY-1 不在。PP,PF の全2組が done、rejected は到達不能。
done elapsed3/work4/result_at2、切断なし、satisfied/exit0。

系譜は不変 S(entry)=S(a)=S(b)={Invoice}、対象 {Invoice}、独立源空、2判定2Actor。
O は Reader.direct={Invoice}、JA/JB.indirect={Invoice}、Invoice/Ledger空。
本期待は実装出力でなく手計算から固定。
