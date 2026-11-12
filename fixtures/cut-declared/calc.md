# cut-declared

Cut を宣言した model。時間と作業量は serial-time と同じ構成で、Cut の宣言が数値を変えないこと、結果 JSON に Cut が記録されること、`W-CUT-0` が消えることを確かめる(ADR 0007)。

## 手計算

単位は分。Office は capacity 1。a(3 分)を時刻 0〜3、b(5 分)を 3〜8 に順に占有する。

| 区間 | 占有 | 作業量 |
|---|---|---:|
| [0, 3) | a | 3 |
| [3, 8) | b | 5 |

経過 8、作業量 8、result_at null。Archive は宣言されているが process から呼ばれないので、占有も作業量も 0。reachable end.done は satisfied、切断なし(clock 8 ちょうどで終わる)。

## Cut

- include は Office。宣言済みの Actor は Office と Archive の 2 つで、Archive は Actor の exclude にある。両 Actor とも包含か除外のどちらかに記載されている。
- exclude Payment は未宣言の名前なので、モデル外を指すラベル(`actor: false`)。exclude Archive は宣言済み Actor(`actor: true`)。
- defer Office.short.rates は 2 語以上なのでラベル。Office は include にあり、exclude した Actor の一部ではない。
- exclude と defer の 3 件すべてに loses がある。`W-CUT-1` は出ない。
- cut を宣言したので `W-CUT-0` は出ない。診断は 0 件。
- `checked` / `unchecked` は ADR 0007 の固定の一覧。

未決: なし。手計算は binary との照合の前に書いた。
