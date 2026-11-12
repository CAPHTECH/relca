# req-observed

ADR 0026 / 計画の第3節の手計算。正例。単位は分。

F3: 観測 true を読むと開始し4分で終了。宣言種だけ latent に変えると静的 E-POL-LATENT / invalid、探索なし。定数に見えても真値読取りは禁止。

変更は `observed case` → `latent case` の一箇所だけ。正常 Task の宣言時間は全て正。
期待は仕様の時刻・区間和・値域から先に固定し、実装出力から採取していない。
未実装 RED は意味論の合格ではない。

## 型・可視性・自己開始書込みの補助受入

- 同型のenum拒否値deniedは正常結果集合{ok}の外でも合法。boolのtrueをelseにするとE-TYPE-0、未知変数はE-NAME-0、boolとokの比較はE-TYPE-0。
- 未呼出しHidden.unusedのrequiresがcase潜在truthまたはHidden.local潜在を読む場合もE-POL-LATENTで探索しない。生存経路や定数値を理由に検査を省かない。
- runにstart flag=false / writes flag / finish flag=trueを付ける場合、開始前のtrueでrequiresを評価してからfalseを書く。Start invariant flag=falseはsatisfied、t4にはtrueに戻りend done4/work4。自分の開始効果を先読みして拒否してはならない。
