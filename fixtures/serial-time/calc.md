# serial-time

## 手計算

単位は分。ADR 0001 の呼出し・work-conserving・作業量の規則、および設計 v0.1 §6.4・§18 に従う。
各結果は pass のみで、逐次呼出しなので経路は次の 1 本。

| 経路 | a の占有区間 | b の占有区間 | 終端時刻 | 作業量 |
|---|---|---|---:|---:|
| a → b | [0, 3) | [3, 8) | 8 | 3 + 5 = 8 |

呼出しは終了まで process を待たせる。経過時間は 3 + 5 = 8 分、作業量も 8 分。
join any が無いので result_at は null。done に到達するため reachable は satisfied、全体も satisfied。
生存 Task は最大 1、clock は終端時刻と同じ 8 分なので切断されない。
取消要求が無いので cancel 宣言は不要。

ADR 0002 により Cut は undeclared、警告 W-CUT-0 を期待する。
未決: なし。手計算のみで、実装との照合はしていない。
