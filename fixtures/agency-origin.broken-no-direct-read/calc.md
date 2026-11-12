# F3 単一故障: Reviewer の reads だけ削除

../agency-origin/calc.md の Reviewer.review から reads [Invoice] を1項目削除。
期待は手計算。S(entry)=S(approval)={Invoice}、対象 {Invoice}、独立源は依然空。
1判定1Actor、源0。Reviewer.direct={} になるが indirect={Invoice} は残る。
Reader.direct={Invoice}、Ledger.indirect={Invoice}、他の O 行は不変。

Reach=approval.pass、文脈は {} だけ、fail が止めるので all_paths/0。
fail は rejected2/2、pass は done3/3、result_at=null、truncated=false、satisfied/exit0。
W-AGY-0/1 は不在。独立源の数でなく origin の direct 集合が故障を捕まえる。
