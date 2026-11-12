# req-start-siblings

ADR 0026 / 計画の第3節の手計算。正例。単位は分。

F6: 別targetなら同時開始で a/b ともtrue、max(2,3)=3、work2+3=5。同targetなら同値trueでも構造的兄弟競合 U-SEM-0。容量や実行順を理由に受理しない。

変更は `start { b = true }` → `start { a = true }` の一箇所だけ。正常 Task の宣言時間は全て正。
期待は仕様の時刻・区間和・値域から先に固定し、実装出力から採取していない。
未実装 RED は意味論の合格ではない。

## 静的補助行列

兄弟writerはstart/startだけでなくstart/finishとstart/kernelも和集合で競合する。容量0や同値定数でもこの静的規則は不変。startブロック2個、ブロック内の同target2個はU-SEM-0。未知targetはE-NAME-0、boolへenum定数はE-TYPE-0、resultやexecutor.idの式はU-SYN-0。空ブロック自体は合法であり、重複だけを拒否する。
