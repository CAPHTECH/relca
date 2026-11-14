# 結果の読み方

stdout は JSON が 1 つ、stderr は診断が 1 行に 1 つ(`CODE: message`)。exit は verdict で決まる。
対応表は `crates/relca-core/src/lib.rs` の `Verdict::exit_code`。
本書の EC 試行は、電子商取引の注文・決済を有限再送つきで記述した試行モデルを指す。
`../scripts/testdata/ec-plain/` には plain 案の `model.relca` と表示試験用の固定 JSON の抜粋がある。個別の観測を全モデルへ一般化しない。

| verdict | exit | 読み方 | 言ってよいこと |
|---|---|---|---|
| `satisfied` | 0 | 宣言した性質をこのモデル・この上限・この Cut で満たす | 「モデル上は満たす」。業務で成り立つとは言わない |
| `violated` | 1 | 少なくとも一つの性質が破れる | 「破れる」。反例は explain で一例。原因とは言わない |
| `inconclusive` | 2 | 計算上の切断などで判定できない | 「判定できていない」。合格・不合格へ丸めない |
| `invalid` | 3 | モデルか入力が不正(静的エラー、読めない path、選択子不足) | 「モデルが通っていない」。性質は評価されていない |
| `unsupported` | 4 | 対応外の構文・意味・バックエンド・非決定 | 「確かめられていない」。合格扱いにしない |
| `vacuous` | 5 | 前提が一度も起きず、空虚に成り立つ | 「検査が働いていない」。満たすとは言わない |

引数の誤り・読込み失敗などで模型が未読の report は hash が null、Cut は unresolved。
構文解析で止まった場合も、Cut の有無は分からないため Cut は unresolved、W-CUT-0 は出さない。
ただし、読めた入力の hash は残る。ソース上に Cut があっても同じ扱い。
parse が通り Cut 未宣言と分かった場合は、その後の型検査や lower で止まっても W-CUT-0 を出す。
compare・diff の各案、mutate の original、sensitivity の各点、conform でもこの区別を守る。
ただし conform は、parse 後の lower 拒否でも Cut が unresolved のままになる既存の例外がある。
そのとき Cut 未宣言なら W-CUT-0 は出る。unresolved や E-SYN-0 / U-SYN-0 だけから parse 失敗や警告無しと判断しない。
explain の構成指定が不足したときは、stderr の --configuration の案内に従う。

### 全体の verdict

check・explain・simulate の report(compare の各案の report も同じ)の全体の verdict は、次を `invalid > violated > unsupported > inconclusive > vacuous > satisfied` で畳む(`crates/relca-core/src/report.rs` の `fold_verdict`):

- 性質ごとの verdict
- diagnostics の severity が error のもの: code が `U-` で始まれば unsupported、それ以外は invalid
- 性質が一つ以上あるときの `W-DEADLOCK-0`: violated(到達できる deadlock は反例)
- 上のどれも無い(性質が空で error も無い)とき: unsupported

warning は中立で、畳み込みに入らない。
後段(ledger の完成処理)で全体が重くなる場合があるかは確かめていない [U]。examples/invoice の実出力では、全 report で畳み直した値と一致した。

compare と diff の最上位は、通常は各案(diff は from と to)の report の verdict と、compare・diff 自身の error の診断を同じ順で畳む(`crates/relca-core/src/compare.rs` の `fold` と `diff.rs`)。
**例外:** 次の拒否では、畳んだ後で最上位を unsupported に上書きする。各案の report が violated でも最上位は unsupported になり、畳み込みより軽い。

- `priority_feature_combination`(compare・diff)
- `retry_feature_combination`(compare・diff。再送を含むモデル)
- `condition_wait_diff_unsupported`(diff だけ)
- `merge_feature_combination`(compare・diff。合流を含むモデル)

EC 試行(再送を含む 2 案)では、compare も diff も最上位 unsupported・exit 4、各案の report はどちらも violated だった(1 回)。各案の性質の verdict は `check --configuration` の結果と一致した。
この拒否では単純な畳み込みだけで最上位を再計算できない。読むときは次のとおりにする。

- 最上位の unsupported は「比較・差分をしていない」という意味で読む。
- 案ごとの結論は `plans[].report.verdict`(diff は `from.report.verdict`・`to.report.verdict`)から読み、**両方を報告する**。案の violated を最上位の unsupported で打ち消さない。
- exit は最上位の verdict に対応する(上の例は 4)。exit だけ見て案の違反を見落とさない。

review_page は、check・explain・simulate・各案・mutate の original を `fold_verdict` と同じ規則で畳み直す。compare の最上位は各案の verdict の最も重いものと照合する(`scripts/review_page.py` の `scan` の `kind == "compare"` の分岐)。どちらも全体の verdict が軽ければ「全体の verdict を信用しない」と警告する。上の拒否では、この警告が出るのが正しい。
review_page の端末・ページ冒頭・結論の未確認件数は、性質や入力の保留に加えて、
Agency の `not_derived` と根拠不明、繰り返しの `coverage: not_derived` も同じ集計で数える。
繰り返しは `repetition.coverage` の1項目として数え、出所と場所を含む項目単位で重複を除く。
HTML内の `review-data.unverified_count` も同じ値。従来の項目別リストは残し、
自己申告の未検査、未実施、警告、AIの申告はそれぞれの欄で読む。
したがって **全体が violated でも、中に vacuous や unsupported の性質が残る**。to-be-a は全体 violated、separation は vacuous だった。

## 必ず見る欄(check / explain / simulate)

| 欄 | 中身 | 注意 |
|---|---|---|
| `verdict`, `mode` | 全体の判定と分析モード(`exhaustive` / `simulation`) | exit と照合する |
| `model.sha256` | モデルファイルの sha256 | `sha256sum` と一致した(as-is・to-be-a で 1 回ずつ確認) |
| `cut` | 宣言した Cut の object、または `"undeclared"` / `"unresolved"` | `"undeclared"` でも exit 0 になりうる |
| `bounds` | `cases / live_tasks / clock / truncated` | `truncated: true` なら切断に関わる性質は inconclusive |
| `properties[]` | `id`, `verdict`, 確率なら `probability.min/max`, `notes` | `notes: ["mechanism_unmodeled"]` は仕組みをモデル化していない印 |
| `terminals[]` | 終端ごとの `elapsed` / `work` の min..max、確率 | 全到達経路の範囲。平均ではない |
| `assumptions[]` | `field / value / affects` | 既定で埋めた欄・置いた仮定の一覧。報告に必ず付ける |
| `actors[]` | Actor の各欄と `status: declared / default` | `default` は省略時既定。業務の人に確かめてもらう対象 |
| `diagnostics[]` | `code / severity / message / span{start,end}` | span はモデルの **バイト位置**(lexer が byte span) |
| `agency` | origin・lineage・refusal・common_causes・viewpoints | 入力構成から導出する欄で、自己申告の値は受けない。実運用の効果を測定した値ではない |
| `responsibilities[]` | 人・対象・原本への到達・拒否の到達 | W-RESP-1(観測が無い)・W-RESP-2(拒否が効果に届かない) |
| `metrics` | 時間・費用・件数。`rates` に単価、`not_configured` は未設定 | 単価未設定は 0 円ではない |
| `uncertainty` | aleatory / estimation / model の三区分 | モデル不確実性に Cut と系譜の仮定が入る |
| `ledger.producer.version` | relca の版(現在 `0.0.0`、出所 `cargo_package`、build identity は未記録) | `relca --version` は無い |
| `ledger.execution.selection.effective.process` | 実際に選ばれた process | 複数 process のモデルで確かめる |
| `unchecked`, `agency.unchecked`, `cut.unchecked` | relca が自分で未検査と申告した項目 | 報告の「確かめていない範囲」に写す |
| `response_receipts`, `case_outcomes`, `response_analysis`, 性質 `receipt_resolution` | 再送を含むモデルで出る(試行の再送なしの版には無かった。他の Phase 2 の構文で `case_outcomes` が出るかは確かめていない [U]) | 下の「再送を含むモデルで出る欄」 |
| `repetition` | repeatを含むモデルの回数、上限後の処理への到達、元位置との対応 | 下の「繰り返しを含むモデルで出る欄」 |

確率の `min / max` は完全情報の緩和境界で、実現できる方策の値ではない(assumptions の `probability.scheduler`)。

## 合流した値の読み方

[合流の例](../../../examples/merge-if/model.relca)のcheckはviolated・exit1。
done_fixedは確率1/4・5分、done_unchangedは3/4・3分。手計算の期待時間7/2分と、
metricsの経路範囲3〜5分を混同しない。Taskはd・r・f・sの4行で、readyはTaskにならない。
同例のexplainは、時刻4に`kind: merged`、`binding: ready`、`source: f`を示す。
既存の`result`に複写した観測値が入り、`location.span`はyield文。
`actor`・`capability`・`executor`は必須nullで、独立参照が選択元と値を再現できない反例の再生はunsupportedになる。

合流値を直接・間接に引数として受ける判定は、lineage行を出さず、
`agency.unchecked`に`lineage(merge_control): 呼出し名`を残す。
全枝の根拠の和集合や積集合を一つの確定した系譜として報告しない。
依存しない判定の系譜、原本への直接・間接到達、commit実行者は保持する。
originの間接到達はActor単位の可能な経路の集約で、全経路共通の根拠や独立検証数ではない。
F/U・行使回数・責務や警告の独立した行は従来どおり導出し、合流値やその制御に依存する項目だけを
`not_derived`、理由`merge_control`にする。
未導出の一覧はW-AGY-0/1/2/3、W-RESP-0/1/2、W-LIN-2。
対象の呼出し・操作・責務の名前とともにagency.uncheckedへ残す。
責務のobservation/refusalも欄ごとに区別し、独立したW-RESP-1などは出す。
同じ判定が合流依存の効果にも関わる場合はrefusal欄全体がnot_derived(merge_control)となり、
独立commitへの拒否欠落もW-RESP-2の警告ではなく未導出の印になる。
derivation_limitは実際の計算上の上限で、未接続を意味しない。
警告が出ないことを、統制が成立した証拠にしない。

確認用ページは値を返すif、各yieldの選択元、Taskでない複写を表と図に出し、
反例のmerged段も表示する。拒否された合流は正常な流れとして描かない。
細部は[合流の出力](../../../docs/guide/results.md#合流の出力)を参照。

## 根拠源の要求と証跡

`evidence_basis` は `contract Basis #1 evidence_basis` のような性質行。
check の行には id/verdict と必要時の cases だけで、新しい根拠欄は無い。
対象が一度も正常終了しなければ vacuous、計算切断なら inconclusive。
simulate の反例なしを全スケジュールの satisfied と読まない。
対象の判断の引数・観測読取りへ合流値が直接・間接に流れ込む組合せはU-SEM-0 merge_control。
無関係な合流や条件だけの参照は根拠へ数えず、契約を通常どおり判定する。

explain の違反した反例 object だけに `evidence` が付く。
[読取りを外した実例](../../../examples/evidence-basis-missing.relca)では、正常終了は時刻2分で、
binding=p、actor=Clerk、capability=match_po、required=[Invoice,PurchaseOrder]、
sources=[PurchaseOrder]、missing=[Invoice]、excluded_edges=[]。
後半4欄は昇順の配列。excluded_edges は `p -> a` のような、数えなかった観測変数の辺である。
sources はモデルの照合に数えた集合で、実際の資料参照を観測したログではない。
確認用ページは source の契約とこの証跡を表示し、欠けた・不整合な証跡を正常な空欄にしない。

mutate の evidence_missing はこの行も使う。original_access_removed が使うのは direct だけ。
正の実例の2/2検出はその入力の件数であり、他のモデルの検出率や網羅率ではない。
契約が悪化しても同じ変異で系譜行が消える場合は、既存規則によりinconclusive / assigned_check_unavailableになりうる。
異議経路の対象は、その性質だけU-SEM-0 evidence_basis_contest_route。他の判定は残す。
compare/diff/sensitivity は U-SEM-0 evidence_basis_unsupported_command で拒否する。
diff の両側の個別検査が violated でも、最上位unsupportedは比較未実施を示す。両方を報告する。
conform は U-LOG-1、not_checked に unsupported_request、unchecked に evidence_basis を残す。
詳しくは[利用ガイド](../../../docs/guide/results.md#根拠源の要求と証跡)を参照。

## 繰り返しを含むモデルで出る欄

| 欄 | 読み方 |
|---|---|
| `repetition.coverage`, `unchecked` | complete / sampled / not_derived と算出しなかった理由。業務の成功とは別 |
| `repetition.sites[].origin` | LoopSite。loop_nameと元のcall_site、bodyのround（1始まり）を対応付ける。phase=on_limitならround=null |
| `repetition.cases[].rounds`, `distribution[]` | 案件・loopごとの開始回数のmin/maxと回数別分布。確率未宣言のprobabilityはnullで、一様にはしない |
| `cases[].on_limit`, `on_limit_probability` | その案件で上限後の処理へ進む件数0〜1の範囲と確率境界。回数上限と計算切断を分ける |
| `cases[].samples`, `excluded_samples`, `distribution[].observed_count`, `observed_on_limit` | simulateの完了・非切断標本の集計と除外数。網羅的な確率ではない |

LoopSiteはmetricsのTask行とexplainのBusinessEventにも付く。元の呼出しの位置を使い、
実行単位の識別子から回数を推測しない。[差し戻しの例](../../../docs/guide/dsl.md#差し戻しと再提出)では
roundsは1〜3、on_limitは0〜1、on_limit_probabilityは[1/8,1/8]。
再確認の例は2回・上限後0件で、doneは7分。

clockや探索予算での切断は、影響する性質をinconclusiveにする。上限後の処理へ進んだことにはせず、
未完了を成功・失敗にも数えない。not_derivedの空のcasesを0件と読まない。
詳しい鍵と値は[利用ガイド](../../../docs/guide/results.md#繰り返しの出力)を参照。
確認用ページでは、案件・loop ごとの回数範囲と分布、上限後の件数と確率、標本数と除外数、
各 site の元の呼出し・round・phase・行と列を表にする。`not_derived` は未確認の件数に加え、
空の `cases` を0件の結果として表示しない。`rules`・`basis`・`unchecked` も併記する。

## 確認用ページの --assumptions

`--assumptions` は Relca 本体の入力ではなく、`scripts/review_page.py` に渡す出所の申告。
list または `{"assumptions": [...]}` の JSON とし、各要素に `target`、`value`、`basis`、
`status`（`placeholder` / `confirmed` / `measured`）を書く。
`confirmed` には `confirmed_by`、`measured` には `source` が必要で、欠ければ仮置きに戻す。
どちらも AI の申告で、人が確認済み・実測済みと認定するものではない。

target はページの「モデルの数値・選択の宣言と出所」にある名前を使う。
存在しない宣言の target と、重複した target は警告になる。主な target は次のとおり。

| target | 対応する宣言 |
|---|---|
| `<Actor>.<Capability>.duration` / `.uses` | 所要時間 / 資源の使用量 |
| `<Actor>.capacity` / `<Actor>.<潜在変数>` | 容量 / 潜在の初期分布 |
| `<Actor>.via_model` | 経由するモデルの版。`Extract.via_model` は `via model "doc@v3"` に対応 |
| `<NAME>.at_most` | 繰り返しの上限回数。例: `rework.at_most`。初回を含む業務の回数で、計算の切断ではない |
| `<NAME>.gap` | 明記した繰り返しの間隔。例: `poll.gap`。省略時既定の0分には宣言の target を作らない |
| `kernel.<名前>` / `case.<変数>` | 確率表 / 案件の初期分布 |
| `metrics.rate.<対象>` / `metrics.window` | 単価 / 分析窓 |
| `resource.<名前>` / `policy` / `arrivals` | 資源の容量 / 方策 / 流入 |
| `scenario.<名前>.arrivals` / `.queue` / `.concurrent` / `.horizon` | シナリオ内の宣言 |
| `verify.probability#<k>` / `contract.<名前>#<k>` | 確率の上限 / 数値を持つ契約の期限 |
| `retry.<結果の名前>` / `response_loss.<結果の名前>` | 再送 / 応答喪失。同名の結果が複数 process にあれば `retry.<process>.<結果の名前>` など |
| `<Actor>.<Capability>.idempotency` / `.effect_scope` | 冪等キー / 効果の範囲の仮定 |

たとえば `{"target":"poll.at_most","value":3,"basis":"説明用の仮定","status":"placeholder"}`。
bounds と simulation は計算の設定なので、この宣言一覧ではなく上限などの欄で読む。

## 再送を含むモデルで出る欄

呼出しに `response_loss` か `retry` を書くと出る(`response_loss none` だけでも出た)。型は `crates/relca-core/src/response_inputs.rs` と `outcome.rs` を参照する。下の例は EC 試行の check(案 A = G 形、案 B = modeled 形、単一案件)と一時モデルの各 1 回の観測。

| 欄 | 中身 | 読み方 |
|---|---|---|
| `response_receipts[]` | `case_id / command(元の binding 名) / attempt(1 始まり) / capability / receipt(delivered・unknown) / at{min,max} / observed_result / resolved_effect` | **到達しうる観測の集合で、実行順・確率の一覧ではない。**同じ試行に unknown の行と delivered の行が両方あるのは「どちらの経路もある」という意味。unknown の行の `observed_result` は null |
| `resolved_effect` | その受信で、その command の効果の未解決が閉じたか | G 形: 2 回目 delivered `rejected_duplicate` で true(同じキーの効果が閉じる)。modeled 形: 1 回目が unknown なら、2 回目 delivered `charged` でも **false**(過去の unknown は後の受信で消えない) |
| `case_outcomes[].terminals[]` | 案件ごとの到達先。`kind: end` は `end`・`completed_at`・`elapsed`・`work`。`kind: receipt_unknown` は `end`・`completed_at`・`elapsed`・`work` が null で、`observed_until`・`observed_elapsed`・`observed_work` だけを持つ | receipt_unknown の行は終端ではなく、結果不明で止まった案件。完了時間・成功・失敗として数えない。非決定のモデルでは `probability` は 0..1 の範囲で、発生率ではない |
| `properties[]` の `receipt_resolution` | 自動で足される性質。`cases[]` に案件ごとの verdict | 結果不明に到達しうるなら inconclusive、しないなら satisfied(`sequence [unknown, delivered]` の G 形、`response_loss none` で satisfied を観測)。他に違反があると全体は violated・exit 1 になり、**inconclusive は全体の verdict の下に隠れる**。性質ごとに見る |
| `response_analysis` | `rules / coverage / business_horizon / scope / checked / unchecked` | checked は `retry_bounded`・`receipt_channel`・`receipt_not_success`。unchecked の `asynchronous_external_task`(timeout 後も取引先で処理が続く)・`general_status_query`(状態照会で解決する)・`correlated_response_loss`・`unbounded_retry`・`fairness` は報告の「確かめていない範囲」に写す。`coverage: complete` は探索の範囲で、業務の完了ではない |
| `assumptions[]` の `response.<binding>` | `max_attempts / backoff / loss`(`Nondet`・`None`・確率と basis) | 再送の上限と間隔は業務の値か仮置きかを仮定一覧に書く。確認用ページは source の retry・response_loss を `retry.<binding>`・`response_loss.<binding>` の宣言として表に出し、この行と照合する(食い違えば警告)。`--assumptions` にはその target で申告する |
| `assumptions[]` の `agency.diagnostics.W-AGY-0` 等 | 値が `unchecked(response_control)` | 再送を入れると W-AGY-0・W-AGY-2・W-LIN-2・W-RESP-2 を導出しなくなった。警告が消えたのは解決したからではない。「確かめていない」と書く |
| `metrics.overall.counts.receipt_unknown_count` | 結果不明の件数の範囲 | `unfinished` の内数で、`completed` に足さない。2 件同時到着の版で 0..2、unfinished も 0..2 だった |

`response` 契約(`… otherwise unresolved`)は、結果不明の案件を違反に数えなかった(試行の一時変形で 1 回: response satisfied、receipt_resolution inconclusive、全体 inconclusive・exit 2)。response 契約だけを見て「必ず閉じる」と書かない。

explain では、再送した呼出しの `steps[]` に `attempt` と `observed_receipt` が入る(試行の案 B: 1 回目 finish が `unknown`、2 回目 finish が `delivered` で `result: charged`)。応答が失われた試行の `result` は null だった。

## explain

`explanation.counterexamples[]` に反例。各 `steps[]` に `time / case_id / actor / capability / executor / result / kind / location{line,column}` と、違反の段に `violation`。
`explanation.omitted[]` は反例を出さなかった性質と理由。確率の性質は `probabilistic_property` で、一本の経路は出ない。
explain は「一つの例で原因ではない」「最短性・現実妥当性の証明ではない」と assumptions に書いている。

explain は `--configuration NAME` と `--scenario NAME` で check と同じ対象を選べる。
必要な構成の指定が無い場合は check と同じく invalid(exit 3)。選択した構成で反例を再生する。

## simulate

`simulation.status / replications / completed_replications / seed / confidence / nondeterminism`。
`nondeterminism: reject-on-encounter` で、未解決の非決定に出会うと U-SEM-0 で unsupported。
basis は性質や中断の有無で異なり、確率性質の案件別推定は `basis: statistical` で表す。**全スケジュールの安全性検証ではない。**

## compare

`plans[]` に案ごとの完全な report(`configuration` と `report`)、`tables.properties / terminals / metrics` に横並び、`mismatch` に比較条件の不一致。
`assumptions` は `compare.purpose = side-by-side-only`(順位・総合点・Pareto は出さない)。`unchecked` に ranking 等が並ぶ。
exhaustive の invoice 3 案は全体 violated(ToBeA が violated)で exit 1、simulation は 3 案とも unsupported で exit 4 だった。
再送を含む案があると、`plans[]` は入るが `checked` は空、`unchecked` に `configuration_selection`・`property_tables` 等が並び、最上位は unsupported・exit 4(EC 試行)。読み方は上の「全体の verdict」の例外。

## diff

`differences[]` に計算した構成差分、`diagnostics` の W-DIFF-1 は「計算にあって intent に無い差分」。
`terminals / properties` に from/to の対。AsIs→ToBeA は exit 1。
再送を含むと最上位は unsupported・exit 4、`from.report`・`to.report` はそれぞれの verdict を持つ(EC 試行で両方 violated)。

## mutate

`original_verdict`、`run{completion, reason, mutant_limit, candidates_total}`、`mutations[]` に種類ごと `status`(generated / not_applicable / unsupported / blocked / not_selected)、`candidates` と `mutants[]`(`result`: detected / undetected / invalid_mutant …)。
invoice では元が satisfied でも生成 run 全体は unsupported(exit 4)。**検出数を検出率・網羅率・モデルの強さと呼ばない。**未完了の変異体は数えない。

`relca mutate <path> [--set bpr-default] [--configuration NAME] [--scenario NAME] [--mutant-limit N] [--kinds k1,k2] [--sites ID1,ID2]`。
構成の選択は check と同じ。元の検査と変異体に同じ構成を使い、共通模型へフォールバックしない。
実行上限は既定128、指定は1〜4096の整数。0、負数、非整数、空値、上限超過は invalid(exit 3)。
受理した計算上限は `ledger.execution.limits.mutant_limit` にも入る。切断後は `not_run` / `mutant_limit`、
run は `inconclusive` / `mutant_limit`。選択した未対応種があれば最上位は unsupported のまま。
`--kinds` は出力の kind 名をカンマ区切りで選ぶ。未知名、空リスト、空要素、重複、余白は invalid。
省略なら全種。選外は `not_selected`、`mutants: []`、`unchecked` に `mutation.kind.<kind>`。
検出・未検出・成功の数に混ぜず、verdict と completion は選択範囲のみで決める。
宣言した除外は計算の切断ではないため、それだけで inconclusive にしない。
`candidates` は選外と上限後も含む候補数で、run の合計も選外を含む。列挙できない種類は null、
該当なしは0。列挙自体を始められなければ合計も null。必要予算は選択行の候補数を足して読む。
詳しい種類名と例は[利用ガイド](../../../docs/guide/results.md)を参照する。
`--sites ID1,ID2` では既存の `mutants[].id` をコピーして場所を選べる。
種類選択との積集合だけを実行し、未知ID・空指定・空要素・重複は invalid。
選外の場所は mutant の `result` / `reason: not_selected`、report は null、
`unchecked` に `mutation.site.<id>`。予算・検出・未検出・成功に数えず、切断にもしない。
候補数は選外も含む。ソースの編集でIDが変わり得る。
全操作0分で、切断なくすべて絶対時刻0に完了する模型の授権失効は
not_applicable / no_time_elapse (候補0)。遅い到着や待ちで時刻が進めば除外しない。
全応答のbackoff=0も直接検査する。正のWaitとWaitUntilも静的に検査し、到達しない枝やEnd後も含む。
カレンダーも別の時間源なので、resource_timeがある場合は実時刻0でもこの除外をしない。
実行した変異体0件でfoldがsatisfiedの場合だけvacuous/exit5、reason=no_selected_mutant。
未対応種があればunsupported/exit4の優先順を維持する。
構成の数を問わず、明示選択した構成の候補とaffected_bindingsは実効process内に絞る。
sitesで1箇所でも外した種類の段と全体の生成/比較はcheckedに載せずuncheckedに残す。
外したkindsがある場合も全体の生成/比較をcheckedにせず、比較0件の段もuncheckedに残す。
無指定の従来の列挙は保持する。不正な上限やsite IDのmutant_limitは
run/ledgerともnot_applicable/value:null/reason:no_analysisで、128とは記録しない。

## verify

外部バックエンドは未接続で、exit は 4。stdout の verdict は unsupported で、`ledger` と `uncertainty` も付く。
stderr は通常 `relca verify: not implemented; outside Phase 1; external backend is not connected`。ただし **priority を使うモデルを渡すと** `U-SEM-0: priority_feature_combination: verify backend is not connected to priority disciplines` になる(`crates/relca-cli/src/main.rs` と `crates/relca-cli/tests/snapshots/inputs/q5-start.relca` を参照)。

## sensitivity と conform

`crates/relca-cli/src/main.rs` には `sensitivity`(`relca sensitivity <manifest> --mode exhaustive|simulation`)と `conform`(`--log --scope [--configuration] [--scenario] [--approval-spec]`)もある。
usage はこれらを含む9コマンドを列挙する。この補助資料の試行では回していない [U]。
[利用ガイド](../../../docs/guide/results.md) と実装・試験の入力を確認してから使う。
