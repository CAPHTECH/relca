# req-bypass

ADR 0026 / 計画の第3節の手計算。正例。単位は分。

F11: 3先行作業でt3。ORはPP/PF/FPでPost[3,4)、FFは拒否3。各Jについて他J=fail文脈では停止できるがpass文脈では迂回1なのでconditional/1。ANDはPPだけ開始し他は拒否、両Jともall_paths/0。各判断は実引数の祖先でありguardを新しい根拠源にしない。

変更は `approval == pass || review == pass` → `approval == pass && review == pass` の一箇所だけ。正常 Task の宣言時間は全て正。
期待は仕様の時刻・区間和・値域から先に固定し、実装出力から採取していない。
未実装 RED は意味論の合格ではない。
