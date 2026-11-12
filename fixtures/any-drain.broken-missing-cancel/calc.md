# any-drain: 取消契約欠落

## 単一故障

正解版 any-drain の long Capability から cancel forbidden_after_start の 1 行だけを削除した。
short の cancel、remaining cancel_then_drain、その他の宣言は変えていない。
ADR 0001 の取消規則と共有診断により、取消が要求されうる Capability の cancel 未宣言は E-CANCEL-0 / invalid。

## 経路と期待結果

正解版の開始イベント順 a, b / b, a はどちらも a [0, 3)、b [0, 5)。
a の pass を時刻 3 に採用すると、実行中の b への取消要求が必ず発生する。
その契約が無いので静的に拒否し、未宣言を forbidden_after_start や cooperative とみなしてはならない。
正解版の経過 5 分・作業量 8 分・採用時刻 3 分は故障版には出力しない。
ADR 0002 により性質は評価せず properties と terminals は空、全体は invalid。
Cut は undeclared、W-CUT-0 も期待する。
未決: なし。手計算のみで、実装との照合はしていない。
