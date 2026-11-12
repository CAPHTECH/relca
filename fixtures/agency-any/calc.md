# F4: any は各判定に迂回を作る

独立手計算(ADR 0009・計画の第4節)、時間は分。
S(entry)=S(a)=S(b)={Invoice}。a,b は where が読む実引数非空の判定。
各対象 {Invoice}、独立源空、群は2判定2Actorで0源。
O: Invoice空、Reader.direct={Invoice}、JA/JB.indirect={Invoice}。
Ledger.post() は無引数なので Ledger.indirect={}。条件の読取りと引数を混ぜない。

Reach(posted)=a.pass OR b.pass。
a の文脈は {b=fail}(a=failで停止), {b=pass}(どのaでも到達)。
b の文脈は {a=fail}(b=failで停止), {a=pass}(どのbでも到達)。
両方 conditional/1、W-AGY-1、W-AGY-0 不在。
PP,PF,FP は done、FF のみ if none が rejected へ即時終了(gotoではない)。
tie 順序・同時finish順序は文脈を増やさない。

entry1 + max(a1,b1)=elapsed2、work=1+1+1=3。
done は post1 を足し elapsed3/work4/result_at2、rejected は2/3/null。
drain 枝は取消不可を明示、容量はActorごと1。切断なし、satisfied/exit0。
対称な where pass→fail の変異はこの集約wireでは区別不能なので採用pairにしない。
