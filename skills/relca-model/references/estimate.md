# 実績ログから所要時間と確率の候補を出す

`relca estimate` は、対応づけできた実績ログから操作の所要時間と kernel 行の確率の候補を出す。モデルは書き換えない。候補は As-Is の観測窓のもので、将来や To-Be の確率ではない。ログの真正性・代表性・捕捉率は人が確認する。

## モデル、ログ、指定ファイルを固定する

1. `relca-log/v1` の JSONL を用意する。先頭の header に時刻の窓、案件の両端点の申告、Actor と実行者の対応を記す。同一案件・同一 `task_id` の start と finish を結べるようにし、戻り値は finish にだけ記す。分からない値を推測で埋めない。
2. `sha256sum model.relca events.jsonl` で元ファイルの SHA-256 を得る。下の二つの hash を実値に置き換える。ファイルを直したら再計算する。hash は同じ bytes を照合するものであり、真正性の証明ではない。
3. `slots` は `"all"` または対象 ID の配列。初回は `"all"` で `slots[].slot.id` を確認できる。業務規則・到着など導けない宣言も含まれるため、その場合の exit 4 を推定値の失敗と混同しない。対象を狭めるなら ID を明記して再実行する。

```json
{
  "schema": "relca-estimate-spec/v1",
  "model_sha256": "ここをモデルの小文字64桁のhashに置き換える",
  "log_sha256": "ここをログの小文字64桁のhashに置き換える",
  "confidence": "19/20",
  "min_samples": 30,
  "max_support": 10,
  "decimals": 3,
  "slots": ["duration.Rev.review", "kernel.Rev.review.Review.1"]
}
```

`duration.Actor.Capability` は所要時間。
`kernel.Actor.Capability.Kernel.ROW` は確率表の行（宣言順、1 始まり）。モデルの版・構成が変われば ID を見直す。

```sh
relca estimate model.relca --log events.jsonl --spec estimate-spec.json > estimate.json
```

複数の構成があるときは `--configuration NAME` を追加し、確認用の `check` または `explain` も同じ構成を選ぶ。`estimate` の実際の終了コードを控え、確認ページの `--estimate estimate.json --estimate-exit N` に渡す。

## 信頼水準と最低件数は目的に合わせて決める

例の 19/20（95%）と 30 件は、探索的に候補を見るときの出発点であり、既定値でも十分性の判定でもない。まれな結果を扱うなら、30 件では未観測でも珍しくない。必要な区間幅を先に決め、必要なら対象ごと・回ごとに観測を増やす。`min_samples` と比較するのは既知の件数 `n−unknown`。開始が確認できなかった案件は n にも足さない。

Clopper–Pearson の区間は、案件が同じ条件で独立に観測されたという前提に依存する。選んだ枝・回・時間の枠数によって多重性の補正がかかるため、同じ n でも対象を増やせば区間は広がり得る。未観測の枝は確率 0 の候補にせず、件数と上端を読む。

## 回ごとの表は混ぜずに読む

`rounds` の第2回の n は、第2回の開始が確かめられた案件の数。第1回と第2回の件数を足して独立な標本にしない。`given [NAME.round]` の各行を該当回と照合する。無条件 `*` の行を反復内で使う場合は回ごとの表だけを出し、まとめた候補は出さない。

呼出し箇所が複数ある、回の途中で対象を飛ばせる、対象に未開始拒否がある、または開始を静的に確認できない場合は回への割当ができない。`occurrences_per_case` は案件ごとの出現数の分布で、例えば「観測2回の案件が3件」であり「第2回に3回成功」ではない。同時刻の複数 start、観測数の上限超え、先頭側の欠落も、回の候補を確かめる前に `reason` を確認する。

## 理由語を見て再実行か保留を決める

[欄ごとの理由語](../../../docs/guide/results.md#推定の欄ごとの理由語)と[全体の理由語](../../../docs/guide/results.md#推定全体の理由語)で、残った件数・区間・候補を読む。withheld は観測や指定を見直せる保留。not_derivable は同じ要求にログを足すだけでは解消しない。保留の厳密値や回別の候補も、人の確認なしに転記しない。

| 理由語 | 次にすること |
|---|---|
| not_requested、no_evidence、unknown_slot | 構成と欄 ID を照合する。必要な欄を選び直す。 |
| no_samples、insufficient_samples、zero_count_branch | 対応表と窓を確認する。目的に合う区間幅まで観測を増やし、未観測を 0 としない。 |
| dsl_precision_insufficient | [転記時の丸め](#モデルへの転記はレビューを通す)を確認する。 |
| zero_duration_observed、time_precision | 採取時刻の粒度を人に確認する。正の時間を推測で足さない。 |
| support_too_large | 許容する種類数を人に確認する。観測した時間を勝手に統合しない。 |
| censored、partial_coverage、missing_start | 先頭から終了までの記録を求める。採れない部分は保留する。 |
| cancelled | 取消を完了にしない。採取元に完了との区別を確認する。 |
| lifecycle、conflicting_terminal、duplicate_phase、task_identity_mismatch | 元の記録で Task の対応を確かめる。矛盾を解消するまで値を転記しない。 |
| missing_result、result_domain | 元の戻り値とモデルの枝を照合する。不明の戻り値は補わない。 |
| unmapped_actor、unmapped_operation、bad_mapping | Actor・実行者・操作の対応と版を確かめる。対応は推測しない。 |
| multiple_observations_per_case、round_order_ambiguous | 元の開始と順序を確認する。同じ案件を分割せず、行順で回を割り当てない。 |
| rounds_exceed_model_limit | 実際の上限と記録を人に確認する。ログに合わせて上限を変えない。 |
| latent_condition、observed_condition | 周辺件数を別に報告し、条件別の値は人に上げる。 |
| branches_not_distinguishable、nondeterministic | 観測で区別できる枝と、確率を置く業務上の根拠を人に聞く。 |
| ambiguous_call_site、round_not_identifiable、no_call_site | 呼出しと観測を照合する。実際の手順が異なる場合だけモデルを直し、回が確定しないなら出現数だけを報告する。 |
| round_pooling_not_supported | 回を混ぜずに表を示す。回で条件づけるかは人が決める。 |
| query、recovery、retry、response_loss | 照会、復旧、再送、応答喪失を通常の開始・完了に読み替えない。使える記録を人と確認する。 |
| latent_initial、observed_initial、arrival、performance | 操作イベント以外の根拠を人に求める。初期値や流入、性能をこの表から補わない。 |
| business_rule、contest_rates、wait、gap、backoff | 規則・率・待ちの根拠を人に確認する。時間の候補で宣言を上書きしない。 |
| truncated、header_line_limit | 上限と記録形式を照合する。窓を変えるなら対象と端点を人と決め直す。 |
| merge_feature_combination | 合流の意味を保てる別の調査を人に相談する。 |
| argument、input、spec_input、spec_bad_field | 指定、ファイル、権限、形式を確認してから再実行する。 |
| spec_hash_mismatch | 同じ入力を使うのか確認する。実際に読むファイルの hash を計算し直す。 |
| bad_field、malformed_jsonl、duplicate_key、duplicate_event_id | 採取元に戻り、ログの欠落・重複・誤記を確認する。 |
| unknown_after、order_cycle、inconsistent_time_evidence | 記録した順序と時刻を採取元に照会する。推定のためだけに並べ替えない。 |
| schema、time_unit | 形式と単位を採取元に確認する。変換の根拠が揃うまで待つ。 |
| multi_branch_row、no_statistical_bound | 補足欄を報告する。二枝の参考点や平均の区間を補わない。 |

## モデルへの転記はレビューを通す

AI が候補をモデルに写す場合は、厳密値、DSL 用の値、標本数、区間、ログ窓、採用理由を並べる。時間は経験分布を優先し、平均・中央値・最小・最大のうち一つを機械的に固定所要に選ばない。分布に載せられない 0 分、過大な support、打切りや時刻幅は理由を見る。平均には統計区間がない。

`dsl_precision_insufficient` は、指定桁で丸めると観測済みの枝が 0 になるという意味。厳密値は残るが同じ行の `dsl` はすべて null になる。転記するなら桁数を増やして再実行し、丸め後も観測した枝が残ることを確かめる。

人が採用前に、ログの採取方法、案件の偏り・独立性、対応表、打切り、適用先、丸め後の和と業務上の意味を確かめる。`assumption_records` は候補の出所であり、既存宣言の値を裏づけたという申告でも、モデルの `basis` に貼る文でもない。モデルを変更したら、以前の版の推定 JSON を新しい版の確認ページへ流用しない。
