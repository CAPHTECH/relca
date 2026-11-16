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

check・explain・simulate の report(compare の各案の report も同じ)の全体の verdict は、次の結果を `invalid > violated > unsupported > inconclusive > vacuous > satisfied` の優先順でまとめる(`crates/relca-core/src/report.rs` の `fold_verdict`):

- 性質ごとの verdict
- diagnostics の severity が error のもの: code が `U-` で始まれば unsupported、それ以外は invalid
- 性質が一つ以上あるときの `W-DEADLOCK-0`: violated(到達できる deadlock は反例)
- 上のどれも無い(性質が空で error も無い)とき: unsupported

warning は中立で、全体の判定には数えない。
後段(ledger の完成処理)で全体が重くなる場合があるかは確かめていない [U]。examples/invoice の実出力では、全 report で計算し直した判定と一致した。

compare と diff の最上位は、通常は各案(diff は from と to)の report の verdict と、compare・diff 自身の error の診断を同じ優先順でまとめる(`crates/relca-core/src/compare.rs` の `fold` と `diff.rs`)。
**例外:** 次の拒否では、結果をまとめた後で最上位を unsupported に上書きする。各案の report が violated でも最上位は unsupported になり、単純にまとめた判定より軽い。

- `priority_feature_combination`(compare・diff)
- `retry_feature_combination`(compare・diff。再送を含むモデル)
- `condition_wait_diff_unsupported`(diff だけ)
- `merge_feature_combination`(compare・diff。合流を含むモデル)

EC 試行(再送を含む 2 案)では、compare も diff も最上位 unsupported・exit 4、各案の report はどちらも violated だった(1 回)。各案の性質の verdict は `check --configuration` の結果と一致した。
この拒否では、単に各案の最も重い判定を選ぶだけでは最上位を再計算できない。読むときは次のとおりにする。

- 最上位の unsupported は「比較・差分をしていない」という意味で読む。
- 案ごとの結論は `plans[].report.verdict`(diff は `from.report.verdict`・`to.report.verdict`)から読み、**両方を報告する**。案の violated を最上位の unsupported で打ち消さない。
- exit は最上位の verdict に対応する(上の例は 4)。exit だけ見て案の違反を見落とさない。

review_page は、check・explain・simulate・各案・mutate の original の全体の判定を `fold_verdict` と同じ規則で計算し直す。compare の最上位は各案の verdict の最も重いものと照合する(`scripts/review_page.py` の `scan` の `kind == "compare"` の分岐)。どちらも全体の verdict が軽ければ「全体の verdict を信用しない」と警告する。上の拒否では、この警告が出るのが正しい。
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
| `contract_scope[]` | 範囲外の契約のid、適用先、理由、not_applicable、masks_call | 検査済みではない。uncheckedにもcontract.scope.IDを残す |
| `terminals[]` | 終端ごとの `elapsed` / `work` の min..max、確率 | 全到達経路の範囲。平均ではない |
| `assumptions[]` | `field / value / affects` | 既定で埋めた欄・置いた仮定の一覧。報告に必ず付ける |
| `actors[]` | Actor の各欄と `status: declared / default` | `default` は省略時既定。業務の人に確かめてもらう対象 |
| `diagnostics[]` | `code / severity / message / span{start,end}` | span はモデルの **バイト位置**(lexer が byte span) |
| `agency` | origin・lineage・refusal・common_causes・viewpoints | 入力構成から導出する欄で、自己申告の値は受けない。実運用の効果を測定した値ではない |
| `responsibilities[]` | 人・対象・案件全体と各段の根拠源の読取り・拒否の到達 | W-RESP-1(案件全体で観測が無い)・W-RESP-2(拒否が効果に届かない)・W-RESP-3(別の段でだけ直接読む) |
| `metrics` | 時間・費用・件数。`rates` に単価、`not_configured` は未設定または defer による撤回 | 単価未設定も撤回も 0 円ではない |
| `cut.defer_effects` | 指標がある対象 report の defer の解決・撤回範囲・原因 | entries[i] は cut.defer[i]。同名の重複も行ごとに読む |
| `uncertainty` | aleatory / estimation / model の三区分 | モデル不確実性に Cut と系譜の仮定が入る |
| `ledger.producer.version` | relca の版(現在 `0.0.0`、出所 `cargo_package`、build identity は未記録) | `relca --version` は無い |
| `ledger.execution.selection.effective.process` | 実際に選ばれた process | 複数 process のモデルで確かめる |
| `ledger.execution.limits.agency_limit_scale` | Agency導出のwork/cellsの倍率。解析時known、省略は1。未解析はnot_applicable/no_analysis | 状態探索の予算や業務の上限ではない |
| `unchecked`, `agency.unchecked`, `cut.unchecked` | relca が自分で未検査と申告した項目 | 報告の「確かめていない範囲」に写す |
| `response_receipts`, `case_outcomes`, `response_analysis`, 性質 `receipt_resolution` | 再送を含むモデルで出る(試行の再送なしの版には無かった。他の Phase 2 の構文で `case_outcomes` が出るかは確かめていない [U]) | 下の「再送を含むモデルで出る欄」 |
| `repetition` | repeatを含むモデルの回数、上限後の処理への到達、元位置との対応 | 下の「繰り返しを含むモデルで出る欄」 |

確率の `min / max` は完全情報の緩和境界で、実現できる方策の値ではない(assumptions の `probability.scheduler`)。

### defer で値が撤回された場合

`cut.defer_effects.process` が true なら、時間・work・費用と品質の数値は全体で撤回される。
件数や開始回数、品質の試行数などの記録が残っていても、そこから撤回された数値を復元しない。
false なら各行の scope にある Actor・Capability・人・資源が対象。指標に無い行を新設したという意味ではない。
entries の resolution は actor / capability / opaque、capabilities は解決した操作の名前順。
withdraws は process / scoped / none（操作が無い Actor）。because は opaque、call、contest_route で、
後二つには操作名が付く。call は死枝も含む字句上の呼出し。scoped だけ scope を持ち、ほかは null。
全体の process が true でも、別の行が scoped であることはある。
撤回した数値は null、理由がまだ無ければ not_configured。既存の aborted / truncated などの理由は残る。
費用の not_configured は scope と rates の単価・reason・required を併読して判断する。

W-CUT-2 は Actor 名にも Actor.capability にも完全一致しない defer の警告で、対象名と位置を示す。
モデルにできない段をその名前で defer した場合は、opaque による全体撤回が意図した結果。警告と撤回を報告に残す。
宣言済みの層の指標だけを保留する場合と区別し、無関係な操作の部分撤回で値を出させない。
先頭が既知の Actor なら、無い Capability 名も message に出る。警告だけで verdict / exit は変わらない。
正しく解決した操作でも本文・異議経路から全体へ広がるため、警告が無くても because を読む。
check / explain / simulate、compare 各案、diff 両側、mutate 元と変異体、sensitivity 各点で同じ欄を読む。
defer が無い、指標が無い、入力検査で拒否されて指標を持たない report、および conform には欄も警告も出ない。

## 呼出し位置ごとの観測

`agency.viewpoints.observation.stages` は call / actor / capability / people / unattributed と、
直接の source Actor の `direct`、生成物経由だけの `via_outputs` を出す。
源と人は名前順、段は呼出し位置順。生成物経由は観測に数えず、観測変数の辺を含む上近似として読む。
実行者の人は executors の宣言で、資格やPermissionの成立や実行された人を認定しない。
合流依存のviaは null / not_derived / merge_control。directは残す。
repeatと有限再送は元の位置へまとめる。枝の行があっても実行した記録ではない。
同じ源の再読はlineageや独立根拠数に足さない。source record の由来は推測せず、登録を明示した記録は次の別欄で読む。版の一致は観測の表だけでは判断しない。

責務の `observation.stages` は本人の対象の各 call の required / direct / missing。
人全体のdirectにあって当該段に無い源があればW-RESP-3を行ごとに1件出す。
人全体で直接にも確定した記録経由にも届かない同じ源はW-RESP-1だけ。記録を使わない模型の行全体statusと時間・拒否の判定は変えない。
stages_statusはderived / not_derived / not_applicable。未導出・対象外はstages=nullでstages_reasonを読む。
maintenanceと、対象操作のexecutorsに本人が含まれないperson_not_executorだけが対象外。
fallback_path / contest_path / target_not_called / no_person_stage / merge_control / no_source_identified / derivation_limitは未導出。
責務のrequiredは引数と自身のreadsだけで、Oのviaが含む観測変数の辺を含まない。
必要源が特定できなければ責務の未導出とuncheckedを残し、空の成功にしない。

縮退・異議経路の段は未検査。Ruleのdecides値を条件で評価する働きは独立した段にしないが、Rule ActorのCapabilityの明示呼出しは普通の段として出す。
diff/compareの段の対応づけも未検査として最上位uncheckedへ残す。
original_access_removedは段だけの欠落増加とW-RESP-3の出現を悪化として数え、旧missing_sourcesと同じ源は重ねない。
確認用ページは、段が無い旧JSONを「段ごとの記録なし」と表示する。
[全文例と値](../../../docs/guide/results.md#呼出し位置ごとの観測)を参照。

## 記録経由の到達

登録を使う模型だけ、origin、人の normal、観測の段、責務の行と段に via_records を出す。

```json
[{"record":"Notes","registrars":[{"call":"r","people":["p1"]}],"sources":[{"source":"Know","route":"registrar_direct"}]}]
```

record は名前順、registrars は呼出し位置順、sources は源の名前順。
registrar_direct は登録者自身の source reads、relayed は引数・観測変数・記録からの中継。
人の配列が空なら補わない。登録者の候補が複数なら、全候補が運ぶ源だけを確定とする。
may だけの源は indirect に残し、専用配列も via_records の観測も作らない。
空配列は確定到達無し、旧JSONで欄が無い場合は記録経由を調べた結果とは読まない。

direct へは加えず、記録自身を source や independent_sources にしない。
via_outputs は引数・観測変数の上近似で、即時の記録辺は via_records で読む。
暗黙知が新しい源として W-LIN-2 を抑止する場合も wlin2_tacit を開示する。may の登録だけでは抑止しない。

責務の required_sources は従来どおり。must の記録で必要源が届けば missing_sources から外し、
直接だけでは満たせなかった行を present_via_record とする。
段の missing は required−direct のままで、記録の到達は段の別欄に出す。
別段で同じ人が直接見た源なら W-RESP-3 を消さない。
記録だけの到達で直接の時間経路が無ければ、時間の証跡は no_reachability_path で未検査になる。
記録は値を持たず、潜在の値を読めるようにするものでも、版の一致を保証するものでもない。

[実行できる全文](../../../examples/record-provenance.relca)は satisfied/exit0、3分。
q1 の行は present_via_record、段 b は direct=[] / missing=[Know] と上の経路を持つ。
conform は unchecked 配列に registration_provenance を残す。ログで由来を照合していない印として読む。
[詳しい読み方](../../../docs/guide/results.md#記録経由の到達を読む)を参照。

## 差し替えと相手別の理由

diff は計算した差分を残したまま、その理由の出所を示す。説明済みでも業務上の安全性の認定にはならない。

| 欄 | 読み方 |
|---|---|
| `intent_block` | 相手別の宣言があるモデルだけ。`{"from":"A","to":"B"}` は相手別、from=null は行き先の既定、全体がnullなら理由の宣言が無い |
| `differences[].covered_by[]` | replace の from/to と via。direct は能力の削除/追加、derived は差し替えだけで説明できた関連の行 |
| `replacements[]` | from/to、reason、loses、宣言のspanと、覆った全行のcovers（kind/target/change/via） |

covered_by と replacements は from 名順。行と宣言は covers と印で対応し、同じ行に複数の宣言が付くこともある。
必要な宣言も単独で説明できる宣言も無い行は、同時に適用してその行を消した全差し替えに帰属する。
覆った行の reason/loses は null で、文は宣言ごとに1回だけ置く。
個別の intent が優先された行は、covers と covered_by に入れず、個別の理由・注記を保つ。
replace を選ばなかった diff では、covered_by と replacements 自体が出ない。

W-DIFF-1 は理由が当たらない差分。replace で説明しきれない行にも残す。
loses が無ければ W-DIFF-2 に加え、派生行の W-DIFF-3 を残す。
[全文例](../../../examples/capability-replacement.relca)の A→B は10行（direct 2、derived 8）で警告0件。
loses だけを外すと W-DIFF-2 が1件、W-DIFF-3 が8件、W-DIFF-1 は0件。
元を呼び続ける部分差し替えなどは E-DIFF-0 / invalid / exit 3。個別の intent に分けて書く。

compare は intent の差分照合をしない。比較する構成の空の宣言でも unchecked に intent が1回出る。
check/explain/simulate などの成功も理由の照合結果ではない。必要な対ごとに diff を実行する。
確認用ページは source の宣言と compare の未照合を表示し、理由の妥当性を判定しない。
[詳しい結果の読み方](../../../docs/guide/results.md#差分の覆い方)も参照。

## 契約の適用範囲

`applies_to` の範囲外では性質を問わず、not_applicable を verdict や satisfied にしない。
compare は行を残し、非適用セルを status=not_applicable、verdict=null、reason=configuration_scope とする。
適用セルは status=applied。scopeを使わない模型には新status欄を出さない。
非対称ならW-CMP-1、checkedにcontract_scope、仮定にcompare.contract_scope=asymmetric。
宣言評価は各案の適用分を使うため、契約が違う比較だという仮定を併記する。

diffはfrom_status/to_statusで適用を区別し、範囲外側のfrom/toはnull。
kind=contract_scopeのadded/removedで統制の増減を示す。行き先のpreserveが非適用契約を選べばE-DIFF-0。
explainはomittedにverdict=null、reason=not_applicable_configuration_scopeを残す。
mutateのauthority_expired候補にも適用範囲を使い、候補0件はvacuousのまま。
sensitivityは適用先の宣言が変わればsensitivity_contract_mismatch。conformは適用分だけを照合する。

W-CONF-1は、複数構成の模型で、対象の呼出しが字句上も無いvacuous契約への案内。
対象を呼ぶ構成や構成が1つだけの模型には出さない。
W-CONF-2 / masks_call=trueは、範囲外の構成の手順・異議経路・復旧・再送・縮退の代替から対象を呼ぶ場合の警告で、到達しない枝も含む。
Capability参照の無い節もtrue。違反なしの保証にはしない。
確認用ページは適用先と理由を表示し、非適用契約を出所とidごとに未確認の件数へ加える。
最上位のverdictの再計算には加えない。
全文例の実行値とJSON欄は[利用ガイド](../../../docs/guide/results.md#契約の適用範囲)を参照。
最初の案で非適用の契約は表の末尾へ回る場合がある。位置でなくidを使って対応させる。
intentには `change contract_scope Controls reason "適用先を見直す"`、追加/削除には
`add contract_scope Controls reason "承認を追加する"` / `retire contract_scope Controls reason "承認を外す" loses "承認契約"` と書ける。
mutate最上位のcontract_scope/uncheckedにもoriginalと同じ範囲を残す。確認用ページでは同じ元検査の重複としてoriginalだけを未確認に数える。

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
derivation_limitは通常は計算上限を表す。ただし応答喪失のLでは探索切断や二つの探索器の不一致にも使い、limit記録が無い。
警告が出ないことを、統制が成立した証拠にしない。

確認用ページは値を返すif、各yieldの選択元、Taskでない複写を表と図に出し、
反例のmerged段も表示する。拒否された合流は正常な流れとして描かない。
細部は[合流の出力](../../../docs/guide/results.md#合流の出力)を参照。

## Agency の上限と警告

- `W-AGY-4`: 主の原本への到達・系譜・拒否と、責務の既定候補を導出できなかった。
- `W-AGY-5`: observation / framing / retention / effects の計算上限。blockごとに1件で、途中の行を導出済みとして出さない。
- blockの `limit` は counter / limit / requested / phase と、記録できたprocessやjudgement。整数に収まらない要求はrequested=null / requested_overflow=true。
- 不整合な内部表現は `not_derived` / `invalid_ir` で区別し、計算上限の記録は付けない。

W-AGY-5は最終blockがderivation_limitでlimit記録を持つときだけ出す。
loop_control / response_controlへ置き換えた場合は、途中の静的導出の警告を残さない。
合流の独立部分を導出する計算が上限に達した場合は、merge_controlで隠さず
derivation_limit / limit記録 / W-AGY-5を残す。合流値への依存など構造上の未導出だけをmerge_controlとする。

`check`、`simulate`、`explain` の `--agency-limit-scale N` はwork/cellsだけをN倍にする。
既定1、最大1000。depth/paths/contextsと縮退の予算、性質の探索予算は固定。
cellsは累計の計数で、最大常駐メモリの上限ではない。倍率を上げても全モデルで導けるとは限らない。
実際の倍率はledgerで確認し、明示時はassumptionsのagency.limit=computational_xNにも残す。
業務モデルの追加仮定とは扱わず、uncertainty.modelでは既存の計算上限の参照に含める。
未解析のno_analysisを倍率1と読まない。compare/diff/mutate/sensitivity/conform/verifyではこのflagを受理しない。
確認用ページは倍率とblockの上限記録を表示するが、記録が無い旧出力では未記録とし、1を補わない。
詳しくは[利用ガイド](../../../docs/guide/results.md#agency-の導出の上限)を参照。

## 値の版と証跡

same_versionのcheck行はid/verdictと必要時cases。番号は模型全体の契約の項の通し順。
explainの違反反例だけにversionを足し、binding/capability/parameter/versionで使う側を、
examinedの同じ4欄で開始前の最後の確認を示す。確認が無ければexamined=null。
[取り違えた例](../../../examples/same-version-reused.relca)はp/Approver.approve/d/d_dup、確認はk/Checker.check/d/dで、違反は4分。
合流ならyield元の呼出し名を示す。結果は独立参照で再生したモデル上の経路で、実物の内容の認定ではない。
確認用ページも使う版と確認した版を分け、証跡の欠落や不正な形を未確認に残す。

| 版の語 | 対象 |
|---|---|
| version_check | 操作中に他から状態が書かれたかを見る、状態の書込みの版 |
| via model | 経由モデルの版の宣言。同じモデル版でも呼出しの結果の版は別 |
| same_version | どの呼出しの実行が値を作ったか。enumの等しさ、祖先の包含ではない |

vacuous/切断を成功にしない。simulateの反例なしも全経路の安全性ではない。
Agency・責務・via_model_pathsはこの契約で補わない。
conformはU-LOG-1、not_checkedにunsupported_request、uncheckedにsame_version。
[詳しいJSONと境界](../../../docs/guide/results.md#版の要求と証跡)を参照。

summary_reusedの系譜の検査は、同じ根拠を独立検証として二重計上しないというRelcaの規則の検査。
この検査のno_assigned_checkはモデルの弱さの印ではなく、その系譜の比較を割り当てられなかった印。
same_versionの行があれば、その悪化も検出へ数える。何が変わったかはassigned_changesを読む。
independence_creditedなら契約の悪化があってもundetectedのまま、悪化はassigned_changesへ残す。
未照合の系譜を、契約が不変だっただけで完了にも変換しない。
詳細は[mutateの二つの意味](../../../docs/guide/results.md#summary_reused-と-no_assigned_check)を参照。

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
選んだ構成に適用される契約のcompare/diff/sensitivity は U-SEM-0 evidence_basis_unsupported_command で拒否する。
diff の両側の個別検査が violated でも、最上位unsupportedは比較未実施を示す。両方を報告する。
conform は U-LOG-1、not_checked に unsupported_request、unchecked に evidence_basis を残す。
範囲外の契約は未対応要求にも数えず、contract_scopeに残す。
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
