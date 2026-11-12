# parallel-time: join 欠落

## 単一故障

正解版 parallel-time の join all checks の 1 行だけを削除した。
ADR 0001 の fork 文は直後に join を必須とするため、共有診断 E-SYN-0 / invalid を期待する。
範囲外の新構文を足したのではなく、既知の fork 文が不完全なので unsupported ではない。

## 経路と期待結果

正解版は開始イベント順 a, b / b, a のいずれも a [0, 3)、b [0, 5)、経過 max(3, 5) = 5 分、作業量 8 分。
故障版は構文検査で拒否するので実行経路は無く、terminals は空。
暗黙の join を補って 5 分と出したり、drain せず done としてはならない。
ADR 0002 により性質は評価せず properties は空、全体は invalid。
Cut は undeclared、W-CUT-0 も期待する。
未決: なし。手計算のみで、実装との照合はしていない。
