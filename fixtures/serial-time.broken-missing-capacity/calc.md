# serial-time: capacity 宣言欠落

## 単一故障

正解版 serial-time から Office の capacity 1 の 1 行だけを削除した。
正の所要時間 3 分・5 分の Capability がある Actor の容量が未宣言なので、ADR 0001 の資源規則と共有診断 E-CAP-0 により静的検査で invalid。
容量を既定の 1 として実行してはならない。

## 経路と期待結果

正解版の経路は a [0, 3) → b [3, 8)、経過 8 分、作業量 3 + 5 = 8 分。
故障版は実行前に拒否するため、時刻・作業量の計算対象となる経路は無く、terminals は空。
ADR 0002 により性質は評価せず properties は空、全体は invalid。
Cut は undeclared、W-CUT-0 も期待する。
未決: なし。手計算のみで、実装との照合はしていない。
