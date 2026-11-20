# 反復内の分岐後に続く文：手計算

`repeat_branch_control.rs` は各 Loop と、`repeat` を使わず回ごとの `if` で書いた Hand の両方を同じ手計算値に照合する。表に載る終端には到達でき、他の終端には到達できない。`D/E/L/T` はそれぞれ `done/early/limit/tail`。値は `経過/作業`（分）、`a–b` は範囲。待ちは作業に数えない。

## 本体の 18 形

各回の審査は 1 分。不合格なら `if` の枝で抜ける。合格なら表の後続文に進む。上限は 2 回で、最後の `again` は `end limit` へ進む。操作は 2 分、固定待ちは 4 分、別の `if` は合格の枝の 5 分待ち。この版では、繰り返しの本体と `on_limit` の `wait until` と `fork` は `U-SEM-0 loop_feature_combination` で拒否されるので、後続文に含めない。これらの後の `leave` は `end done` へ進む。枝と後続で両方 `again` なら、審査結果にかかわらず 2 回実行する。

| 後続文 | 枝 `again` | 枝 `leave` | 枝 `end early` |
|---|---|---|---|
| 操作 | D 3–4/3–4、L 2/2 | D 1–3/1–3 | D 3/3、E 1/1 |
| `wait` | D 5–6/1–2、L 2/2 | D 1–5/1 | D 5/1、E 1/1 |
| `if` | D 6–7/1–2、L 2/2 | D 1–6/1 | D 6/1、E 1/1 |
| `again` | L 2/2 | D 1–2/1–2、L 2/2 | E 1–2/1–2、L 2/2 |
| `leave` | D 1–2/1–2、L 2/2 | D 1/1 | D 1/1、E 1/1 |
| `end tail` | T 1–2/1–2、L 2/2 | D 1/1、T 1/1 | E 1/1、T 1/1 |

## `on_limit` の 18 形

本体の 1 分待ちを 2 回、それから審査 1 分で分岐点は 3/1。`leave` 後は 2 分の操作を実行して `end done`。後続文の追加時間・作業は本体と同じ。`again` は `E-CON-0 loop_again_in_limit` (A) のため、この組合せに受理される Loop はない。

| 後続文 | 枝 `again` | 枝 `leave` | 枝 `end early` |
|---|---|---|---|
| 操作 | A | D 5–7/3–5 | D 7/5、E 3/1 |
| `wait` | A | D 5–9/3 | D 9/3、E 3/1 |
| `if` | A | D 5–10/3 | D 10/3、E 3/1 |
| `again` | A | A | A |
| `leave` | A | D 5/3 | D 5/3、E 3/1 |
| `end tail` | A | D 5/3、T 3/1 | E 3/1、T 3/1 |

## 入れ子、両枝、確率

| 形 | 手計算 |
|---|---|
| 本体 `if fail { if fail { again } }; leave` | 不合格だけ次の回。D 1–2/1–2、L 2/2 |
| 本体 `if pass { end early } else { again }; leave` | 合格で早期終端。E 1–2/1–2、L 2/2 |
| 本体 `if fail { again } else { leave }; leave` | 後続の `leave` に到達しない。D 1–2/1–2、L 2/2 |
| 本体 `if pass { end early } else { leave }; again` | 後続の `again` に到達しない。D 1/1、E 1/1 |
| `on_limit` `if fail { if fail { leave } }; end tail` | 本体は 1 分待ち 1 回。不合格なら外で 2 分の操作。D 4/3、T 2/1 |
| `on_limit` `if fail { leave } else { end early }; end tail` | 本体は 1 分待ち 1 回。D 4/3、E 2/1。T なし |
| 本体 `if fail { 修正; again } else {}; leave`、上限 3 | 審査 1 分、不合格の修正は 2 分。D 1–7/1–7、L 9/9。確率ありの場合、各審査で独立に合格 3/4、不合格 1/4。時刻 1, 4, 7 の D は順に 3/4, 3/16, 3/64、合計 63/64。時刻 9 の L は (1/4)^3 = 1/64 |
