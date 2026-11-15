# 段ごとの観測の手計算（ADR 0074、段階 1）

これは現 binary の出力を写した期待値ではない。`inputs.py` の既存 DSL から、
以下の集合演算と分数を先に求める。ADR は構文を追加しないので、旧 DSL への
書換えは恒等写像である。各入力そのものを parser と lower に通す `Substrate` を置く。
模型は仮定であり、実業務の測定値ではない。

## 1. 演算と再計算

源の全集合を I = {Invoice} とする。段 s の直読 D(s) はその操作の reads の
source Actor のみ。祖先の集合 A(s) は引数・観測変数をたどる上近似。
V(s) = A(s) − D(s)。V は観測ではない。
責務の必要源 R(s) は引数と自分の reads だけをたどり、M(s) = R(s) − D(s)。
人全体の直読を P とすると W-RESP-3 の対象は M(s) ∩ P。
W-RESP-1 の対象 R(行) − P の規則は変えない。

| 模型 | D(e), V(e) | D(v), V(v) | D(x), V(x) | P(c1) | R(v), M(v) | 行の O | W-RESP-1 / 3 |
|---|---|---|---|---|---|---|---|
| S1 artifact | I, ∅ | ∅, I | ∅, I | I | I, I | present | 0 / 1 |
| S1 reread | I, ∅ | I, ∅ | ∅, I | I | I, ∅ | present | 0 / 0 |
| S2 artifact | I, ∅ | ∅, I | ∅, I | I | I, I | present | 0 / 1 |
| S2 reread | I, ∅ | I, ∅ | ∅, I | I | I, ∅ | present | 0 / 0 |
| S3 other | I, ∅ | ∅, I | ∅, I | ∅ | I, I | missing | 1 / 0 |
| S3 reread | I, ∅ | I, ∅ | ∅, I | I | I, ∅ | present | 0 / 0 |
| S4 variable | I, ∅ | ∅, I | ∅, I | I | 未導出 | not_derived | 0 / 0 |
| S4 reread | I, ∅ | I, ∅ | ∅, I | I | I, ∅ | present | 0 / 0 |
| S5 repeat | I, ∅ | ∅, I | ∅, I | I | I, I | present | 0 / 1 |
| S5 reread | I, ∅ | I, ∅ | ∅, I | I | I, ∅ | present | 0 / 0 |

S1 は同じ Actor、S2 は別 Actor だが同じ人、S3 は enter だけ c2。
S4 の agency は観測変数の書き手をたどるが、責務はたどらない。
そこで S4 artifact の行・段はともに no_source_identified、段配列は null。
空の missing を成功扱いしない。unchecked の responsibility_observation を保持。
S5 の字句上の v は一つ。実行の 2 回を 2 段とも 2 警告とも数えない。
lineage の既存の別名は @loop:rework:1:v と @loop:rework:2:v のまま。

分数で時間を独立に再計算する。各操作は 1/1 分。S1〜S4 の posted は
1/1 + 1/1 + 1/1 = 3/1 分、returned は 1/1 + 1/1 = 2/1 分。
S5 は初回 pass なら 3/1、fail 後 pass なら 4/1、2 回 fail なら returned 3/1。
逐次なので作業量も同じ。repeat の v の行数は 2/2 = 1、警告は 1/1 = 1 件。
非決定を一様分布にしない。posted も returned も確率区間は [0/1,1/1]。
反復回数の平均や pass 確率 1/2 は求めない。到達性は existential なので
上の全模型は check/explain satisfied、exit 0。bounds は切断しない。
simulate は pass に固定して 4 標本、case_outcomes に posted count 4。
到達した reachable は satisfied、safety は反例が無ければ inconclusive。

## 2. 追加の境界

- PerSite: one は Z のみ、two は A のみを要求。人 a は両方を他の段で読む。
  各段の missing はそれぞれ一源、警告は行に 1 件。段 one に A を混ぜない。
  新しい配列の人は [a,z]、源は昇順。旧配列の宣言順を変更しない。
- reads [e] は引数名であって直読ではない。if/requires/wait の条件、潜在 kernel
  の読取りも直読にしない。源として宣言された記録は普通の源として数え、由来を推測しない。
- Merge: v は ready に依存。direct は ∅ と導けるが via は null、
  via_outputs_status = not_derived、reason = merge_control。
  責務も stages = null、stages_status = not_derived、同じ reason。
- 状態条件はその変数の書き手のうち責務の人の段だけ。書き手が c1 で責務が c2
  なら no_person_stage。Capability の非実行者は person_not_executor、
  保守は maintenance。異議 route を正常 process の段に偽装しない。
- observation の計算上限は not_derived(derivation_limit)。未導出を空配列にしない。
- W-LIN-2、lineage、lineage_groups、common_causes の既存値に段の direct を戻さない。
  reread の independent_sources は ∅（I − I = ∅）。源の数は増えない。

## 3. JSON の補足裁定（2026-11-15）

段の鍵は call（既存の呼出し位置名）、actor、capability（既存 lineage と同じ修飾名）、
people、unattributed、direct、via_outputs、via_outputs_status。
via_outputs_reason は not_derived のときだけ存在し、配列は null。
derived では配列（空も可）を持ち、reason の鍵は持たない。
責務 observation は stages、stages_status、必要なときだけ stages_reason を足す。
各段の源は required、direct、missing。段の順は既存の呼出し位置順、
源と人は名前昇順。新欄だけの契約であり、既存欄の順や nullable reason は変えない。
via_records は後から via_outputs と同じ並びで追加でき、今は存在を要求しない。

| 欄 | status | reason（固定一覧） | 配列 |
|---|---|---|---|
| via_outputs | derived | 鍵なし | 源の配列 |
| via_outputs | not_derived | merge_control、derivation_limit（上限時は block 自体の未導出も可） | null |
| 責務 stages | derived | 鍵なし | 段の配列 |
| 責務 stages | not_derived | no_source_identified、no_person_stage、target_not_called、merge_control、derivation_limit | null |
| 責務 stages | not_applicable | person_not_executor、maintenance | null |

not_applicable を 2 値の指示で消さない、という再裁定も受領した。
diff/compare は unchecked に stage_observation と理由
stage_correspondence_not_defined を同じ項目に残す。比較していないことを satisfied と言わない。
fallback_path、rule、contest_path は ADR の指定の unchecked 理由であり、
CLI 全体の unsupported の新理由ではない。
rule は元の3操作のまま（3/1 または 2/1 分）、contest は元の3操作に
正常経路の段を増やさない。異議の権限は明示した ReviewGrant を根拠とする。
Fallback は Probe 1/1 + Worker 1/1 = 2/1 分で、案件別の case_outcomes に出る。
Merge は enter 1/1 + fix 1/1 + confirm 1/1 = 3/1 分。PerSite は4操作で4/1分。
control だけの先行関係で post の引数を外した対は V(x) = ∅。
source record への変更は源の同一性を変えず、由来を生成しない。

## 4. 変更生成と故障校正

S1 reread に種類 9 original_access_removed を適用:

| 削除位置 | 変更後 R(v), D(v), M(v) | 結果 |
|---|---|---|
| confirm:reads:Invoice | I, ∅, I（P = I） | detected、責務の段欠落と W-RESP-3 |
| enter:reads:Invoice | I, I, ∅ | undetected / no_worsening |

種類 0 evidence_missing の割当ては変更しない（責務の変化で detected にしない）。
F1〜F7 は手書きした正常な将来 JSON を観測 seam で壊し、同じ受入判定器が
拒否することを検査する。これは未実装 production への故障注入と同じ保証ではない。
実 binary で GREEN の旧入力保存・拒否・基盤試験は --inject で入力時間または
診断理由を一時破壊し RED を確認する。src は一切変更しない。

## 5. 旧出力を byte で保存する検査

baseline_bytes.py は基点 308177b2 の全 snapshot を git の blob として読み、
現行 snapshot の stdout/stderr/exit と照合する。実行結果から手値を作らない。
履歴に依存するこの検査器は必須の Cargo 試験から切り離して明示実行し、
公開版への書き出しからはファイルを一つだけ正確に名指して外す。
隣の既存 DSL 入力と受入試験をまとめて除外してはならない。
JSON を parse して再出力すると空白や順序の変化を見逃すので、字句の byte 範囲だけを
取り去る。許すのは observation の追加 stages 関連欄と、U10 の新しい unchecked 項目、
明示指定された stage_observation(fallback_path)、stage_observation(rule)、
responsibility_stage(contest_path) だけ。基点にない新しい snapshot の追加は許し、
基点の snapshot の削除・改名は許さない。
それ以外（独立性、既存欄、診断、終了値など）の差は一つでも失敗する。
W-RESP-3 が既存の snapshot に新出する場合も黙って削除して比較を通さず、
その意図した差を別途裁定する必要がある。検査器の故障対照は、既存数値、
源、空白、stderr、exit の 1 byte を変えて失敗を確かめる。

## 6. 確かめた範囲

RED/GREEN と故障校正の実行記録は作業結果に残す。手書き JSON の故障対照は
判定器の非空虚性だけを示し、実装の成功を意味しない。公開語の検査と clippy は
別途実行する。利用者の外部モデル、段階 2 の must の辺、版・由来の判定は対象外。
