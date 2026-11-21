# 並行を含むやり直しを、書き下した版と `repeat` 版で比べた記録

分割して2人で並行に詳細化する仕事で、検査をどこに置くかを比べた。
値はすべて仮定で、実業務の測定値ではない。手計算を先に書き、その後に relca の出力と照合した。
照合は `scripts/test_design_examples.py` の `Unrolling` で再実行できる。

最初の記録(2026-11 上旬)の時点では、`repeat` の本体に `fork` を置けず、契約も `mutate` も `repeat` と併用できなかった。
そのため check-at-end は回ごとに `if` で書き下した。今はどちらも `repeat` で書ける。下の記述は今の挙動に合わせてある。

## 2案

- **split-first**(`examples/split-first.relca`): 分割案を先に確認し、落ちたら分割し直す。通った案を2人が並行に詳細化し、統合して最後に検査する。
- **check-at-end**(`examples/check-at-end.relca`): 分割してすぐ2人が並行に詳細化し、統合して検査する。落ちたら分割からやり直す。

check-at-end のやり直しは並行を含む。どちらの案も、回ごとに `if` で書き下した版と、`repeat` で書いた版を置いた
(`examples/split-first-repeat.relca`、`examples/check-at-end-repeat.relca`)。

## 仮定

- 分割案が健全である確率は 0.7。分割し直すたびに独立に引く。
- 分割案の確認(split-first だけ)は、健全なら必ず pass、不健全なら 0.8 で fail。所要は10分。
- 最後の検査は、健全なら 0.95 で pass、不健全なら 0.1 で pass。所要は10分。
- 時間は、分割 30分、詳細化 60分(2人が並行)、統合 20分、引渡し 1分。
- 分割はどちらも最大2回。split-first は、最後の検査で落ちたら差し戻して終わる(やり直さない)。

## 手計算

split-first:
- 1回目の確認を通る確率は 0.7 + 0.3 × 0.2 = 0.76。内訳は健全 0.7、不健全 0.06。
- 2回目で通る確率は 0.24 × 0.76 = 0.1824。内訳は健全 0.168、不健全 0.0144。2回とも落ちる確率は 0.0576 で、escalated になる。
- 引渡し 0.868 × 0.95 + 0.0744 × 0.1 = 0.83204。そのうち不健全な引渡しは 0.00744。
- 最後の検査で落ちる確率は 0.868 × 0.05 + 0.0744 × 0.9 = 0.11036。
- 時間は、引渡しが 30 + 10 + 60 + 20 + 10 + 1 = 131分、2回目なら 171分。escalated は 80分。
- 作業は、引渡しが 191分、2回目なら 231分。2人の詳細化を 60 + 60 と数える。

check-at-end:
- 1回目で pass する確率は 0.7 × 0.95 + 0.3 × 0.1 = 0.695。そのうち不健全は 0.03。
- 2回目で pass する確率は 0.305 × 0.695 = 0.211975。2回とも落ちる確率は 0.093025 で、escalated になる。
- 引渡しは 0.906975。不健全な引渡しは 0.03 + 0.305 × 0.03 = 0.03915。
- 時間は、1回あたり 120分。引渡しは 121分か 241分、escalated は 240分。
- 作業は、1回あたり 180分。引渡しは 181分か 361分。

## 結果

| | split-first | check-at-end |
|---|---|---|
| exit | 0 | 1 |
| 不健全な引渡し(上限 0.01) | 93/12500 = 0.00744、満たす | 783/20000 = 0.03915、違反 |
| 引渡し | 0.83204 | 0.906975 |
| 引渡しまでの時間 | 131〜171分 | 121〜241分 |
| 引渡しまでの作業 | 191〜231分 | 181〜361分 |
| `evidence_basis` と `same_version` 2本 | satisfied | satisfied |

- 時間と作業は経路の範囲で、期待値ではない。
- check-at-end は、1回で通れば速く、引渡しの確率も高い。ただし、関門が最後の検査だけなので、不健全な分割案を約5倍通す。
- split-first は、安いうちに分割の誤りを止めるので線を守る。ただし、11% が最後で落ちる。

## 書き下した版と `repeat` 版について確かめたこと

- split-first と check-at-end の両方で、書き下した版と `repeat` 版の終端の確率・時間・作業が一致した。check-at-end の `repeat` 版は、上の手計算の値(引渡し 0.906975、不健全な引渡し 783/20000)とも一致した。
- check-at-end の `repeat` 版では、`evidence_basis` と `same_version` 2本が satisfied になった。本体の中で、検査に別の分割案を渡すように壊すと、`same_version` が2本とも violated になった。書き下した版で2回目の検査に1回目の分割案(`p1`)を渡した場合と同じである。
- split-first の `repeat` 版(例のファイルには契約を2本だけ置いている)に `same_version` 2本を足すと、どちらも satisfied になった。`evidence_basis direct Reviewer.check [Spec]` と `evidence_basis Reviewer.check [Spec]` を足すと、どちらも satisfied になった。`repeat` の結果(分割案)を引数に取る、`repeat` の外の操作が対象で、書き下した版に同じ契約を足した場合と同じ判定である。
- `mutate` は、書き下した版の check-at-end では回り、検出11件と `no_worsening` 11件だった。
  - split-first の `repeat` 版でも回る。`evidence_missing`・`hold_delayed`・`summary_reused`・`refusal_path_cut` は変異体が作られ、`authority_expired`・`command_resent`・`receipt_unknown`・`original_access_removed` は `unsupported / loop_feature_combination` になった。全体は exit 4 なので、変異ごとの行を読む。
  - 変異の数は書き下した版と一致しない。`repeat` 版は元の呼出しと分岐を単位に変異を作り、書き下した版は回ごとに別の箇所として数える。
  - check-at-end の `repeat` 版(本体に `fork` がある)は、`mutate` 全体が `loop_feature_combination` で拒否された。

## 確かめていないこと

- この2例での `compare`・`diff`・`sensitivity`、`contest` との併用、`registers` との併用。
- 1件の案件(`cases 1`)だけで、繁忙時の負荷は見ていない。
- 期待値の指標(`metric mean.…`)は、この2例の試験には入れていない。
