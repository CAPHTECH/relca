# req-kernel

ADR 0026 / 計画の第3節の手計算。正例。単位は分。

F12: 偽guardではt0 deniedへ質量1、outcome=noneのまま、work0、kernel/duration draws0。trueなら[0,4)占有、ok/badが各1/2、outcomeも対応値。denied確率0。正常kernelへelseを混ぜない。

変更は `flag: bool = false` → `flag: bool = true` の一箇所だけ。正常 Task の宣言時間は全て正。
期待は仕様の時刻・区間和・値域から先に固定し、実装出力から採取していない。
未実装 RED は意味論の合格ではない。

## 標本経路の補助受入

4反復ともfalse requiresで拒否されるのでdenied終端4、completed_replications4、各経路のdrawsは0。公開の拒否行はobserved_count4/samples4/coverage sampled/時刻[0,0]。候補を列挙した回数を観測回数へ加えない。未解決の同時刻順序を持つreq-same-timeのsimulateはunsupportedで、標本と拒否行を全撤回する。
