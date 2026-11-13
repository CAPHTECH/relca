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
Cut の有無は分からないので W-CUT-0 は出さない。読んだ模型の Cut 未宣言とは区別する。
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

EC 試行(再送を含む 2 案)では、compare も diff も最上位 unsupported・exit 4、各案の report はどちらも violated だった(1 回)。各案の性質の verdict は `check --configuration` の結果と一致した。
この拒否では単純な畳み込みだけで最上位を再計算できない。読むときは次のとおりにする。

- 最上位の unsupported は「比較・差分をしていない」という意味で読む。
- 案ごとの結論は `plans[].report.verdict`(diff は `from.report.verdict`・`to.report.verdict`)から読み、**両方を報告する**。案の violated を最上位の unsupported で打ち消さない。
- exit は最上位の verdict に対応する(上の例は 4)。exit だけ見て案の違反を見落とさない。

review_page は、check・explain・simulate・各案・mutate の original を `fold_verdict` と同じ規則で畳み直す。compare の最上位は各案の verdict の最も重いものと照合する(`scripts/review_page.py` の `scan` の `kind == "compare"` の分岐)。どちらも全体の verdict が軽ければ「全体の verdict を信用しない」と警告する。上の拒否では、この警告が出るのが正しい。
review_page の端末・ページ冒頭・結論の未確認件数は、性質や入力の保留に加えて、
Agency の `not_derived` と根拠不明も同じ集計で数える。出所と場所を含む項目単位で重複を除く。
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

確率の `min / max` は完全情報の緩和境界で、実現できる方策の値ではない(assumptions の `probability.scheduler`)。

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
