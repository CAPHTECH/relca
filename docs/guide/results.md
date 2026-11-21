# 結果の読み方

CLI は標準出力に JSON を出し、診断は標準エラーにも書きます。

診断の `span` はモデルの UTF-8 バイト区間です。`location` では同じ区間を
`line`、`column`、`end_line`、`end_column` で読めます。行と列は1から数え、列は
バイト数ではなく Unicode の文字数で数えます。終点は区間に含みません。位置が不明なときは `null` です。

`detail` がある診断は、判定を妨げたものを `blockers` に列挙します。たとえば根拠源の契約では、
候補の源、対象の呼出し、根拠として数えなかった書き手を確認できます。
`kind` が `rewrite` なら、示された機能を使ったまま、意味を保つ局所の書き方に直せます。
`analysis_not_connected` は、この機能の組合せのままでは判定できないという意味です。
機能を別の形に置き換えたモデルまで拒否する、という意味ではありません。
`property_undecided` は、モデルは受理したがその性質だけを判定できない場合です。
`see` は利用ガイドの該当節です。
詳細がない診断の `detail` は `null` です。標準エラーの従来行は変わらず、
詳細があるときだけ直後に補足を出します。
機械的な成否は最上位 verdict に対応する exit で返します。

| verdict | exit | 意味 |
|---|---:|---|
| `satisfied` | 0 | 対象の性質が、今回のモデルと分析条件で成立 |
| `violated` | 1 | 少なくとも一つの検査で違反 |
| `inconclusive` | 2 | 切断などにより結論を出せない |
| `invalid` | 3 | 入力の不整合や静的エラー |
| `unsupported` | 4 | 必要な機能や組合せが対応範囲外 |
| `vacuous` | 5 | 性質の前件が発生せず、検査として働いていない |

## check、simulate、explain

最初に `mode` と `verdict` を確認し、次に以下を読みます。
explain は check と同じ最上位の欄に `explanation` を加えます。反例はその欄にあります。
explain と mutate も `--configuration NAME` と `--scenario NAME` で check と同じ対象を選べます。
構成の指定が必要なモデルで省略した場合や、未知の構成名は invalid です。
mutate の元の検査と変異体、explain の反例と再生は同じ選択を使います。
引数の誤りなどで模型を読む前に止まった report は、hash が null、Cut は `unresolved` です。
この場合は Cut 未宣言とは判断できないため、W-CUT-0 を出しません。
また、模型を読めても構文解析で止まった report（`E-SYN-0`、`U-SYN-0` など）は、
Cut の有無を判断できないため `cut: "unresolved"` とし、W-CUT-0 を出しません。
この場合は読んだ入力の hash が残ります。ソースに Cut が書いてあっても同じ扱いです。
構文解析が通って Cut が無いと分かった場合は、その後の型検査や lower で止まっても、
従来どおり W-CUT-0 を出します。構文解析後に Cut 宣言の解決に失敗した場合も
`unresolved` なので、Cut の欄だけで停止した段を判断せず、診断を併読してください。
この区別は compare・diff の各案、mutate の original、sensitivity の各点、conform にも適用します。
ただし conform は既存の例外として、parse 成功後に lower が拒否した場合も `cut: "unresolved"` のままです。
その入力に Cut が無ければ W-CUT-0 を同時に出します。`unresolved` だけで parse 失敗や警告無しとは判断できません。
また、`E-SYN-0` / `U-SYN-0` は lower から出る場合もあるため、診断 code だけから停止した段を推測しないでください。
欄は宣言や分析モードによって省略されることがあります。欄の省略だけから、対応済み・検査済みとは判断しないでください。

| 欄 | 確認する内容 |
|---|---|
| `model.sha256` | 分析した入力の識別子 |
| `cut`, `cut_context` | 宣言した範囲と、除外・保留・未宣言事項 |
| `cut.defer_effects` | defer の解決先と、指標を撤回した範囲・原因。対象の指標がある report だけに出る |
| `bounds` | 案件数、同時 Task 数、clock、`truncated` |
| `properties[]` | 各性質の `verdict`、確率境界、根拠や注記 |
| `contract_scope[]` | 選んだ構成では範囲外にした契約、その適用先と理由。検査済みの性質ではない |
| `terminals[]` | 到達先別の時間・work の範囲。平均とは限らない |
| `diagnostics[]` | `code`、`severity`、`message`、入力のバイト区間 `span`、行・列 `location`、判定の補足 `detail` |
| `assumptions[]` | 既定値や分析で置いた前提 |
| `actors[]` | 宣言値と省略時に補った欄 |
| `metrics`, `quality` | 要求した量と、算出できない理由 |
| `repetition` | 繰り返しの回数、上限後の処理への到達、元の呼出し位置との対応 |
| `ledger`, `uncertainty` | 入力選択、結論の根拠、未検査事項の記録 |
| `unchecked` | 今回は検査していない項目 |

最上位が violated でも、別の性質は vacuous や inconclusive のままの場合があります。
null は 0 ではありません。`not_configured`、`undetermined`、`truncated` などの理由を
値と一緒に扱ってください。`not_configured` は単価などの未設定だけでなく、defer による値の撤回でも出ます。
`per_case_relaxation` の区間を一つの方策で実現可能な区間と読まないでください。

simulate の `basis` は性質や中断の有無で異なり、`properties[].cases[]` の `basis: statistical` は網羅探索の確率ではなく標本推定を表します。

[2件が競合する例](../../fixtures/mean-repeat-policy/)では、待ち始めた時刻が同じなので案件 ID が順を決めます。先に待ち始めた案件があれば、その案件が先です。`then call_order` の字句順まで使うのは、同じ案件の `fork` の枝が同時に待ちに入る場合など、最初の二つの鍵で決まらないときです。`repeat` の各回には書き下した版と同じ順を付けます。

### 同時刻の順序を変えても結果が同じとき

`simulate` は、同じ時刻に残った行動の順序を試します。その時刻を閉じた状態と途中で得た観測がすべて一致した場合だけ、先へ進みます。該当する実行には `assumptions` に `simulation.same_time` が出ます。状態や観測が異なる場合や、4096 状態を超えた場合は `unsupported` です。別の標本経路や全スケジュールの安全性は、この検査では確かめません。

### 異議を起票しない場合の見積もり

`scenario.contest_not_filed` が出た結果の時間・待ち・作業・稼働率・処理量は、その異議が起票されない場合の値です。W-CONTEST-1 と `cut_context.reentry` に対象が出ます。`contest_response` の `excluded: scenario_contest_not_filed` は応答の期限を検証していない印です。判定は `inconclusive` です。案を比べるときは、異議の対象となる終端の件数も見てください。差戻しが多い案ほど、見積もりに含めなかった負荷は大きくなります。

### 業務指標で値が出ない理由

`metrics` の `reason` で、測る対象がなかった場合と、結果を確かめられなかった場合を区別します。どちらも、値が `null` なら 0 として集計しません。
集計の中断や標本不足については、[品質指標で値が出ない理由](#品質指標で値が出ない理由)も参照してください。

| `reason` | 読み方 |
|---|---|
| `no_terminal` | この終端での完了が観測されていないため、完了案件の経過時間や費用を出せません。他の終端での完了や、観測終了時の未完了とは区別してください。 |
| `rejected` | 受付拒否の区分では完了時間を、受付を拒否された案件では開始までの待ち時間を算出しません。終端表の受付拒否の行がこの理由でも、その案件が実際に拒否されたかは件数で確認します。 |
| `never_created` | 操作が作られなかったため、その操作の待ち時間などは対象外です。容量超過で操作を作らなかった案件の費用も、0 円の確定値にはしません。 |
| `cancelled_before_start` | 開始前に取り消されたため、開始までの待ち時間は算出しません。取消件数は別に読みます。 |
| `no_task_started` | 案件は終端に着きましたが、開始した操作がないため、最初の開始までの待ち時間は出ません。 |
| `not_refused` | 操作の開始が拒否されたときの待ち時間を集計する欄で、拒否が発生していません。 |
| `no_capacity` | 対象に容量の設定がないため、稼働率の分母がありません。 |
| `zero_capacity` | 稼働率の分母が 0 です。稼働率が 0 という意味ではありません。 |
| `join_any` | 先着合流では、全枝の完了を待つ時間を定義しません。採用後に残った枝の処理を合流待ちに数えません。 |
| `refused` | 操作の開始が拒否されたため、通常の開始までの待ち時間を出せません。拒否の件数や、拒否までの待ち時間は別の欄で確認します。 |
| `unfinished` | 観測終了までに操作が開始しなかったなど、値を確定する出来事がまだ起きていません。未完了を成功や失敗に振り分けません。 |
| `deadlock` | 行き詰まりで操作が開始できなかったなど、待ちの終点を観測できていません。行き詰まりの件数と待ち時間を区別してください。 |
| `range_stopped` | 許された値域の外へ書き込む枝を止めたため、その先を含む指標は算出できません。計算上限による切断とは別の理由です。 |

### 時間・作業・費用の期待値

mean を宣言したモデルだけ、`quality.quantiles` に続く位置に `quality.means[]` が出ます。
`measure` は量、`property` は到達する性質、`form` は全流入の `all_arrivals` または到達条件付きの `reached_only` です。
網羅探索の `value.lower`・`upper` は確率で重み付けた期待値の区間で、時間は分です。

`work` は占有時間で待ちを含まず、`named_people_work` は名前付きの実行者の占有だけです。匿名作業を人の作業と推定しません。
`cost` は宣言した通貨での占有費と開始費の和です。料金時間外の単価を宣言したモデルでは、その区分も既存の費用指標と同じように使います。
性質を省いた行は `property: null` で、到達・受付拒否・deadlockなど案件が閉じた時点までの量を、全流入数で割ります。
`repeat` では、実行された回ごとの作業と費用を数えます。上限で `on_limit` に進んだ案件も全流入に含まれます。経過時間の性質に通常の終端と引継ぎなどの終端を両方書くと、どちらの経路の経過も平均に入ります。[手計算例](../../fixtures/mean-repeat/)では、回の間の3分の待ちを含む平均経過は `9/2` 分、占有作業は `3` 分です。
計算の切断やhorizonで未完了の案件は閉じたことにせず、理由つきnullと下界に分けます。
例えば [mean-chance](../../fixtures/mean-chance/) の全終端への経過は `15/4` 分、done 到達条件付きでは `3` 分です。
非決定の選択には確率を補わず、最小・最大を取ります。端が同じなら `exact`、異なれば `per_case_relaxation` です。
2案件が資源を取り合う [mean-contention](../../fixtures/mean-contention/) では `[3,6]` 分となりますが、
実際の平均はどの順序でも `9/2` 分です。`quality.mean` の仮定にある通り、案件ごとの区間の和は同時の範囲より広くなりえます。

到達質量が足りなければ `beyond_reached_mass`、分母が0になりうるなら `denominator_may_be_zero`、
切断では `truncated`、horizon の未確定では `undetermined` となり、値は null です。
`lower_bound` が付く場合も、値とは区別してください。`given reached` の未確定には下界を出しません。
単価の必要集合は選んだprocessの `metrics` と同じです。到達条件付きの費用でも、必要な単価を0円に補いません。
`metrics` の観測設定が無い作業・費用も `not_configured` です。経過時間はこの設定無しでも計算します。
影響不明の process 全体の defer では mean の値と下界を撤回し、`not_configured` にします。
`simulate` の `value.point` は、全反復の案件をまとめた標本平均です。`given reached` では到達した案件を合計し、反復ごとの平均を先に取りません。`observations` は量を観測できた案件数です。計算上の切断や horizon で止めた反復が一つでもあれば、点推定は `null` です。`censored` はその反復に含まれ、集計から除いた案件数です。標本には下界や信頼区間を出しません。
Rule を含むモデルでも、mean は未接続として `aborted` と `unchecked` に残します。既存のcohort・分位点まで撤回する指定ではありません。
`terminals[].elapsed` や `metrics` の経路範囲、`metrics.unchecked` の `exact_expectations` は従来どおりです。

### 品質指標で値が出ない理由

標本がない場合と、観測した結果から比を定義できない場合では、次に確認する箇所が異なります。
平均の理由は[時間・作業・費用の期待値](#時間作業費用の期待値)も参照してください。

| `reason` | 読み方 |
|---|---|
| `aborted` | 未解決の非決定で解析を中断した場合など、指標の算出を取りやめた印です。中断までに観測した分だけで結果を補わず、診断と `unchecked` を確認してください。 |
| `no_samples` | その量の集計に使える標本がありません。例えば、指定した終端への到達時間を測れる標本がなければ、平均は出ません。 |
| `zero_denominator` | 分母が 0 と分かっているため、比や到達条件付きの値を定義できません。分母が 0 になりうる場合とは区別し、値を 0 に置き換えません。 |

### 指標の条件を満たしたか読む

`verify metric` を書くと、`metric_checks` に条件ごとの判定が出ます。
区間全体が条件を満たせば `satisfied`、全体が破れば `violated`、両方を含めば `inconclusive` です。
下界だけの場合、`interval.upper` は `null` です。例えば条件が「30分以下」なら、下界が30分を超えれば違反です。下界が30分以下でも、上界がなければ合格とは言えません。
値を計算できなかった理由は `reason` で確認できます。

`check`・`explain`・`simulate` では、この判定も全体の `verdict` と exit に反映されます。下界で判定した行の `basis` は `lower_bound` です。
`simulate` では cohort を信頼区間で判定します。確率の問いと cohort の成分の案件別区間を一緒に数え、片側に割り当てる誤り率を `(1 − confidence) / (2 × rows)` にします。分母も案件の性質なら分子と分母の両方を数えます。実効値は `simulation.multiplicity.alpha_after` と、区間を使った `metric_checks[].alpha` に出ます。複数案を比較しても、この割当は案内で閉じます。案をまたぐ補正は `hard` のための別枠です。
mean には合否を決める信頼区間がないため、標本の判定は `unsupported`（`no_statistical_bound`）です。`quality.means[].value` の標本行には `point`、`observations`、`censored`、`reason` が出ます。

`diff` と `sensitivity` では、構成や点ごとの指標条件は子レポートの `verdict` に反映されます。最上位の `unchecked` にある `metric_constraints` は、子レポート間で条件の違いや変化を別途検査していない、という意味です。`mutate` の同じ印は、条件を変異の検出に使わないことを示します。

`compare` の hard でも同じ区間の規則で判定します。一方、目的の値に `lower_bound` は使いません。
同じ性質を参照する mean と分位点、mean と同じ性質を分子または分母に使う cohort、`mean(cost)` と `metrics.overall.observed_cost_per_arrival` を一緒に指定すると、`compare.pareto` の仮定欄に共有する観測として表示されます。目的を合算する指定ではありません。

## defer による指標の撤回

Cut の `defer` は、手順を実行対象から消す指定ではありません。
保留した範囲に依存する時間・work・費用などの量と、品質・cohort・quantile の数値を撤回します。
撤回した数値は null になり、ほかの理由がまだ無い値には `not_configured` が付きます。
すでに `aborted` や `truncated` などの理由がある場合、その理由は保ちます。
件数、開始回数、品質の試行数などの構造的な記録は残ることがあり、それを撤回された数値の代わりに使ってはいけません。

解決は名前の完全一致です。Actor 名ならその Actor の全 Capability、`Actor.capability` ならその一つが対象です。
それ以外の名前は `opaque` となり、指標を process 全体で撤回します。
モデルにできない段をその段の名前で defer した場合、この全体撤回は意図した扱いです。
その段を含まない値を使わないため、W-CUT-2 と撤回を報告に残してください。
宣言済み Actor や操作の defer は、その層を実際にモデルに持つ場合に使います。
無関係な操作を代わりに指定して、欠けた段があるまま値を出させてはいけません。
正しく解決した対象でも、選んだ process の本文で呼ぶか、起動しうる異議経路の route なら、撤回は全体へ広がります。
本文の呼出しは、到達しない枝も含めて字句上で数えます。
交差しない対象は、対応する Actor・Capability・実行者・資源の行だけを撤回します。
この説明のために、新しい指標の行を作ることはありません。

`cut.defer_effects` は、defer があり、metrics または quality を持つ対象 report に付きます。
この対象では `cut.checked` に `deferred_metrics` が入ります。
`entries[i]` は宣言の `cut.defer[i]` と同じ行であり、同名の defer が複数あっても統合しません。

| 欄 | 読み方 |
|---|---|
| `process` | どれかの行が全体を撤回したか。true なら、ほかの行が scoped でも全体の数値は戻らない |
| `entries[].target` | 宣言した対象の綴り |
| `resolution` | `actor` / `capability` / `opaque` |
| `capabilities` | その行から解決した操作名。名前順 |
| `withdraws` | `process` は全体、`scoped` は scope の行、`none` は対象操作が無い Actor |
| `because` | 全体へ広がった原因。`opaque`、本文の `call`、異議経路の `contest_route`。後二つは操作名付き |
| `scope` | scoped のときの `actors` / `capabilities` / `people` / `resources`。モデル上の名前であり、実際に指標の行があるとは限らない。それ以外は null |

例えば本文で呼ばない `R.y` を保留し、その操作の実行者が q、使用資源が Desk なら、次の形です。

```json
{"process":false,"entries":[{"target":"R.y","resolution":"capability","capabilities":["R.y"],"withdraws":"scoped","because":[],"scope":{"actors":["R"],"capabilities":["R.y"],"people":["q"],"resources":["Desk"]}}]}
```

値側に原因の欄は追加しません。`not_configured` を見たら、まずこの表の process と scope を読み、
費用なら `metrics.rates` の単価、`reason`、`required` も併読してください。
defer によって撤回した品質の hard や目的を、残っている試行数から再計算して採否・順位を決めることはできません。

**W-CUT-2** は、Actor にも Actor.capability にも解決しない defer ごとの警告です。
message に対象名を出し、先頭が宣言済み Actor なら存在しない Capability 名も示します。
警告自体は verdict と exit を変えませんが、数値を使う前に綴りと保留する範囲を確認してください。
正しく解決した対象には、全体を撤回する場合でも W-CUT-2 は出ません。その場合の原因は `because` を読みます。

この欄と警告は check・explain・simulate、compare の各案、diff の両側、mutate の元と変異体、
sensitivity の各点に同じ規則で付きます。defer が無い場合、metrics も quality も無い場合、
入力検査で拒否された場合など、指標を持たない report には出ません。conform も対象外です。

## 契約の適用範囲

`applies_to` で選んだ構成を範囲外にした契約は、`properties` には入りません。
節の id は範囲内の構成と同じで、他の節の通し番号も変わりません。
[構文の全文例](../../examples/configuration-contracts.relca)の Old では次の行が出ます。

```json
{"id":"contract Controls #1 authority_basis","contract":"Controls","applies_to":["New"],"reason":"Oldには承認画面がない","status":"not_applicable","masks_call":false}
```

この行は `contract_scope[]` の要素です。
`unchecked` にも `contract.scope.contract Controls #1 authority_basis` が残ります。
**not_applicable は verdict ではありません。** satisfied として数えず、全体の判定や exit を決めるときも数えません。
適用される要求が0件になれば、範囲外の宣言を要求に数えず unsupported / exit 4 で止まります。

| 警告 | 読み方 |
|---|---|
| W-CONF-1 | 複数の構成があるモデルで、適用中の契約が vacuous であり、選んだ process に対象の字句上の呼出しも無い。統制が設計上無いなら applies_to の指定を検討する。verdict は vacuous のまま |
| W-CONF-2 | 範囲外の契約の対象が、選んだ構成の手順や異議の経路から呼ばれうる。`masks_call: true`。到達しない枝、応答喪失からの復旧、再送、縮退の代替も数える。Capability を参照しない節も true。違反が起きないという意味ではない |
| W-CMP-1 | 比較した案で契約の適用範囲が異なる。同じ統制を問う比較として扱わない |

compare の `tables.properties` は契約の行を落とさず、範囲外のセルを次の形にします。

```json
{"configuration":"Old","status":"not_applicable","verdict":null,"reason":"configuration_scope"}
```

適用側の status は `applied` です。applies_to を使うモデルでは、共通の verify のセルにも `applied` が付きます。
使わないモデルには新しい status 欄を出しません。
非対称な比較では `checked` に `contract_scope`、仮定に `compare.contract_scope: asymmetric` が付きます。
hard と目的の宣言評価は、各案に適用される要求で採否を決めます。仮定の行で対象の契約と案を確認してください。
最初の案で非適用の契約は、性質表の末尾に加わることがあります。行の位置ではなく id で対応させてください。

diff の性質行は、適用範囲を使うモデルだけ `from_status` / `to_status` を持ちます。
範囲外の側の from / to は null です。`differences` の `kind: contract_scope` は、
統制が加わる向きを `added`、外れる向きを `removed` と示します。
差分に対応する intent が無ければ W-DIFF-1、保持・上書き・保守・loses の注記も無ければ W-DIFF-3 が出ます。
たとえば行き先の intent に `retire contract_scope Controls reason "承認を使わない" loses "承認の根拠を求める契約"` と書けば、
Controls を外す差分へ理由と失うものを添えられます。追加は `add contract_scope Controls reason "承認を追加する"`、
変更の説明は `change contract_scope Controls reason "適用先を見直す"` の形です。
行き先で preserve した契約が範囲外なら E-DIFF-0 / invalid / exit 3 です。
非適用を preservation の satisfied に混ぜません。

explain は `explanation.omitted` に `verdict: null`、
`reason: not_applicable_configuration_scope` の行を残します。
mutate は範囲外の authority_basis から期限切れ候補を作りません。
mutate の最上位の `contract_scope` は、`original.contract_scope` と同じ内容です。
最上位の `unchecked` にも同じ契約の未検査を残します。変異体ごとに検査した結果ではありません。
全文例の Old は元の check が satisfied でも、authority_expired だけを選ぶと候補0件で vacuous / exit 5 です。
sensitivity は適用先の宣言も同じ問いの検査に含め、点ごとに違えば `sensitivity_contract_mismatch` で拒否します。
conform も適用範囲外の契約は照合しません。いずれも、その範囲を検査済みとして報告しないでください。

確認用ページは source の適用先と理由、および各 report の範囲外の契約を表示します。
範囲外の契約は出所と id ごとに未確認の件数に数えます。verdict の再計算には混ぜません。
mutate の最上位と original は同じ元の検査の情報なので、確認用ページでは original の1件だけを数えます。

## 差分の覆い方

`diff` の `differences` は計算した差分です。理由が付いても、変更の安全性や現実の妥当性を
認定したことにはなりません。両側の性質の判定と、差分の注記を分けて読んでください。

相手ごとの `intent from A to B` を1つでも書いたモデルには、`intent_block` が出ます。

| 値 | 使った理由の宣言 |
|---|---|
| `{"from":"A","to":"B"}` | A→B にだけ書いた宣言 |
| `{"from":null,"to":"B"}` | B の構成内の既定の宣言 |
| `null` | この対の宣言も、行き先の既定も無い |

相手別の宣言が無いモデルでは、この欄自体を出しません。空の相手別宣言を選んだ場合は
その対を表示し、既定の理由は使いません。`preservation` も選んだ宣言の preserve だけから作ります。

`replace` によって説明できた行には、次の配列が付きます。

```json
{"covered_by":[{"intent":"replace","from":"Clerk.check","to":"Bot.check","via":"derived"}]}
```

`via: direct` は差し替える能力の削除・追加、`derived` は差し替えだけで説明できた関連の差分です。
行の `reason` と `loses` は null のまま、文は最上位の `replacements` に宣言ごとに1回だけ置きます。
`replacements[].covers` は、その宣言が説明した全行の `kind`・`target`・`change`・`via` の一覧です。
各行の `covered_by` と、各宣言の `covers` を対応させて、理由の出所を辿れます。
個別の intent が優先された行は個別の理由・注記を持ち、どちらの一覧にも入りません。

複数の差し替えはまとめて扱います。ある宣言を外すと説明しきれなくなる行には、その宣言を記録します。
そのような宣言が無ければ、単独で説明できる宣言をすべて記録します。
単独で説明できる宣言も無い場合は、その行をまとめて説明したすべての差し替えを記録します。
したがって同じ行に複数の宣言が付く場合があります。`covered_by` と `replacements` は差し替え元の名前順です。
同じ種類・対象の差分が値や向きを変えて残る場合は、説明済みにはしません。
`replace` を選んでいない diff には `covered_by` も `replacements` も出しません。

| 診断 | 読み方 |
|---|---|
| W-DIFF-1 | 計算した差分に対応する intent が無い。差し替えで説明しきれない行にも残る |
| W-DIFF-2 | retire（keep fallback を除く）または replace に、失われるものの記録が無い |
| W-DIFF-3 | 保持・上書き・保守・loses の注記が無い。replace の派生行は loses がある場合だけこの警告を消す |
| E-DIFF-0 | 宣言した変更が計算に当たらない、または保持したい契約が行き先では範囲外。invalid / exit 3 |

[全文例](../../examples/capability-replacement.relca)の A→B は10行を説明します。
能力の削除・追加の2行が direct、関連する8行が derived で、W-DIFF-1/2/3 は0件です。
同じ宣言から loses だけを外すと、W-DIFF-2 が1件、派生行の W-DIFF-3 が8件になります。
C→B は別の宣言を使うので、理由は「別製品へ移行」です。A→C には宣言が無く、10行の W-DIFF-1 が残ります。

compare は、比較する構成に intent の宣言があれば、空でも `unchecked` に `intent` を出します。
相手ごとの理由の照合はしていません。3案以上の理由を確かめるには、diff を必要な対ごとに回してください。
確認用ページは宣言と compare の未照合を表示しますが、diff の結果を代わりに照合するものではありません。

## 合流の出力

[値を返す if](dsl.md#分岐した値を先へ渡す) は、選んだ枝の値を複写します。
例の `check` は `done_fixed` が確率1/4・5分、`done_unchanged` が確率3/4・3分です。
期待時間は手計算で7/2分ですが、`metrics` の `min` / `max` は経路の範囲3〜5分で、平均ではありません。
Task の行は `d`、`r`、`f`、`s` の4行です。合流値 `ready` の行はなく、送付 `s` も枝ごとに増えません。

`relca explain examples/merge-if/model.relca` の期限違反の反例には、時刻4に
`kind: merged`、`binding: ready`、`source: f` の段が入ります。
`result` は複写した観測結果、`location.span` は `yield` 文のバイト位置です。
Task ではないので `actor`、`capability`、`executor` はいずれも null です。
独立した参照実行で選択元と値を再現できない反例は、再生を `unsupported` として扱います。

合流後の系譜は経路に依存するため、全枝の根拠を足し合わせて一つの判定の根拠にはしません。
合流値を直接・間接に引数として受ける判定の `lineage` 行は出さず、
`agency.unchecked` に `lineage(merge_control): 呼出し名` を残します。
合流に依存しない判定の系譜、原本への直接・間接到達、commit の実行者は保持します。
`origin` の間接到達は、Actor ごとに存在しうる読取り経路の集約であり、
すべての経路で同じ根拠を使うという主張でも、独立検証の数でもありません。
合流から独立した判定、拒否、行使回数、責務や警告の導出は保持します。
合流値やその値で選ぶ制御に依存する項目だけを `not_derived`、理由 `merge_control` とします。
終端や操作ごとの行には、導出済みと未導出の項目が混在することがあります。

| 未導出になる警告 | 対象 |
|---|---|
| `W-AGY-0` / `W-AGY-1` | 効果を止めない判定 / 迂回のある判定 |
| `W-AGY-2` | 判定・拒否・実行の集中 |
| `W-AGY-3` | 縮退のために保持する操作の通常経路での行使 |
| `W-RESP-0` | 責務の宣言が無い判断の担い手 |
| `W-RESP-1` / `W-RESP-2` | 宣言した責務に必要な観測 / 拒否 |
| `W-RESP-3` | 案件内の別の操作では直接読むが、責務の当該段では直接読まない根拠源 |
| `W-LIN-2` | 独立な検出力に必要な新しい根拠源 |

導出しなかった対象は、コードと `merge_control`、呼出し名・操作名・責務の対象などを
`agency.unchecked` に記録します。警告が無いことを、問題が解消した証拠と読まないでください。
責務の observation / refusal も依存する欄だけを `merge_control` とし、独立した欄の
`W-RESP-1` などは従来どおり導きます。
ただし、同じ判定が合流に依存する効果にも関わる場合、責務の refusal は欄全体が
`not_derived(merge_control)` になります。独立した commit への拒否が欠けていても、
その欄の `W-RESP-2` は警告ではなく未導出の印として残ります。

## Agency の導出の上限

| 警告 | 意味 |
|---|---|
| `W-AGY-4` | 主の Agency 導出が計算上限に達した。原本への到達・系譜・拒否と、責務の既定候補を導出しない |
| `W-AGY-5` | observation / framing / retention / effects のいずれかが計算上限に達した。対象の block ごとに1件出す |

上限に達した block は全体を `not_derived`、理由を `derivation_limit` とし、
途中までの行を導出済みとして出しません。他の block、性質の verdict、exit はこの警告だけでは変わりません。
警告は最終出力の理由が `derivation_limit` で、計算上限の記録がある観点だけに出します。
反復や応答の扱いによって `loop_control` / `response_control` へ置き換わった観点の途中の警告は残しません。
`limit` の `counter` は depth / paths / contexts / cells / work、`limit` は上限値、
`requested` は要求した量、`phase` は計算箇所です。process や judgement も、記録できた場合は併記します。
量が整数に収まらない場合は `requested: null`、`requested_overflow: true` になります。
不正な内部表現は `invalid_ir` として区別し、計算上限の記録や警告を捏造しません。

`check`、`simulate`、`explain` の `--agency-limit-scale N` は、主の導出、共通原因、
observation / framing / retention / effects の `work` と `cells` の上限をN倍にします。
既定は1、最大1000です。`cells` は累計確保量の計数で、最大常駐メモリの上限ではありません。
倍率を上げると時間やメモリが増える場合があります。
`depth`、`paths`、`contexts` と、縮退経路の導出の予算は変わりません。
たとえば `counter: contexts` で止まった計算は、この倍率では上限を増やせません。
反復を含むモデルで主の Agency が経路数の上限に達した場合は、経路上で値が決まる内部条件を分岐に数えず、経路の展開をやり直します。
利用者が `NAME.round` を条件に書いた分岐は Agency では両側を数えるため、実行時には通らない回の呼出しが系譜に載ることがあります。
合流を含むモデルでも、独立した部分の導出が計算上限に達した場合は `derivation_limit` と
`limit` を残し、W-AGY-5を出します。この場合は倍率で導出できるようになる可能性があります。
`merge_control` は合流値への依存など、構造上の理由で導出しない項目です。
`merge_control`、`state_guards` そのものを、倍率だけで対応済みにはできません。

実際の倍率は `ledger.execution.limits.agency_limit_scale` に常に出します。
解析した場合は `{"status":"known","value":20,"reason":null}` のように記録し、省略時は値1です。
引数の誤りなどで解析しなかった場合は `{"status":"not_applicable","value":null,"reason":"no_analysis"}` です。
明示した場合だけ、`assumptions` に `field: agency.limit`、`value: computational_x20` のような計算設定も残します。
これは業務モデルについて追加した仮定ではなく、`uncertainty.model` では既存の計算上限の参照で扱います。

framing の文脈は、Or などで多くの判定をまとめると急増する場合があります。
同じモデルでも導出の有無は倍率に依存するため、結果を比べるときは倍率もそろえてください。
倍率の指定は `compare`、`diff`、`mutate`、`sensitivity`、`conform`、`verify` では `invalid`、exit 3 になります。
性質の verdict と exit は、Agency の警告とは分けて読みます。

応答喪失を含むモデルの retention には別の照合があります。探索の切断、上限、
二つの探索器の不一致でも `derivation_limit` となり、この場合は `limit` がありません。
`notice` にある「切断・上限・両探索器の不一致では行使回数を導かない」も読んでください。
確認用ページにも、結果に記録された倍率と viewpoint の上限を表示します。記録が無い旧出力から倍率1を推測しません。

## 根拠源の要求と証跡

`evidence_basis` の性質の id は `contract Basis #1 evidence_basis` のようになります。
`check` の性質行は他の契約と同じ `id`、`verdict`、必要な場合の `cases` で、根拠の証跡は加えません。
`satisfied` は、終了した対象の呼出しが要求したすべての源を含んだというモデル上の結果です。
正常終了が一度も無い契約の `vacuous` と、切断の `inconclusive` を合格として読まないでください。
`simulate` で違反が見つからなくても、全スケジュールの安全性を満たしたという結果にはしません。
値を返す if と併用する場合、対象の判断の引数や観測読取りへ合流値が直接・間接に流れ込む形は
`U-SEM-0`、理由 `merge_control` で拒否します。無関係な合流や条件だけの参照は、契約の根拠に加えず通常どおり判定します。

[上流の読取りを外した例](../../examples/evidence-basis-missing.relca)の explain は、
時刻2分に `Clerk.match_po` が正常に終了するまでの反例を出します。
その性質の `explanation.counterexamples[]` にある `evidence` は次の形です。

```json
{"binding":"p","actor":"Clerk","capability":"match_po","required":["Invoice","PurchaseOrder"],"sources":["PurchaseOrder"],"missing":["Invoice"],"excluded_edges":[]}
```

| 欄 | 読み方 |
|---|---|
| `binding`, `actor`, `capability` | 違反を確認した操作の正常終了。steps と同じ識別子・操作名 |
| `required` | 契約が要求した源 |
| `sources` | その呼出しの照合に数えた源。direct の場合は自身の reads だけ |
| `missing` | 要求した源のうち、その呼出しでは候補にもない源 |
| `conditional_sources` | 要求した源のうち、記録や観測変数などを経由して届く候補はあるが、到達を確かめられないもの。未要求の候補は含めない。確認用ページでは不足した源とは別の行に示し、反例が通った繰り返しの出口も記録があれば表示する |
| `excluded_edges` | 必ず先に正常終了して最後の値を書くとは認めず、数えなかった観測変数の辺。`p -> a` のような binding 間の表記 |

`required`、`sources`、`missing`、`conditional_sources`、`excluded_edges` は昇順の配列です。`excluded_edges` がある場合、そこを読んだはずだと補完しません。
繰り返しの中の反例では `binding` が違反した回の呼出しを指し、`repetition` に元の呼出し名と回が入ります。
繰り返しの結果を引数や観測変数を経由して使う後続操作では、`repetition_exits` に反例が通った出口が入ります。
`loop_name` は繰り返しの名前、`round` は回、`phase` は本体での `leave` なら `body`、上限の処理での `leave` なら `on_limit` です。
`leave` が複数ある繰り返しでは `leave_site` も出ます。本体と `on_limit` の `leave` を合わせて、ソースに書かれた順に1から数えます。
いずれかの出口の組で要求源が確かに欠ければ `violated`、そのような出口がなくても条件付きスナップショットで確かめられない出口があれば `unsupported` です。
反例は参照実行器で再生した経路であり、`evidence` はモデル上の系譜の証跡です。
実際の資料アクセスを記録したログではありません。確認用ページも、この区別を付けて表示します。

`mutate --kinds evidence_missing` はこの契約の行も割当てに含めます。
`original_access_removed` に割り当てるのは `direct` の行だけです。
[正の例](../../examples/evidence-basis.relca)では2つの読取り削除をどちらも検出しますが、
他のモデルの検出件数や確率・網羅率を表す値ではありません。
原検査の全体が `vacuous` なら、mutate は候補の実行へ進みません。
契約が悪化しても、同時に割当てた系譜の行が消えて照合できなくなった場合は、
`inconclusive / assigned_check_unavailable` のままです。契約の変化だけで `detected` に置き換えません。
確かな読取りを削除しても、記録や観測変数を通じて同じ源が届く候補が残ることがあります。到達を確かめられない場合、その変異は `detected` ではなく `inconclusive / runtime_unsupported` です。

`compare` と `diff` は、構成ごとの契約の判定を性質の行に載せます。呼出しや経路の対応は取らず、
契約が適用される構成があれば `unchecked` に `contract_witnesses` を示します。根拠の証跡は各構成の `explain` で確認してください。
`sensitivity` では、要求する根拠源を変えた点は `sensitivity_contract_mismatch` になります。

登録を使う構成の `diff` では、記録 Actor ごとの登録者 `registrars` と読取り手 `readers` を `record` 行に示します。
Agency の origin の `via_records` は、記録を経由して届く根拠の由来に変化があったときの差分です。
異議経路を対象にした契約は `evidence_basis_contest_route` で未対応にします。
通常の呼出しとして追跡できない対象を `vacuous` として隠さず、他の性質の結果は残します。
`conform` は `U-LOG-1` で拒否し、`not_checked` に契約を `unsupported_request` として、
`unchecked` に `evidence_basis` を残します。読取りを観測しないログから満たしたとは判定しません。
契約が範囲外なら、未対応要求ではなく `contract_scope` に残します。

## 版の要求と証跡

`same_version` は `contract V #1 same_version` のような性質の行になります。
番号は契約名ごとでなく、模型全体の契約の項の順です。
check の行は `id`・`verdict`・必要時の `cases` だけで、版の証跡を全出力へ足すものではありません。
後の操作が一度も正常終了しない `vacuous` と、切断した `inconclusive` を合格と読まないでください。
simulate で違反が見つからない場合も、全スケジュールで満たすという保証ではありません。

[版を取り違えた例](../../examples/same-version-reused.relca)の explain は、時刻4分の承認終了までを再生します。
その反例の `explanation.counterexamples[].version` は次の形です。

```json
{"binding":"p","capability":"Approver.approve","parameter":"d","version":"d_dup","examined":{"binding":"k","capability":"Checker.check","parameter":"d","version":"d"}}
```

| 欄 | 意味 |
|---|---|
| `binding`, `capability`, `parameter` | 使う側の呼出し・完全修飾した操作名・引数名 |
| `version` | その引数の結果を作った呼出し名。値が等しくても別の呼出しなら別の版 |
| `examined` | 使う側の開始前に最後に正常終了した確認。同じ4欄で示す。確認が無ければ null |

合流を通っても `version` は合流名ではなく、選ばれた元の呼出し名です。
この証跡はモデルの経路の照合であり、実物の文書内容や運用上の版管理を認定したものではありません。
系譜・責務・`via_model_paths` の出力を、契約の成功で補完しません。
確認用ページにも、使う版と確認した版を分けて表示します。

「版」には次の三つの別の意味があります。

| 書き方 | 何を区別するか | 今回の値の同一性との関係 |
|---|---|---|
| `version_check [x] else rejected` | 操作の実行中に、状態 x が他から書かれたかを見る書込みの版 | 引数の値の版は比べない |
| `via model "draft@v1"` | 出力を生成する経由モデルとその版の宣言 | 同じモデル版の別の呼出しでも、値の版は別になる |
| `same_version A.check(d), B.use(d)` | どの呼出しの実行が引数の結果を作ったか | enum の等しさや内容の新旧ではなく、結果の同一性を比べる |

`compare` と `diff` は各構成の判定を載せますが、版の証跡は比較しません。`unchecked` の `contract_witnesses` でこの範囲を示します。
`sensitivity` では、契約の操作・引数の対が異なる点を `sensitivity_contract_mismatch` で拒否します。
conform は `U-LOG-1` とし、`not_checked` に契約を `unsupported_request`、`unchecked` に `same_version` を残します。
範囲外なら、他の契約と同じく `contract_scope` へ残します。

## 繰り返しの出力

`explain` の手順にある `values.observed` の `@loop:名前:active` は内部状態です。`1` はまだ抜けていない状態、`0` は番号を付けずに抜けた状態です。別名を受ける操作を対象にした `evidence_basis` がある場合に限り、複数ある `leave` に番号を付けます。そのときは `2` 以上の値から1を引いた番号の `leave` で抜けています。

`repeat` を含むモデルには `repetition` が付きます。
[構文と2つの例](dsl.md#繰り返し)と合わせて読んでください。
回数は body を開始した回数で、初回を含みます。
`on_limit` は最後の回の `again` でだけ到達する処理であり、計算の切断ではありません。

| 欄 | 読み方 |
|---|---|
| `repetition.rules`, `basis` | 回数表の規則と算出根拠 |
| `repetition.coverage` | `complete` は回数の解析が完了、`sampled` は完了した標本での集計、`not_derived` は算出できなかった状態。業務の成功とは別 |
| `repetition.sites[]` | 呼出しの実行単位と元の位置。`origin` が LoopSite、`source_span` が元の呼出しのバイト位置。識別子から回数を推測せず、対応欄を使う |
| `repetition.cases[]` | `case_id` と `loop_name` ごとの回数表 |
| `cases[].rounds` | 開始した body の回数の `min` / `max`。到達しない繰り返しは0回になりうる |
| `cases[].distribution[]` | body の開始回数が `rounds` になる場合の分布。`probability.min/max` は宣言された確率による境界。確率未宣言なら null で、一様分布ではない |
| `cases[].on_limit` | その案件で上限後の処理に到達する件数の `min` / `max`。1つの繰り返しにつき0または1件 |
| `cases[].on_limit_probability` | 上限後の処理に進む確率の境界。確率を算出しない場合は null |
| `cases[].samples`, `excluded_samples` | 標本集計に含めた件数と除外した件数。切断や未完了を完了標本に混ぜない |
| `cases[].distribution[].observed_count`, `observed_on_limit` | simulate での回数別の観測件数と上限後の処理への観測到達数。網羅探索の確率ではない |
| `repetition.unchecked` | 回数表を算出できなかった理由など |

LoopSite は `loop_name`、`call_site`、`round`、`phase` を持ちます。
`phase: body` の `round` は1始まりの回、`phase: on_limit` の `round` は null です。
上限後の処理を最後の body の一部として数えないための区別です。
この対応は `sites[].origin` に加え、metrics を要求した場合の
`metrics.cases[].tasks[].repetition` と、explain の反例内の
`steps[].events[].repetition` にも付きます。
反例では、先行する分岐でその回を抜けて後続の `again` や `leave` を飛ばした位置に、`branch` のステップが出ることがあります。

差し戻しの例では、`repetition.cases[0].rounds` は `{"min":1,"max":3}`、
`on_limit` は `{"min":0,"max":1}`、`on_limit_probability` は `{"min":"1/8","max":"1/8"}` です。
回数1、2、3の確率はそれぞれ1/2、1/4、1/4です。3回には、3回目で通る場合と
上限後の処理へ進む場合の両方が入ります。
`escalate()` の元位置は `call_site: e`、`phase: on_limit`、`round: null` で読めます。
再確認の例は `rounds: {"min":2,"max":2}`、`on_limit: {"min":0,"max":0}` です。

clock、同時 Task 数、探索予算などで計算を切った場合、影響を受ける性質は
`inconclusive` となります。未完了を `on_limit` の到達や成功・失敗に置き換えません。
再確認の例の clock を6分にすると、7分の終了を確認する前に切れ、
`inconclusive`、exit 2、`repetition.coverage: not_derived` となり、回数表の `cases` は空です。
`coverage` と `unchecked` を読み、未導出の分布を0件と解釈しないでください。

確認用ページは、回数の範囲と分布、上限後の件数と確率、site の round と phase を表にします。
`coverage: not_derived` は、出所ごとの `repetition.coverage` として未確認の集計に1件加えます。
端末の要約、ページ冒頭と結論、埋め込み JSON の `unverified_count` は同じ集計です。
重複項目は除きますが、別の report の未導出を1件にまとめません。

繰り返しを含む Agency の F/U などには、未導出の項目が残ります。
警告が出ないことを統制の成立とはせず、`agency.unchecked` も確認してください。

`compare` の `tables.repetition` では、案件 ID と繰り返し名が同じ行に各案の回数を並べます。
その繰り返しがない案は `not_applicable`、計算の切断などで回数を算出できなかった案は `not_reported` です。
`not_reported` の理由は `reason` が指す各案の未算出欄で確認できます。報告できたセルが一つもない場合は `unchecked` に `repetition_tables` が出ます。
Task の指標行は `loop:呼出し名:round2` など、回ごとに分けます。上限後の呼出しには `loop:呼出し名:on_limit` を使います。
body の並行作業では、合流と待ちの内訳も回ごとに分かれます。fork の内部名は `@loop:繰り返し名:回:fork名` です。
手で展開した構成の通常の呼出しとは自動で対応させません。

`diff` の `repetition` 行には、繰り返しの名前、上限、間隔、元の呼出し名、`on_limit` の終端が出ます。
`relation` の生産者は元の呼出し名で示します。実際の回数や上限に達する確率を比べるには `compare` を使ってください。

`sensitivity` の `effective_inputs` は、回数を条件にした kernel の変数を `loop.繰り返し名.round`、値を整数で示します。
回数の制御に使う内部変数は、調整できる入力ではないので `initials` には出ません。
上限や間隔が異なる点は `sensitivity_contract_mismatch` で拒否します。

未対応で分析できなかった案は、その案の判定と診断で確認してください。`compare` と `diff` の全体判定では `invalid`、`violated`、`unsupported` の順に優先します。
`compare` が `unchecked` に `record_differences` を示すときは、登録者・読取り手の差分を `diff` で確認してください。

## 確認用ページと仮定の出所

`skills/relca-model/scripts/review_page.py` は source と Relca の JSON から確認用ページを作ります。
表と流れ図は source の表示であり、Relca に代わってモデルの妥当性を検証するものではありません。
繰り返しの図には名前・回数上限・gap、body と `on_limit`、`again` の戻り先、`leave` の出口、
`end` を出します。入れ子や `over` など拒否された形は「Relca が受け付けない形」として区別します。

ページに渡す `--assumptions` は、数値や選択をどこから得たかの申告です。
Relca 本体の `assumptions[]`（既定値や分析の前提）とは別です。たとえば次の JSON を渡します。

```json
{"assumptions": [
  {"target": "poll.at_most", "value": 3, "basis": "説明用の仮定", "status": "placeholder"},
  {"target": "poll.gap", "value": "5min", "basis": "説明用の仮定", "status": "placeholder"},
  {"target": "Extract.via_model", "value": "doc@v3", "basis": "モデル宣言を転記。運用は未確認", "status": "placeholder"}
]}
```

| target | 意味 |
|---|---|
| `NAME.at_most` | 繰り返しの上限回数（初回を含む） |
| `NAME.gap` | 明記した繰り返しの間隔。省略時既定の0分には target を作らない |
| `Actor.via_model` | `via model` に宣言した経由モデルの版 |
| `Actor.capability.duration` / `kernel.NAME` | 操作の所要時間 / 確率表 |

target の `NAME` や `Actor` は宣言名に置き換えます。宣言に対応しない target と重複は警告します。
`confirmed`（`confirmed_by` が必要）や `measured`（`source` が必要）も AI の申告として表示し、
人が確かめるまで確認済み・実測済みとは扱いません。
ほかの target は[補助資料の一覧](../../skills/relca-model/references/results.md#確認用ページの-assumptions)を参照してください。

## ログから導いた候補

`relca estimate <model> --log <events.jsonl> --spec <estimate-spec.json>` は、実績ログの観測窓に限って確率と所要時間の候補を出します。モデルは変更しません。`relca-estimate/v1` の JSON では、次の欄を照合してください。

| 欄 | 読み方 |
|---|---|
| `log` / `provenance` | ログの SHA-256、時刻の窓、案件数、端点の申告、使ったイベントの行範囲。hash と端点の申告は、ログの真正性・完全性の認証ではありません |
| `n`, `counts`, `unknown`, `censored` | 対応づけできた開始 Task の数と結果別の件数、未知、打切り。開始されなかった案件の数はこのログから分かりません |
| `candidate` / `interval` | 厳密な候補と DSL 用の値、割合の Clopper–Pearson 区間。`multiplicity` に指定信頼水準と補正が出ます。最小標本数は既知の件数で判定します。未知ありでは割合の候補が出ても `partial` です。未観測の枝は確率 0 を提案せず、観測済みの枝が丸めで 0 になる場合は厳密値だけを残します |
| `duration_summary` | 所要時間の経験分布とは別の記述統計（平均・中央値・最小・最大など）。平均に統計区間はありません。打切りなどで候補を出せないときも、使える完了分の要約は残します |
| `rounds` / `occurrences_per_case` | 回に割り当てられる場合の回別の表と、回に割り当てられない場合の出現数の分布。各回の単位は案件です。回を混ぜて n を増やさず、`given [NAME.round]` の行は該当する回で読みます |
| `declared_vs_candidate` | 宣言値が候補と一致するか、区間内か。候補の採否には使えますが、exit は変えません |

exit 0 は選んだ欄を導出でき、未知・打切り・未対応がなかったという意味です。業務の適合判定ではありません。
exit 2 では `withheld` と `reason` を確認します。入力切断では候補も件数も null です。
exit 3 は入力不正、exit 4 は構造上導けない要求などの未対応です。未対応の欄は `not_derivable` と理由が出ます。他の欄に使える候補が残る場合があるので、欄ごとの状態も読んでください。

確認用ページには `--estimate estimate.json --estimate-exit N` を渡せます。別モデルの結果は使わず、宣言値が区間から外れれば警告します。宣言値と候補が一致し、`derived`・`complete` で未知・打切りがともに0の場合に限り、ログを根拠として「出所の申告なし」から外します。一部未確定なら件数を添えて候補を表示するだけです。候補は観測窓の結果であり、別の期間や To-Be の確率ではありません。ログの真正性や代表性は人が確かめてください。

指定ファイルの例と転記時の確認は、[実績ログから候補を出す](../../skills/relca-model/references/estimate.md)にあります。

## 構造の出力

JSON の `agency` はモデルの呼出し、読取り、条件、commit から作ります。
O/F/P/J/U/E/L は次のような集合・件数・状態を読む記号として使います。
鍵名は製品 API であり、入力に同名の自己評価欄を追加するものではありません。

| 記号 | 主な欄 | 操作的に読む内容 |
|---|---|---|
| O | `origin`, `viewpoints.observation` | source までの直接・間接経路、Actor と実行者の集合。`stages` は呼出し位置ごとの直接の読取りと生成物経由を分ける |
| F | `viewpoints.framing` | 終端への到達条件を変える call の列と対象集合 |
| P | `viewpoints.proposal` | 現在は `not_derived` とその理由 |
| J | `lineage` | commit の効果に関わり、引数などで先行 call の結果を受ける判定 call と、その祖先の根拠源。合流に依存する判定は理由付きで未導出 |
| U | `refusal` | commit を止める条件と迂回文脈、`all_paths` / `conditional` / `none` |
| E | `viewpoints.effects` | commit の位置、実行可能な人、Permission や Authority の記録 |
| L | `viewpoints.retention` | 通常経路での呼出し回数の min/max、contest route や intent の有無 |

`case.id` だけを引数に取る call や、source を `reads` するだけで他の call の結果を受けない call は、J に数えません。
他の call が書いた観測値や、先行の登録を持つ記録を読む場合も、先行結果への依存として扱います。
繰り返しでは、`case.id` 以外の引数を受け、条件に使う call も対象に加えます。

`not_derived` は「存在しない」ではありません。`rules`、`scope`、`unchecked` を併読します。
回数や経路の有無から、実際の人物の能力や業務上の正当性を判定できません。

## 呼出し位置ごとの観測

`agency.viewpoints.observation.stages` は、process の呼出し位置ごとの表です。
同じ操作を2か所で呼べば2行になります。判定に限らず、前の入力操作や確定する操作も出します。
構造上の読取りであり、その位置が実際に実行された記録ではありません。
分岐の枝や取り消されうる fork の呼出しも表に残します。

| 段の欄 | 読み方 |
|---|---|
| `call`, `actor`, `capability` | 結果の名前、Actor、修飾した操作名 |
| `people`, `unattributed` | Actor の `executors` の人を名前順に出す。人が無ければ空の並びと true。資格・Permission・実際の実行者の証明ではない |
| `direct` | この操作自身の `reads` にある source Actor。名前順。引数名の reads や、観測変数・潜在値・条件式は含めない |
| `via_outputs` | 引数と観測変数を通して祖先から届く源のうち、direct に無いもの。名前順。直接読んだことには数えない |
| `via_records` | 登録を使うモデルだけ。全経路で届く記録経由の源、登録した呼出しと人を record ごとに示す。direct には加えない |
| `via_outputs_status`, `via_outputs_reason` | 導出できれば derived で reason は無し。合流した値に依存すれば not_derived / merge_control、via_outputs は null。空の並びと区別する |

観測変数を通る経路は、字句上の先行する書き手を親にするため、届くかもしれない源の上近似です。
`repeat` と有限再送の試行は元の呼出し位置へまとめ、生成物経由の源はその位置の分を合わせます。
同じ源を読み直しても `lineage` の独立した根拠源や W-LIN-2 の判定には加えません。
`source record` と宣言した原本の由来は推測しません。登録を明示した記録の由来は次の別欄で読みます。
前後で同じ版を読んだかは、この観測の表だけでは判断しません。

[段ごとの例](../../examples/stage-observation.relca)の手値は次のとおりです。

| call | 操作 | people | direct | via_outputs |
|---|---|---|---|---|
| e | Clerk.enter | c1 | Invoice | 空 |
| v | Clerk.confirm | c1 | 空 | Invoice |
| k | Ledger.post | 空(unattributed) | 空 | Invoice |

### 記録経由の到達を読む

[登録の例](../../examples/record-provenance.relca)では、origin の Reviewer、人 q1 の normal、段 b に、同じ形の欄が出ます。

```json
{"via_records":[{"record":"Notes","registrars":[{"call":"r","people":["p1"]}],"sources":[{"source":"Know","route":"registrar_direct"}]}]}
```

- `record` は読んだ記録。record の名前順に1要素ずつ出ます。
- `registrars` は最後の登録となりうる呼出しを位置順に示します。`people` が空なら人を特定していません。
- `sources` は全経路で届く源を名前順に示します。`registrar_direct` は登録者自身の直接の読取り、
  `relayed` は引数・観測変数・別の記録からの中継です。
  複数候補のいずれかが中継なら、その源は relayed として出します。
- 空の並びは確定した記録経由の到達が無いことを表します。登録が無い旧モデルでは欄自体を出しません。
- 届くかもしれないだけの源は、既存の `indirect` の上近似に残します。専用の may 配列はありません。
  `via_outputs` は即時の記録辺を数えず、引数と観測変数から届く源を示します。

記録自身は根拠源の節点になりません。lineage の subject 側へ由来を引き継ぎ、独立した源として足しません。
暗黙知が新しい源として W-LIN-2 を抑止する場合も、`wlin2_tacit` の不確実性の開示を残します。
条件付きの登録だけから届く源を、新しい確定した源として警告の抑止には使いません。
ここでいう観測は、モデルに書いた操作の系譜です。実際の資料アクセスや知識の正しさを認定するものではありません。
記録は値を持たず、観測変数と潜在の値の区別も変えません。

mutate で記録の読取りを削除する変異点は、`site.record` に記録名を出し、`site.source` は null にします。
原本を読む辺の削除は従来どおり `site.source` です。同じ変異の種類の中でも、この欄で原本と記録の読取りを区別します。
登録する効果そのものを削除する変異ではありません。

### 責務の行全体と、その段を分ける

`responsibilities[].observation` の既存の `status`、`direct`、`missing_sources` は人の案件全体の欄です。
`stages` は対象のうち本人が `executors` に含まれる呼出しごとの `call / required / direct / missing` を示します。
`required` はその位置の引数と自身の reads から求め、`missing` は required から direct を引いた集合です。
同じ操作の別の位置に、互いの入力の源を要求しません。

上の例では、案件全体の direct は Invoice、missing_sources は空で status は present のままです。
一方、責務の段 v は required が Invoice、direct が空、missing が Invoice です。
別の段 e で同じ人が読むため、W-RESP-3 が責務の行に1件出ます。message に段と源を並べます。
案件全体で直接にも確定した記録経由にも届かない源は、W-RESP-1 の対象です。同じ源を両警告へ重ねて数えません。
W-RESP-3 は規範上の警告で、警告だけでは性質の verdict や exit を変えません。

登録の例では、責務の行にも同じ `via_records` が付きます。
必要源 Know が記録から確定して届くため missing_sources は空、status は `present_via_record` です。
直接の読取りだけで満たせる行は従来どおり `present` です。
段の required−direct は変更しないので、段 b の missing には Know が残り、段の via_records が届き方を説明します。
同じ人が別の段で Know を直接読んでいれば、記録経由でも W-RESP-3 は残ります。
必要源を求める規則は変えません。また、記録だけの到達に直接の時間経路を作らず、時間の証跡を再構築できなければ未検査です。
conform の `unchecked` 配列の要素 `registration_provenance` は、ログで登録の由来を照合していない印です。

| `stages_status` | 段の並びと理由 |
|---|---|
| derived | stages は導出した段の並び。stages_reason は無し |
| not_applicable | stages は null。保守の maintenance、または対象操作の Actor の executors に本人が含まれない person_not_executor |
| not_derived | stages は null。fallback_path、contest_path、target_not_called、no_person_stage、merge_control、no_source_identified、derivation_limit など、導出しなかった理由を stages_reason に残す |

状態条件の責務では、その条件が読む変数の書き手のうち本人の段を対象にします。保守は段にしません。
責務の required は引数の辺と自身の reads をたどり、観測変数の辺は含みません。
この範囲で必要源を特定できない形は、no_source_identified と responsibility_observation の unchecked に残します。
O の via_outputs に源が出ていても、それだけで責務の必要源が導出済みとは読まないでください。

縮退経路の呼出しは段の表に入れず、agency.unchecked に `stage_observation(fallback_path)` を残します。
責務の対象が本人の縮退経路の呼出しにしか無い場合は、対象外でなく未導出の fallback_path です。
Rule の `decides` の値を条件で評価する働きは独立した呼出し位置ではなく、段にせず `stage_observation(rule)` を残します。
ただし、Rule Actor の Capability を process から明示的に呼ぶ操作は、他の Actor の操作と同じく段に出します。
異議経路は通常の段にせず、`responsibility_stage(contest_path)` を残します。
計算上限では observation block 全体が not_derived / derivation_limit となり、途中の段表は出しません。
段の計算も同じ予算を使うため、追加の計算で上限に達すると、既存の sources/people や責務の行全体も未導出になり、既存の W-RESP-1 が出なくなる場合があります。

`original_access_removed` は、段だけの missing 増加と W-RESP-3 の出現を悪化として検出します。
同じ源を旧 missing_sources の増加で既に数えた場合、段の記録を重ねません。
源の消失による段の missing 減少を、新たな改善の記録にはしません。`evidence_missing` の割当は変わりません。
diff / compare は構成間の段の対応づけを行わず、最上位の unchecked に
`stage_observation(stage_correspondence_not_defined)` を出します。段の差が無いという意味ではありません。

確認用ページは、責務の段の欠落を案件全体の欄と並べます。
stages が無い旧出力は「段ごとの記録なし」と表示し、欠落を確かめたことにはしません。
引数の生成物だけを見る段には「この段では生成物だけ」と表示します。

## 複数案とその他の出力

- compare は `plans[].report`、diff は `from.report` と `to.report` を持ちます。
  最上位だけでなく各案の判定も報告してください。未対応の組合せでは、
  案の report が violated でも比較自体は unsupported になります。
- compare の表、hard の採否、目的の比較を区別します。目的の良さで安全性違反を相殺しません。
  diff の `differences` は構造差分で、未計算の種類の不在を「変化なし」とは読めません。
- mutate は `original` と生成した変更の結果を分けます。`run` が未完了のものや未対応種は
  検出・未検出の数に混ぜません。件数から検出率や網羅率を主張できません。
- 応答喪失の `response_receipts` は到達可能な観測の集合です。実行順や発生率の一覧ではありません。
  `case_outcomes` の `receipt_unknown` と `receipt_resolution` の保留を、成功や失敗に置き換えません。
- conform は観測ログの範囲の照合、sensitivity は manifest の有限点の分析です。
  子の結論、理由、対象を最上位の表示と一緒に読みます。

[保証の範囲](guarantees.md) と [手計算](../../fixtures/README.md) も参照してください。

### 比較の採否と目的を読む

宣言つきの `compare` では、必須条件の判定が `evaluation.hard`、案ごとの採否が `evaluation.plans`、目的の値が `evaluation.objectives` に出ます。区間の端は次の根拠と一緒に読みます。

| `basis` | 読み方 |
|---|---|
| `path_range` | 網羅探索で比較した目的の値です。`metrics.overall` の指標について、到達しうる経路での最小・最大をそのまま示し、平均や期待値ではありません。 |
| `with_undetermined` | 打切りや切断で未確定の分を含めて、網羅探索の cohort の必須条件を判定した区間です。未確定を条件に有利に数えず、区間全体と閾値を比べます。 |

`evaluation.plans[].status` は、案自体の検査結果 `verify_verdict` と必須条件の判定 `hard_verdict` で決まります。理由は `reasons` で確認してください。

| 採否の `status` | 読み方 |
|---|---|
| `admissible` | 両方が `satisfied` で、比較上の採用候補です。実際の採用を決めたわけではありません。 |
| `excluded` | どちらかが `violated` で、採用候補から外れました。性質そのものを評価対象から外したという意味ではありません。 |
| `undetermined` | 違反で候補外とはならなかったものの、両方が `satisfied` でもないため、採否を確定できません。未対応や空虚な成立も合格には数えません。 |

`evaluation.pareto` は、採用候補の中で、すべての目的で劣らず、少なくとも一つで確実に優る案があるかを調べます。目的の総合点や順位は出しません。`non_dominated` は確実に劣るとは判定されなかった案であり、最適だと証明された案ではありません。

`status: not_computed` のときは優劣を計算していません。結果のリストが空でも、案が同等だとは判断しないでください。

| `evaluation.pareto.reason` | 読み方 |
|---|---|
| `simulation_estimates` | 標本推定で比べる実行です。目的の標本値は読めますが、優劣判定には使いません。 |
| `no_objectives` | 比べる目的が宣言されていません。必須条件の判定や採否は別に読めます。 |
| `no_admissible_plans` | 採用候補がありません。候補外になった理由と、採否を確定できない理由は案ごとに確認します。 |

網羅探索で比較を行っても、優劣を確定できない組は `undetermined_pairs` に出ます。

| 組ごとの `reason` | 読み方 |
|---|---|
| `missing_value` | どちらかの案で目的の値が欠けています。欠けた値を 0 に補って比較しません。 |
| `overlapping_intervals` | 目的の区間が重なり、この比較ではどちらが確実に優るかを判定できません。区間の中点で順位を付けたり、同等とみなしたりしません。 |

## mutate の範囲と実行上限

### summary_reused と no_assigned_check

`summary_reused` には、Relca の系譜の規則を確かめる役割があります。
要約の作り手を複製しても、同じ根拠源を独立した検証として二重に数えないことを調べます。
この比較での `detected` は、系譜の規則が保たれたという意味で、モデルの強さの点数ではありません。
`undetected / no_assigned_check` も、対応する系譜の検査を割り当てられなかったという意味です。
**モデルが弱いという印ではありません。** 呼出しが系譜の判定行を持つか、比較できる根拠があるかに依存します。

`same_version` を書いた場合は、その性質の行もこの変異の検査へ割り当てます。
関係しない消費者を付け替えた変異体でも行の割当てはあるため、悪化が無ければ理由が `no_assigned_check` から `no_worsening` に変わることがあります。
`satisfied` または `vacuous` から `violated` への悪化は、モデルに書いた版の要求による検出です。
どちらの意味で検出したかは `assigned_checks` と `assigned_changes` を見てください。
[確認した版を承認する例](../../examples/same-version.relca)の再利用1件は、この契約の悪化で `detected` になります。
すべての使い回しを検出するという意味ではなく、件数や検出率を他のモデルへ一般化できません。

系譜の比較自体が独立した根拠を余分に数えた `independence_credited` なら、契約が悪化しても
`undetected / independence_credited` を維持します。契約の悪化は `assigned_changes` に残し、規則の失敗を隠しません。
契約が変わらなかっただけでは、照合できなかった系譜の検査を完了扱いにもしません。

### 実行数と選択

```sh
relca mutate model.relca --mutant-limit 512 --kinds evidence_missing,original_access_removed
```

`--mutant-limit N` は変異体全体の実行数の上限です。既定は128、指定できるのは
1〜4096の整数です。0、負数、非整数、空値、上限超過は `invalid` (exit 3)。
受理した計算上限は `run.mutant_limit` と `ledger.execution.limits.mutant_limit` に入ります。
上限後の候補は `not_run` / `mutant_limit` として残り、run は
`completion: inconclusive` / `reason: mutant_limit` になります。
未対応種が選択されていれば、最上位の verdict は優先順により `unsupported` です。

`--kinds` は `mutations[].kind` の名前をカンマで区切って指定します。
名前は `evidence_missing`、`authority_expired`、`hold_delayed`、`command_resent`、
`summary_reused`、`stale_definition_write`、`receipt_unknown`、
`old_promise_denominator_removed`、`refusal_path_cut`、`original_access_removed` です。
未知名、空リスト、空要素、重複、余白は `invalid`。省略時は全種です。
選外の行も残り、`status: not_selected`、`reason: not_selected`、`mutants: []` と表示します。
`unchecked` の `mutation.kind.<kind>` も併読してください。
選外は成功にも検出漏れにも数えず、verdict と completion は選択範囲だけで決めます。
明示した範囲の除外であって計算の切断ではないため、選外だけでは `inconclusive` にしません。
実行した変異体が0件でfoldがsatisfiedになる場合だけ `vacuous` (exit5)、reasonは `no_selected_mutant` です。
未対応種が選択されてfoldがunsupportedなら、0件でもunsupported/exit4を維持します。
該当箇所なしやkinds/sitesの積集合が空でも、成功とはしません。元の検査の拒否や計算の切断は別扱いです。

| 欄 | 意味 |
|---|---|
| `mutations[].candidates` | 既存の列挙規則で見つかった候補数。選外や `not_run` の分も含む。該当なしは0。未対応・元の検査で停止など、列挙できない場合は null |
| `run.candidates_total` | 列挙できた種類の候補数の合計。選外も含む。列挙に進めなければ null |

全種を回すには合計、種類を絞るなら選択した行の合計を実行上限の目安にします。
合計が4096超なら種類を分けられますが、1種類だけで4096超なら全候補を実行できません。
上限を増やしても、個々の検査の切断や未対応は解消するとは限りません。
候補数は実行数でも網羅率でもなく、null を0と読んではいけません。

場所を絞るには `--sites ID1,ID2` を指定します。ID は出力の `mutations[].mutants[].id`
をそのまま使います。kind と既存の site の区別を含むため、同じ reads でも種類が違えば
別IDです。ソースを編集するとIDが変わることがあります。未知ID、空指定・空要素、重複は invalid。
`--kinds` と両方指定すると積集合を実行します。
選外の場所も mutant 行に残り、`result: not_selected`、`reason: not_selected`、report は null。
`unchecked` の `mutation.site.<id>` と併読します。選外の場所は予算・検出・未検出・成功に
数えず、それだけでは inconclusive にしません。候補数は選外の場所も含みます。

種類を指定せずに `mutate` を実行すると、繰り返しの有無にかかわらず未対応の `stale_definition_write` を含むため、全体は `unsupported`（exit 4）になります。繰り返しを含む構成では、下表のとおり未対応の種類が増えます。どの変異が検出されたかは各変異の行を読んでください。対応する種類だけを回すには、`--kinds evidence_missing,hold_delayed,summary_reused,refusal_path_cut` のように選びます。

| 繰り返しを含む構成での変異 | 扱い |
|---|---|
| `evidence_missing`、`hold_delayed`、`summary_reused`、`refusal_path_cut` | 元の呼出しや分岐を単位に作る。割り当てた検査で悪化を確認できた変異は `detected`。 |
| `refusal_path_cut` の拒否経路 | 反復があると構成全体の拒否経路を導出しない。ほかの割当て済み検査で悪化を検出できなければ、反復の外の分岐でも `unsupported / loop_control`。未検出とは区別する。 |
| `summary_reused` の系譜 | 対象の消費者の系譜が回ごとの内部名でしか得られず、束縛名で照合できない場合、ほかの割当て済み検査で検出されなければ `unsupported / loop_control`。 |
| `authority_expired`、`command_resent`、`receipt_unknown`、`old_promise_denominator_removed`、`original_access_removed` | 繰り返しを含む構成では `unsupported / loop_feature_combination`。反復の外側に候補になりそうな箇所があっても、その種類全体を実行しない。 |
| `stale_definition_write` | `unsupported / rule_version_unavailable`。 |

反復の本体に `fork` がある構成の `mutate` は未対応です。`wait until`、`quantile`、`evidence_basis`、`registers`、応答喪失・再送を使う構成と反復の組合せも `loop_feature_combination` で拒否します。ほかのコマンドで解析できることと、変異の判定に使えることは別です。

授権失効の変異は、全操作が0分で、切断の無い完了時刻もすべて絶対時刻0の場合、
`not_applicable` / `no_time_elapse`、候補数0です。期限0は時刻0を含むためです。
0分の操作でも、到着や待ちで時刻が進む場合はこの除外をしません。
全応答のbackoffが0であることも直接検査します。正のbackoffが到達しない箇所にあっても除外しません。
静的な正のWaitまたはWaitUntilがあれば、End後や到達しない枝も含めて同じく除外しません。
カレンダーも別の時間源なので、Actor・資源・人のカレンダーを含むresource_timeがある場合は、この除外をしません。
この保守的な判定では実際の時刻が進まなくても変異体がundetectedとなり得ますが、その場合はviolated/exit1でありexit0にはなりません。

構成を明示選択したときは、構成の数を問わず実効process内のcall・reads・guard・終端に候補を絞ります。
共有Capabilityのaffected_bindingsにも、そのprocessの呼出しだけを記録します。
種類の候補箇所を1つでもsitesで外すと、その種類の段はcheckedに入れずuncheckedへ残します。
全体の生成/比較の段も同様です。kindsで1種類でも外した場合も全体の段はuncheckedです。
比較を0件しか行っていない段もcheckedには載せません。
構成の数を問わず、明示した構成の実効processで列挙します。無指定の宣言ベースの列挙は保持します。
不正な上限やsite IDでは上限は未使用なので、run/ledgerのmutant_limitは
not_applicable / value:null / reason:no_analysis と表示し、128とは表示しません。
