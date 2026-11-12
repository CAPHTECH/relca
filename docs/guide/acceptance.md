# 受入項目と根拠

39 項目の ID、要約、分類、根拠の対応です。番号は来歴の識別子です。
`covered` はこの表の対象となる受入があること、`later` は当初の対象から後続へ分けたこと、
`partial` は一部を分けていることを示します。開発段階の完了表や現在の全機能一覧ではありません。
たとえば有限再送やログ照合の現在の入口は、[DSL](dsl.md) と [結果](results.md) を参照してください。

`dir:` はモデル・手計算・期待値のあるディレクトリ、`test:` はソースと Rust 試験名です。
`scripts/test-phase1-acceptance.py` は39行の順序・要約と根拠の存在を照合します。
手計算の正しさや意味の検証は、その試験の実行と別の根拠を必要とします。

<!-- phase1-acceptance-v01:start -->
| ID | 要約 | 分類 | 根拠と限定 |
|---|---|---|---|
| V01-01 | 有限型と確率の静的拒否 | covered | `dir:fixtures/prob-sum`、`dir:fixtures/prob-rows.broken-missing-difficult`、`dir:fixtures/state-policy.broken-latent-difficulty`。和や条件の不正を拒否 |
| V01-02 | 逐次の所要時間 | covered | `dir:fixtures/serial-time`。3+5=8分 |
| V01-03 | 並行の所要時間 | covered | `dir:fixtures/parallel-time`。max(3,5)=5分、work8 |
| V01-04 | 容量による直列化 | covered | `dir:fixtures/resource-contention`。容量1で8分 |
| V01-05 | 先着採用後のdrain | covered | `dir:fixtures/any-drain`。採用3分と終了5分を区別 |
| V01-06 | 取消要求とackの間隔 | covered | `dir:fixtures/coop-cancel`、`dir:fixtures/met-cancel`。要求後も ack まで時間と費用を数える |
| V01-07 | 同時刻の採用規律 | covered | `test:crates/relca-core/src/engine.rs::ties_use_the_complete_simultaneous_cohort_not_finish_iteration_order`。明示した tie の比較 |
| V01-08 | writer間の衝突 | covered | `dir:fixtures/cf-version`、`dir:fixtures/cf-version.broken-no-check`。衝突結果と無解決の拒否 |
| V01-09 | 再送での二重登録 | later | 後続へ分けた行。`dir:fixtures/aset-static-retry.broken-retry` は集合操作の構文での拒否対照。現在の有限再送とは別の受入 |
| V01-10 | 初期誤りと見逃しの積 | covered | `dir:fixtures/k8-conditional-error`、`dir:fixtures/k8-conditional-error.broken-miss-rate`、`dir:fixtures/k8-conditional-error.broken-tight-upper`。1/50×1/5=1/250、変更側1/200 |
| V01-11 | 共通障害の同時失敗 | covered | `dir:fixtures/k8-common-cause`、`dir:fixtures/k8-common-cause.broken-independent-upper`。共有障害で同時失敗1/10、独立積1/100ではない |
| V01-12 | 再実行間の依存 | later | 後続へ分けた行。固定不良10%なら反復しても失敗10%となる問い。初期潜在の固定だけではこの受入を代用しない |
| V01-13 | 終了と回復の区別 | covered | `test:crates/relca-core/src/completion.rs::m22_exit_self_loop_is_recoverable_but_not_terminating_and_closed_loop_is_neither`、`dir:fixtures/done-clock.broken-short-clock`。内部グラフのループと製品 DSL の切断を別に扱う |
| V01-14 | 排除後にも残る義務 | covered | `dir:fixtures/ctx-obligation`、`dir:fixtures/ctx-obligation.broken-excludes-target` |
| V01-15 | 実行者IDの分離 | covered | `dir:fixtures/exec-separation`、`dir:fixtures/exec-separation.broken-shared-approver` |
| V01-16 | 非活性制約の分類 | covered | `dir:fixtures/done-vacuity`、`dir:fixtures/contract-vacuous`。到達不能の警告と vacuous を区別 |
| V01-17 | 計算切断の保留 | covered | `dir:fixtures/prob-cut.broken-short-clock`。[1/2,1]を成功へ再正規化しない |
| V01-18 | 標本と解析値の区別 | covered | `dir:fixtures/sim-statistical`、`test:crates/relca-core/tests/simulation.rs::s_statistical_consistency_is_not_exhaustive_probability_equality`。解析値1/2と固定seedの4956/10000を混同しない |
| V01-19 | 欠けたログの不明判定 | later | 後続へ分けた行。現在の conform は観測ログを照合するが、この表の分類を全ログの検証完了という意味にしない |
| V01-20 | 比較条件の整合 | covered | `dir:fixtures/compare-terminal`、`dir:fixtures/compare-terminal.broken-other-end`。終端集合の不一致を拒否 |
<!-- phase1-acceptance-v01:end -->

<!-- phase1-acceptance-v02:start -->
| ID | 要約 | 分類 | 根拠と限定 |
|---|---|---|---|
| V02-01 | grain変更による非干渉 | covered | `dir:fixtures/act-grain`、`dir:fixtures/act-grain.broken-swapped` |
| V02-02 | 省略欄の報告 | covered | `dir:fixtures/act-defaults`、`test:crates/relca-core/tests/actor_table_cut_debtor.rs::f2_all_twelve_default_fields_and_only_value_filling_assumptions`。試験名の twelve は旧名で、表は13欄 |
| V02-03 | 匿名実行者の検査限界 | covered | `dir:fixtures/exec-anonymous.broken-no-executor` |
| V02-04 | Cutなしの警告 | covered | `dir:fixtures/act-cut-context.broken-no-cut` |
| V02-05 | Actorの包含漏れ | covered | `dir:fixtures/cut-declared.broken-missing-member` |
| V02-06 | partyの経路不足 | covered | `dir:fixtures/resp-cut.broken-no-explicit-cut`。警告は当事者の同意の判定ではない |
| V02-07 | 異なるCutの比較拒否 | covered | `dir:fixtures/config-diff-add-actor.broken-other-cut` |
| V02-08 | intentと実差分の対応 | covered | `dir:fixtures/config-diff-parallel.broken-intent-only`、`dir:fixtures/config-diff-add-actor.broken-missing-intent` |
| V02-09 | 共通根拠の重複防止 | covered | `dir:fixtures/agency-lineage`、`dir:fixtures/agency-lineage.broken-reused-source`。3判定でも根拠源は2、変更側は1 |
| V02-10 | 根拠不足の独立性警告 | covered | `dir:fixtures/lin-invoice`、`dir:fixtures/lin-basis` |
| V02-11 | チームと個人の重複容量 | covered | `dir:fixtures/act-composite.broken-team-work` |
| V02-12 | 判定と停止経路の区別 | covered | `dir:fixtures/agency-refusal`、`dir:fixtures/agency-refusal.broken-cut-guard` |
| V02-13 | 使用ゼロの縮退操作 | covered | `dir:fixtures/agy2-keep-fallback`。回数0と intent を別に表示 |
| V02-14 | 資源近似と台帳機構の対照 | partial | `dir:fixtures/k8-cut-r`、`dir:fixtures/k8-cut-g`、`dir:fixtures/k8-cut-r.broken-slow-post`、`dir:fixtures/k8-cut-g.broken-slow-post`、`test:crates/relca-core/tests/acceptance_gaps.rs::failure_free_cuts_have_equal_time_and_wrong_registration_not_equal_mechanisms`。単一案件で両側2分、誤登録1/50。変更側は登録だけ1→2分。この模型の応答喪失・再送の対は未対応であり、この数値の一致から機構の一致を主張しない |
| V02-15 | 真値による分岐の拒否 | covered | `test:crates/relca-cli/tests/invoice_phase1_artifact.rs::live_single_fault_keeps_truth_hidden` |
| V02-16 | 許可と授権の別検査 | covered | `dir:fixtures/exec-authority.broken-missing-grant` |
| V02-17 | 規則改定前の授権 | covered | `dir:fixtures/rule-self-authorize.broken-t05`、`dir:fixtures/rule-not-by.broken-t04` |
| V02-18 | 異議の構造的表示 | covered | `dir:fixtures/resp-contest`、`test:crates/relca-core/tests/responsibility_standing_contest.rs::r_structural_simulation_and_compare_global_rows`。発生率や受諾の統計として数えない |
| V02-19 | 請求書モデルの数値受入 | covered | `test:crates/relca-cli/tests/invoice_phase1_artifact.rs::live_quality_time_waits_and_quantiles_follow_hand_fractions`、`test:crates/relca-core/tests/rule_quality.rs::invoice_rule_and_no_rule_hand_risks_in_both_interpreters`。17/14/9分、16/16/10人分、93/125000・93/125000・231/125000。全件免除Bの81/12500を条件付き免除Bの未設定値と同一視しない |
<!-- phase1-acceptance-v02:end -->
