# nondet-race.broken-quick-never-passes

正解版との差分は 1 つ: `quick` の結果集合を `{ pass, fail }` から `{ fail }` にした。速い枝が採用されない構成変更で、fast への到達が消えることを検出する。

## 手計算

単位は分。a は時刻 3 に必ず fail で終わり、採用されない。b は継続し時刻 5 に終わる。

| b の結果(時刻 5) | 採用 | 終了時刻 | 作業量 | result_at | 終端 |
|---|---|---:|---:|---:|---|
| pass | b、時刻 5 | 5 | 3 + 5 = 8 | 5 | slow_ok |
| fail | なし | 5 | 3 + 5 = 8 | — | rejected |

- `if a == pass` は常に偽なので fast は到達不能。b が pass なら採用後に slow_ok。`if a == void` は正解版と同じく到達しない。探索は切断なく完了するので reachable end.fast は violated(inconclusive ではない)。
- slow_ok と rejected は satisfied、safety は satisfied。全体は violated(終了コード 1)。
- 切断なし。W-CUT-0 を期待する。

期待する検出: reachable end.fast の violated。
