# ADR 0057 独立レビュー裁定の手値

根拠: 設計 v0.1 §16・§18、設計 v0.2 §5.3・§5.4・§20・§22.3、ADR 0057 決定2〜7。
期待値は実行前に固定する。実装の stdout を期待値へ移す機能はない。
試験本体は log_conformance_review.rs。入力は既存 support/log_conformance_inputs.rs の
手書き模型・ログに明記した変更を適用して作る。新しい production の変更は行わない。
M4 の五故障注入には別の実行証拠が必要で、この手値だけでは実施済みとしない。

## 共通の境界

各試験は先に模型の syntax/lower の成功を要求する。誤った DSL を意図した RED と数えない。
実 CLI と core 入口を各々同じ手値へ照合する。両者が同じ実装を通ることを独立参照とは呼ばない。
CLIの手値照合が落ちてもcoreの手値照合まで実行し、どちらか一方でも失敗なら試験を失敗にする。
実 CLI の wire 全必須欄、三値/verdict/exit、stderr 診断、入力ファイル非更新も検査する。
実行時の模型・JSONL はソース木内の .tmp に保存する。
新しいログ reason は diagnostics.reason を読み、message の部分一致を使わない。

## M1: permit と executors

Poster.executors={p,q}、permit={p}。Task t1 は start0、finish1、committed。
差は1分、正常 commit は1件なので once と lifecycle は適合する。

| 試験の末尾 | 対照からの一変更 | 手値 |
|---|---|---|
| permit_inside_is_conformant | 基準: t1=p | 有効集合{p}に属する。conformant/satisfied/0 |
| permit_outside_known_person_is_violating | t1 全体を q に再割当 | q は既知、executors 内だが permit 外。permission の正の証拠(start/finish)、E-LOG-2、violating/violated/1。unmapped にしない |
| permit_unknown_person_stays_unknown | t1 全体を未対応IDに再割当 | 人を推測できない。no_executor_mapping、unknown/inconclusive/2。E-LOG-2なし |
| without_permit_executors_are_the_fallback | permit を除いた模型で t1=q | 有効集合{p,q}。conformant/0 |
| multiple_permits_form_a_union | q の permit を一条追加し t1=q | permit の和{p,q}。conformant/0。q を一度割当てたログは直前の許可外対と共通 |

## M2: Capability 各欄の分類

基準は同じ t1。Ack に denied を加え、Desk容量1、観測 snapshot=pass、潜在 hidden=false、
Poster の executors={p}、共有 state marker=false / capacity1 を共通に宣言する。capacity1 は共有writerの
既存lowerの境界を守るためであり、ログ再生の証明ではない。取消は forbidden_after_start と明示し、結果は committed 一値。
marker/snapshot/hidden はログ payload に入れず、潜在を業務条件に読ませない。
Clerk は共通に source document と宣言し、reads が未宣言の原本で静的エラーにならないようにする。

| 欄 / 試験 | 基準への一機能の追加 | 手値と故障対照 |
|---|---|---|
| uses | uses Desk:1 | 固定時間/結果/commitは不変。conformant/0、Poster.post と uses を特定する not_checked/outside_scope/span。行を省く実装を拒否 |
| params | key:CaseId と call(case.id) | 引数内容は未照合。上と同じ、欄名 params |
| reads | reads Clerk | 静的原本系譜のみ。上と同じ、欄名 reads |
| read_variables | reads snapshot | 静的観測系譜のみ。上と同じ、欄名 read_variables |
| requires | marker==false else denied | 状態前条件は記録から認定できない。U-LOG-1、unsupported/4 |
| atomic | marker==trueならdenied、それ以外marker=trueを書きcommitted（writes宣言つき） | 状態依存の正常結果を再現しない。同上。片枝の書込みはlowerが要求するatomicの形 |
| version_check | marker のcheckと必要なfinish書込み | 共有stateの開始以降の版を再現しない。同上。書込みはlowerが要求する依存で、独立の二故障ではない |
| start_writes | startでmarker=true | 開始効果をログから再現しない。同上 |
| writes | writes marker と finishでtrue | 終了効果をログから再現しない。同上 |
| kernel | 無条件にcommittedを返すkernel | kernel意味のログ照合は対象外。同上。確率をログ頻度で更新しない |
| kernel_reads_latent | hiddenごとの二行kernel、両行committed | 潜在読取り付きkernel。同上。潜在の実値をログから認定しない |
| kernel_writes | kernelから案件局所の潜在hidden=true | kernel潜在書込み。同上。writes宣言は構文上必須の依存。Actorの観測stateをkernelに使う未対応構文と混同しない |
| duration_distribution | 1分/2分それぞれ1/2 | 一観測の1分は分布の適合証明でない。U-LOG-1、unsupported/4 |

全非対応対は unknown check、DSL span、実在の t1-start 証拠、U-LOG-1、
not_checked/unsupported_semantics、summary.unsupported>0 を要求する。
空欄の基準模型は0で、存在しないuses/params/reads/read_variablesのnot_checkedを捏造しない。
feature_inputs_really_populate_the_named_ir_fields は各変更が実際に同名のIR欄を埋めることを
全欄の構造体分解で検査する。欄追加時に受入台帳もコンパイルエラーになる。
productionの機能検査も構造体分解に追従しているかは、この試験だけで保証しない。
name/actor/span・有限return_type/results・固定duration・commits・cancelは既存72件とM1で対応を検査する。

## M3: 入力を切ったときの全体 unknown と、既読の件数

まず t1 s0/f2、所要1分の対照は差2≠1。違反check数は1、violating/1。
同じTaskのfinishを異なるevent_idで一行だけ加えると、単一Taskの二重記録となり、
二回実行でも確実なduration違反でもない。duplicate_phase、違反数0、unknown/2。
この小さな全読込対照が、未読の側で前提が覆り得る根拠である。

| 試験 | 読める範囲 / 未読の変更 | 手値 |
|---|---|---|
| event_cut_with_prefix_violation_and_unread_duplicate | 5000Taskの10000event。最初だけs0/f2で他はs(2i)/f(2i+1)。10001行目のeventは最初のfinishの二重記録 | event_limit、truncated、全体unknown/inconclusive/2。summary.checks.violating=1を別に残す |
| line_cut_with_prefix_violation_and_unread_duplicate | s0/f2の後に65536bytesを超すeventを挟み、その後ろに同じfinishの二重記録 | line_limit、truncated、全体unknown/2。既読event=2、違反check数1 |

両対とも log.sha256/log.lines は null、late-duplicate を読んだ証拠は出さない。
W-LOG-4と、既読のduration違反checkを残す。全体unknownはE-LOG-2の消去を要求しない。
order_limitだけで独立した局所違反を残す既存core試験は意味が変わらない。

## minor の手値と壊した対

| 裁定 / 試験 | 正例と一変更 | 手値 |
|---|---|---|
| m1 future_header_with_new_field | v2 headerに新欄一つ | v1の未知欄検査よりschema選択が先。U-LOG-0、unknown/unsupported/4 |
| m1 future_header_does_not_require_v1_clock | v2 headerからclockだけ除く | v1の必須欄を将来版へ適用しない。同上 |
| m1 v1_header_with_new_field | 正常v1に同じ新欄一つ | 既知版の不正、E-LOG-0、invalid/3 |
| m2 order_cycle_has_its_own_machine_reason | 同tickの二頂点に双方のafter | 時刻自体は整合するが明示辺が循環。E-LOG-1/reason=order_cycle、invalid/3 |
| m2 inconsistent_time_has_a_different_machine_reason | s0にf1のafter、逆辺は無し | afterだけなら非循環、しかし時刻と両立しない。E-LOG-1/reason=inconsistent_time_evidence、invalid/3 |
| m2 unknown_after_has_a_machine_reason | 正常対へ存在しないIDへのafter一つ | E-LOG-0/reason=unknown_after、invalid/3 |
| m3 unsupported_capability_survives_duplicate_phase_distrust | requires付きの対に同じTaskのfinishを一行追加 | duplicate_phaseも残しU-LOG-1行を消さない。unknown/unsupported/4 |
| m3 unsupported_capability_survives_unmapped_actor_distrust | requires付きの対のActor IDを未対応に再割当 | 未対応一覧とU-LOG-1行の両方を保持。unknown/unsupported/4 |
| m3 header_exact_byte_boundary | clockをpaddingしheaderを65536bytes、UTF-8の3byte文字を含む | headerはeventに数えず、conformant/0、truncated=false |
| m3 header_over_byte_boundary | 同じheaderにASCII一byte追加 | 観測範囲を読めない不正な入力。E-LOG-0/reason=header_line_limit、invalid/3、checks空 |
| m4 explicit_finish_before_start_after | 同tickのstart.after=finish | 意味的s→fと宣言f→sが矛盾。E-LOG-1、invalid/3 |
| m4 time_only_lifecycle_reversal | after無しでs2/f1 | 宣言矛盾でなく正のライフサイクル違反。violating/1、E-LOG-1なし |
| m4 explicit_normal_lifecycle_after | f1.after=s0 | 正常な因果。conformant/0 |

header長の算式は新clock長=目標bytes−置換前header長+旧clock長、
日本語一文字の3bytesを除いた残りをASCIIで埋める。LFを数えない。
既存72件の入力・意味期待はこれらの裁定で変更が必要なものを含まないため変更0件。
旧検査の診断codeだけの確認を新しいreason機械欄の保証と読み替えず、専用の三試験で補う。
