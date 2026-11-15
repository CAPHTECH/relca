# 請求書の例で手順を回した記録

`examples/invoice` を題材に、SKILL.md の手順を一度回した記録。
確率と時間は `examples/invoice/README.md` のとおり検算用の仮定であり、実業務の測定値ではない。
数値はすべて、先に手計算を書き、その後に relca の出力と照合した。
照合は `scripts/test_design_examples.py` で再実行できる。

## 0. 問いと線

- 問い: 請求書の入力を人から AI 抽出へ移してよいか。
- 線(hard): 誤登録の確率 ≤ 0.001、承認の不変条件、職務分離、権限、案件ごとの登録は1回まで。
- 目的(soft): 登録までの時間、人の作業量。
- 鍵の量の式。誤登録は「抽出が誤り、かつ照合2つと承認がすべて pass」で起きる。
  - 誤りのとき3段とも pass する確率は、易しい案件で 0.2 × 0.15 × 0.1 = 0.003、難しい案件で 0.5 × 0.4 × 0.3 = 0.06 である。
  - よって P(誤登録) = 0.8 × e_r × 0.003 + 0.2 × e_d × 0.06 = 0.0024 e_r + 0.012 e_d。
  - e_r と e_d は、易しい案件と難しい案件での抽出の誤り率。

## 1. 現行を測る

| モデル | exit | 誤登録 | 手計算 |
|---|---|---|---|
| `as-is.relca`(人が入力) | 0 | 93/125000 = 0.000744 | 0.0024 × 0.01 + 0.012 × 0.06 = 0.000744 |

## 2. 現行の検査を強くする

`mutate examples/invoice/as-is.relca` の結果を、呼ばれる操作かどうかで分けた。

| 見逃し | 箇所 | 分類 | 対応 |
|---|---|---|---|
| evidence_missing / original_access_removed | `Extractor.extract` | この流れで呼ばれない(`metrics.capabilities` に無い) | 穴ではない。対応しない |
| evidence_missing / original_access_removed | `Clerk.recheck` | 異議経路(`contest ... route Clerk.recheck`)で呼ばれうる(`starts` 0〜1) | 根拠と版の契約は異議経路では未対応。未検査として残す |
| evidence_missing | `Clerk.enter` の `reads [Invoice]` | 呼ばれる操作で悪化しない | 承認に必要な根拠源を契約にする |
| summary_reused | `Clerk.match_po`、`Clerk.match_receipt` | `no_assigned_check` | 弱さの印ではない。照合した入力と承認する入力が同じ版であることは業務の要件なので、`same_version` を書く |

足した契約(`examples/as-is-strengthened.relca`):

```relca
evidence_basis Approver.approve [Invoice,PurchaseOrder,ReceiptRecord]
same_version Clerk.match_po(e), Approver.approve(e)
same_version Clerk.match_receipt(e), Approver.approve(e)
```

予測と結果(check は exit 0):

| 見逃し | 予測 | 結果 |
|---|---|---|
| `Clerk.enter` の根拠の欠落 | `detected` に変わる | `detected` |
| summary_reused 2件 | `detected` か、検査が割り当てられて `no_worsening` | `no_worsening`。版の検査は割り当てられたが、照合の出力の使い回しは入力の版を変えないので悪化しない |
| `recheck`・`extract` の4件 | 変わらない | 変わらない |

途中で `same_version` が `U-SEM-0 same_version_feature_combination` で拒否された。
process の束縛名 `entry` が、`enter` の書く観測変数 `entry` と同じ名前だったため(`docs/guide/dsl.md` に記載のとおり)。
束縛名を `en` に変えて通した。

## 3. 案を並べて落とす

| 案 | exit | 何が起きたか |
|---|---|---|
| ToBeP(照合の並列化) | 0 | 線をすべて守る |
| ToBeA(AI 抽出) | 1 | 誤登録 231/125000 = 0.001848 > 0.001。職務分離が `vacuous` |
| ToBeC(難しさで振り分け) | 3 | `E-POL-LATENT`。観測できない難しさを条件に使っている |

- ToBeA の `explain` は反例を出さない。確率の性質の違反は、1本の原因経路にしないためである。原因は手計算の式で読む。
- ToBeA の職務分離が `vacuous` なのは、`Clerk.enter` が呼ばれなくなり、分離の前件が起きないから。統制が消えたのであって、守られたのではない。
- 違反している案の `mutate` では、違反中の性質はそれ以上悪化できない。
  - `to-be-a.relca` では、検出11件・`no_worsening` 5件・無効1件だった。検出は確率以外の性質によるもの。
  - `to-be-a-phase1.relca` では、ほとんどが `no_worsening` だった。
  - 違反している案の見逃しを穴とは読まず、直した後に回し直す。

`configurations.relca` の `compare`(1件・上限内)の値は次のとおり。

| 案 | 登録までの時間 | 作業 | 差戻しの確率 |
|---|---|---|---|
| AsIs | 17分 | 17分 | 2407/125000 |
| ToBeP | 14分 | 17分 | 2407/125000 |
| ToBeA | 9分 | 12分 | 5519/125000 |

## 4. 違反した案の余裕を測り、直す

ToBeA の余裕。e_r = 0.02 に固定すると、線を守る条件は e_d ≤ (0.001 − 0.000048) / 0.012 = 0.0793。
`sensitivity` はこのモデルを `sensitivity_feature_combination`(contest を含む)で拒否した。
そこで点ごとのモデルを作り、`check` で確かめた。

| e_d | 誤登録 | 判定 |
|---|---|---|
| 0.15 | 0.001848 | violated |
| 0.10 | 0.001248 | violated |
| 0.08 | 0.001008 | violated |
| 0.07 | 0.000888 | probability は satisfied。全体は vacuous(分離が空虚なまま) |
| 0.05 | 0.000648 | 同上 |

誤り率を下げても、統制が消えた問題は残る。そこで構造を足す。

修理案(`examples/to-be-a-review.relca`):

- 抽出の直後に人の確認 `Clerk.review_entry(entry)` を置き、原本 `Invoice` を読ませる。
  見つけた誤りは差し戻す。誤りを見つける率は仮定で 0.5。
- 職務分離を、新しい人の段と承認の間へ書き直す: `separation Clerk.review_entry.finish, Approver.approve.finish`。
- 根拠の契約: `evidence_basis Approver.approve [...]` と `evidence_basis direct Clerk.review_entry [Invoice]`。

手計算と結果は次のとおり。

| 量 | 手計算 | relca |
|---|---|---|
| 誤登録 | 0.001848 × 0.5 = 0.000924 | 231/250000 |
| 差戻し | 0.023 + 0.5 × 0.044152 = 0.045076 | 11269/250000 |
| 登録までの時間 | 1 + 2 + max(3, 5) + 2 + 1 = 11分 | 11 |
| 作業 | 1 + 2 + 3 + 5 + 2 + 1 = 14分 | 14 |
| 職務分離 | c1/c2 と a1 | satisfied |

- 修理案の検査が効いているかを `mutate` で確かめた。初めは、確認の段で原本を読まなくする変更が `no_worsening` で抜けた。
- 承認の根拠契約は、引数の系譜で AI の抽出から届く Invoice も数える。そのため、確認の段の読取りが消えても満たされたままになる。
- `evidence_basis direct Clerk.review_entry [Invoice]` を足すと `detected` になった。`direct` はその操作自身の `reads` だけを数える。
- 確認の段の差止めを遅らせる `hold_delayed` は `no_worsening` のまま残った。登録の前で止まることは変わらないためで、穴とは見ていない(推論)。

誤りを見つける率 d の余裕。0.001848 × (1 − d) ≤ 0.001 から d ≥ 0.4589。

| d | 誤登録 | exit |
|---|---|---|
| 0.45 | 2541/2500000 | 1 |
| 0.46 | 6237/6250000 | 0 |

`W-LIN-2` は、確認の段(`Clerk.review_entry`)と承認(`Approver.approve`)の両方に出る。
どちらも、AI の誤りと条件付き独立に誤りを見つける、という仮定を指摘している。
人も AI と同じ難しい案件を見落とすなら、0.5 や承認の検出率は過大になる。

## 5. 残った案と、人が決めること

| 案 | 線 | 時間 | 作業 | 実測で確かめる仮定 |
|---|---|---|---|---|
| ToBeP | 守る | 14分 | 17分 | 照合の所要時間 |
| ToBeA + 人の確認 | 守る | 11分 | 14分 | 難しい案件での AI の誤り率、人の確認の発見率 ≥ 0.46、両者の相関 |

- この表は check の出力を人が並べたものである。修理案は `compare` の構成に入れていない。
- どちらを選ぶかは価値判断で、relca は順位を付けない。
- 修理案は、発見率の実測が 0.46 を下回れば線を越える。導入するなら、試行運用で発見率を測ることを条件にする。

## 確かめていないこと

- 1件の案件(`cases 1`)の上限の中だけ。繁忙日の負荷は `simulate` が非決定で `unsupported`(exit 4)になり、確かめていない。
- `conform` は回していない(この例に業務ログが無い)。
- 修理案と ToBeP の `compare` は回していない。
- 仮定の確率は実測ではない。
