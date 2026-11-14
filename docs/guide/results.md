# 結果の読み方

CLI は標準出力に JSON を出し、診断は標準エラーにも書きます。
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
| `bounds` | 案件数、同時 Task 数、clock、`truncated` |
| `properties[]` | 各性質の `verdict`、確率境界、根拠や注記 |
| `contract_scope[]` | 選んだ構成では範囲外にした契約、その適用先と理由。検査済みの性質ではない |
| `terminals[]` | 到達先別の時間・work の範囲。平均とは限らない |
| `diagnostics[]` | code、severity、message、入力内のバイト位置 span |
| `assumptions[]` | 既定値や分析で置いた前提 |
| `actors[]` | 宣言値と省略時に補った欄 |
| `metrics`, `quality` | 要求した量と、算出できない理由 |
| `repetition` | 繰り返しの回数、上限後の処理への到達、元の呼出し位置との対応 |
| `ledger`, `uncertainty` | 入力選択、結論の根拠、未検査事項の記録 |
| `unchecked` | 今回は検査していない項目 |

最上位が violated でも、別の性質は vacuous や inconclusive のままの場合があります。
null は 0 ではありません。`not_configured`、`undetermined`、`truncated` などの理由を
値と一緒に扱ってください。`per_case_relaxation` の区間を一つの方策で実現可能な区間と読まないでください。
simulate の `basis` は性質や中断の有無で異なり、`properties[].cases[]` の `basis: statistical` は網羅探索の確率ではなく標本推定を表します。

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
| `W-LIN-2` | 独立な検出力に必要な新しい根拠源 |

導出しなかった対象は、コードと `merge_control`、呼出し名・操作名・責務の対象などを
`agency.unchecked` に記録します。警告が無いことを、問題が解消した証拠と読まないでください。
責務の observation / refusal も依存する欄だけを `merge_control` とし、独立した欄の
`W-RESP-1` などは従来どおり導きます。`derivation_limit` は実際に計算上の上限へ達した場合の理由です。
ただし、同じ判定が合流に依存する効果にも関わる場合、責務の refusal は欄全体が
`not_derived(merge_control)` になります。独立した commit への拒否が欠けていても、
その欄の `W-RESP-2` は警告ではなく未導出の印として残ります。

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
| `missing` | required のうち sources に含まれなかった源 |
| `excluded_edges` | 必ず先に正常終了して最後の値を書くとは認めず、数えなかった観測変数の辺。`p -> a` のような binding 間の表記 |

後半の4欄は名前の昇順の配列です。`excluded_edges` がある場合、そこを読んだはずだと補完しません。
反例は参照実行器で再生した経路であり、`evidence` はモデル上の系譜の証跡です。
実際の資料アクセスを記録したログではありません。確認用ページも、この区別を付けて表示します。

`mutate --kinds evidence_missing` はこの契約の行も割当てに含めます。
`original_access_removed` に割り当てるのは `direct` の行だけです。
[正の例](../../examples/evidence-basis.relca)では2つの読取り削除をどちらも検出しますが、
他のモデルの検出件数や確率・網羅率を表す値ではありません。
原検査の全体が `vacuous` なら、mutate は候補の実行へ進みません。
契約が悪化しても、同時に割当てた系譜の行が消えて照合できなくなった場合は、
`inconclusive / assigned_check_unavailable` のままです。契約の変化だけで `detected` に置き換えません。

選んだ構成にこの契約が適用される `compare`、`diff`、`sensitivity` は、対応外の比較として exit 4 で拒否します。
diff の個別 check が `violated` でも、最上位の `unsupported` は比較していないという意味です。
個別の違反を、比較の拒否で打ち消さないでください。
異議経路を対象にした契約は `evidence_basis_contest_route` で未対応にします。
通常の呼出しとして追跡できない対象を `vacuous` として隠さず、他の性質の結果は残します。
`conform` は `U-LOG-1` で拒否し、`not_checked` に契約を `unsupported_request` として、
`unchecked` に `evidence_basis` を残します。読取りを観測しないログから満たしたとは判定しません。
契約が範囲外なら、未対応要求ではなく `contract_scope` に残します。

## 繰り返しの出力

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

## 構造の出力

JSON の `agency` はモデルの呼出し、読取り、条件、commit から作ります。
O/F/P/J/U/E/L は次のような集合・件数・状態を読む記号として使います。
鍵名は製品 API であり、入力に同名の自己評価欄を追加するものではありません。

| 記号 | 主な欄 | 操作的に読む内容 |
|---|---|---|
| O | `origin`, `viewpoints.observation` | source までの直接・間接経路、Actor と実行者の集合 |
| F | `viewpoints.framing` | 終端への到達条件を変える call の列と対象集合 |
| P | `viewpoints.proposal` | 現在は `not_derived` とその理由 |
| J | `lineage` | commit の効果に関わり、引数などで先行 call の結果を受ける判定 call と、その祖先の根拠源。合流に依存する判定は理由付きで未導出 |
| U | `refusal` | commit を止める条件と迂回文脈、`all_paths` / `conditional` / `none` |
| E | `viewpoints.effects` | commit の位置、実行可能な人、Permission や Authority の記録 |
| L | `viewpoints.retention` | 通常経路での呼出し回数の min/max、contest route や intent の有無 |

`case.id` だけを引数に取る call や、source を `reads` するだけで他の call の結果を受けない call は、J に数えません。
他の call が書いた観測値を読む場合も、先行結果への依存として扱います。
繰り返しでは、`case.id` 以外の引数を受け、条件に使う call も対象に加えます。

`not_derived` は「存在しない」ではありません。`rules`、`scope`、`unchecked` を併読します。
回数や経路の有無から、実際の人物の能力や業務上の正当性を判定できません。

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

## mutate の範囲と実行上限

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
