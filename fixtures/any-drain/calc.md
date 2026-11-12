# any-drain

## 手計算

単位は分。ADR 0001 の join any・remaining・取消規則と設計 v0.1 §6.1・§6.4・§18 に従う。
capacity 2 と work-conserving により両枝は時刻 0 に開始し、結果はそれぞれ pass のみ。

| 経路(開始イベント順) | a の占有区間 | b の占有区間 | 採用 | drain 完了 | 作業量 |
|---|---|---|---|---:|---:|
| a, b | [0, 3) | [0, 5) | a、時刻 3 | 5 | 3 + 5 = 8 |
| b, a | [0, 3) | [0, 5) | a、時刻 3 | 5 | 3 + 5 = 8 |

時刻 3 に a を採用して result_at = 3 分を記録し、b へ取消要求を送る。
b は既に開始済みで forbidden_after_start のため要求を無視し、時刻 5 に通常終了する。
要求時点で資源を解放したり作業量を止めたりせず、経過 5 分、作業量 8 分。
tie by [a, b] は必須の同時採用規則を満たすが、この所要時間では同時候補は無い。

drain 後は a == pass が真で done。if none の飛び先 no_match は else 側で定義してあるが到達不能。
reachable end.done と全体は satisfied。生存 Task は最大 2、clock 5 分ちょうどで終わり切断は無い。
ADR 0002 により Cut は undeclared、警告 W-CUT-0 を期待する。
未決: なし。手計算のみで、実装との照合はしていない。
