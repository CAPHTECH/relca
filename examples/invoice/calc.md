# 請求書例の独立した手計算

設計 v0.2 §12.3・§12.5 の仮定から計算する。実装の出力や写しは oracle にしない。
一回の受付、一案件、待ち無し、正しい抽出なら判定は必ず pass、登録は成功のみ。
難度は routine が 4/5、difficult が 1/5。難度と抽出の正誤を条件として
二照合と承認は独立である。無条件の独立性や現実での正確さは保証しない。

## 誤登録と終端

人の入力の誤りは routine 1/100、difficult 3/50。
AI の抽出の誤りは routine 1/50、difficult 3/20。
誤抽出時の三つの pass は routine (1/5,3/20,1/10)、difficult (1/2,2/5,3/10)。

As-Is と To-Be P の routine 項:
(4/5) × (1/100) × (1/5) × (3/20) × (1/10) = 3/125000。
difficult 項:
(1/5) × (3/50) × (1/2) × (2/5) × (3/10) = 90/125000。
合計 93/125000 = 0.000744 = 0.0744%。P は順序だけの変更なので同じ。

To-Be A の routine 項:
(4/5) × (1/50) × (1/5) × (3/20) × (1/10) = 6/125000。
difficult 項:
(1/5) × (3/20) × (1/2) × (2/5) × (3/10) = 225/125000。
合計 231/125000 = 0.001848 = 0.1848%。

人の誤抽出は (4/5)(1/100)+(1/5)(3/50)=1/50=0.02。
AI は (4/5)(1/50)+(1/5)(3/20)=23/500=0.046。
正抽出は必ず posted なので、人の posted は
49/50+93/125000=122593/125000=0.980744、returned は
1-122593/125000=2407/125000=0.019256。
AI の posted は 477/500+231/125000=119481/125000=0.955848、
returned は 5519/125000=0.044152。exception は到達しない。
未到達の terminal の行や、0 分の lead_time を捏造してはいけない。

閾値 0.1%=0.001=1/1000=125/125000 に対して、
93 < 125 < 231。確率性質は As-Is/P が satisfied、A が violated。
A の職務分離は vacuous であり satisfied ではない。root は violated が優先する。
担当と開始順によってカーネルを変えないので、これらの確率は min=max。
旧5モデルでは無条件の reached 確率であり、登録条件付き誤り率や品質の分母の宣言は無い。
下の Phase 1 補完では同じ算術を、明示した品質の性質と全流入分母へも照合する。

## 時間・人の作業・全 Actor の作業を区別する

| 案 | posted 経過時間 | posted 人の作業 | Clerk 占有 | terminal.work |
|---|---|---|---|---|
| As-Is | 6+3+5+2+1=17 | 6+3+5+2=16 | 6+3+5=14 | 17 |
| To-Be P | 6+max(3,5)+2+1=14 | 6+3+5+2=16 | 14 | 17 |
| To-Be A | 1+max(3,5)+2+1=9 | 3+5+2=10 | 8 | 1+3+5+2+1=12 |

Clerk の二名は並列時に別の人を使う。開始の待ちは無く、時刻 6 (A は 1) で
二照合が開始し、3 分後と 5 分後に終了する。両方 pass の場合だけ承認の 2 分、
その後登録の 1 分が加わる。同じ posted 経路で Clerk の 14/14/8 と Approver の 2 が
共に実現する。returned では Approver が呼ばれない経路もあるため、その全体の範囲は
0..2。人 c1/c2 の別経路の最大値を足してはいけない。
全 Actor の作業和には Ledger の 1 分と A の Extractor の 1 分も入り、人の作業とは異なる。
17-14=3 分は非 AI の並列化、14-9=5 分は入力の移転。並列化は照合の 8 人・分を減らさない。

## 名目能力と BusyDay の境界

Clerk 二名 × 60 分 = 120 人・分/時。
As-Is/P の名目能力は 120/14=60/7≈8.571428571 件/時。
A は 120/8=15 件/時。これは手計算の名目能力で、runtime が安定性を検証した値ではない。
15 件/時での需要は 15×14=210、210、15×8=120 人・分/時。
利用可能 120 に対して前二案は過負荷、A は余裕 0。
BusyDay の到着は下の別モデルで宣言する。営業時間のカレンダー、安定性、平均、P95、実測は
この一案件の算術からは結論できず、未解決の非決定を残す BusyDay の simulation からも生成しない。

## 受入用の固定値

下表は上の算術の索引。テストはこの表と、実物の出力に別々に assert する。

| key | As-Is | To-Be P | To-Be A |
|---|---|---|---|
| wrong | 93/125000 | 93/125000 | 231/125000 |
| posted | 122593/125000 | 122593/125000 | 119481/125000 |
| returned | 2407/125000 | 2407/125000 | 5519/125000 |
| elapsed | 17 | 14 | 9 |
| human | 16 | 16 | 10 |
| clerk | 14 | 14 | 8 |
| work | 17 | 17 | 12 |
| capacity | 120/14 | 120/14 | 120/8 |
| threshold | 1/1000 | 1/1000 | 1/1000 |
| probability_verdict | satisfied | satisfied | violated |
| separation_verdict | satisfied | satisfied | vacuous |

plan の算術に修正は無い。単価は未入力なので not_configured、0 円ではない。
旧例で品質を生成したとは主張しない。Phase 1 の品質は下の宣言に限る。
事業価値・比較順位・経営要約・AI を介した変化の認定は、どちらでも生成したと主張しない。

## D2: 差戻し後の異議と終端の凍結 (ADR 0022)

ここからは異議の実行時検査の手計算。終端の時刻・名前・その時点の作業量と
誤登録の確率は上表から変えない。posted に異議は無く、returned だけで
出す／出さないの両方を展開する。発生率や二択の平均を置かない。

| 案 | returned の時刻 t | 起票なしの閉鎖 | 起票ありの応答・閉鎖 | 期限 |
|---|---|---|---|---|
| As-Is | 14 または16 | t | 15 または17 | t+7200 |
| To-Be P | 11 または13 | t | 12 または14 | t+7200 |
| To-Be A | 6 または8 | t | 7 または9 | t+7200 |

二照合はjoin allで完了してから分岐するので、returnedの時点でClerkの人は
空いている。recheckはc1またはc2を実際に占有して1分走る。正常結果は
pass/failの両方で、どちらも期限内の応答。終端をpostedへ覆さない。
最後の応答もclock60以内。各案の静的contestの直後のcontest_responseは
satisfiedだが、Aの誤登録違反や職務分離vacuousを上書きしない。
各完走経路で異議も閉じるのでcontest_summary.business_ended_openは0..0。

returned経路にだけClerkの占有と人作業が1分加わる。全終端を合わせた
Clerk占有はAs-Is/Pが14..15、Aが8..9分、人作業は14..17／14..17／8..11分。
posted経路のClerk14/14/8、人16/16/10、terminal.work17/17/12は不変。
全Actor作業の全経路範囲は14..17／14..17／9..12分で、最大が変わらないのは
異議を無視したからではなく、postedのLedger1分とreturned後のroute1分が
同じ最大に届くため。終端費用と全体費用はどちらも単価未設定でnot_configured。
通常call費目と通常call+route費目の充足は別に判定する。

response #6の台帳はcase1:response6:received。trigger0、deadline60、fulfilledで
resolved_atはAs-Is14/16/17、P11/13/14、A6/8/9の集合。免除・違反は無い。
endより前にこの義務を閉じ、後のrecheckで再解決しない。contest #7の台帳は
case1:contest7。起票なしは結果・実行者・期限を捏造しない。起票ありは各tで
実行者c1/c2×結果pass/failの組を保持するが、経路数・頻度・確率ではない。

diff/compareは義務・異議の表を運ぶだけで、その表の差はunchecked。
explainは静的contestをomittedにし、runtimeの違反が無い本例では異議の
反例を作らない。simulationの選択遭遇はunsupportedであり、異議の発生率や
応答時間の実測、受諾、救済、再審、Vendorの実在のStandingは検証していない。

## P1-1: 補完するモデルと、変えない仮定

ここからはモデル・写しより先に固定した手計算。旧5モデルと旧17 invocation は変更しない。
受入作成時は条件付きkernelとCaseId集合の結合がAgencyのU-SEM-0で止まった。
後の取り込みで修正され、写し採取前にatomic付き本体の手計算を含む新受入が成功した。
以下の数値はその修正や写しから作り直していない。atomicを外した対照の一致だけを本体の検証済みとはしない。
`phase1.relca` は Rule 無しの AsIs/ToBeP/ToBeA、`to-be-a-phase1.relca` は同じ共通宣言の A 単独。
`quantile-compare.relca` は phase1 と同じで、人作業の目的を分位点だけの目的へ置き換える。
`rules.relca` は A/B の静的改定を分離し、`busyday.relca` は旧3構成に負荷を足す。
追加は設計 v0.2 §12 が述べる型・統制・品質・時間・Agency の範囲だけである。

Entry は `record { confidence: enum { high, low } }` だが、全 kernel は
`Entry { confidence = high }` だけを返す。型の値域は2、正質量の戻り値は1で、
confidence に誤りを識別する能力は無い。入力IDは人の enter の正常終了で c1 または c2、
A の extract は人のIDを作らず input_by は none のまま。IDが抽出の正確率を変えることはない。
協調取消 ack_within 1min を通常の非 Ledger Capability に書くが、3通常 process は取消を呼ばない。
したがって本例の終端時間の照合だけで取消応答を実行検査したとは言わない。

## P1-2: requires と台帳の機構

Approver の開始には po/receipt がともに pass、Ledger の開始には approval/po/receipt が
すべて pass を要する。各 requires には明示の拒否値 fail/held を書く（ADR 0026）。
process の if も残すので、通常の post に到達する経路では開始前件は必ず真。
requires の追加で正質量の経路・既存の誤登録確率・17/14/9 を変えない。

Ledger は空の keys から始め、正常終了時の atomic で key in keys を検査する。
一案件で key=case.id=1。通常の1回の post では false の枝で keys に1を挿入し registered=true、
結果 committed、commit1回。重複を乱数で選ばない。
壊した/対照の2回呼出しは、最初の post の直後に同じ引数で再度呼ぶ。
2回目は1分を消費して rejected_duplicate、keys と registered を書き換えず commit を出さない。
posted の時刻は AsIs/P/A の 18/15/10、terminal.work は18/18/13、at_most_once は satisfied。
これはモデル内の同一キーの再呼出しであって、通信障害と再送の意味を実装した証拠ではない。
通常1回版と2回版の at_most_once に mechanism_unmodeled 注記は無い（ADR 0035）。

## P1-3: 品質の分母と結末（Rule 無し）

WrongPosting = reached(registered && !extraction_correct) は評価だけで潜在を読む。
Resolved = reached(end.posted || end.returned)、Posted = reached(end.posted)。
WrongPerArrival は E[count(WrongPosting)] / arrivals。流入は1、拒否・未完了も分母から外さない。
この例ではそれらは0なので、期待件数と比は同じ分数になる（ADR 0045）。

| P1 quality | AsIs | ToBeP | ToBeA |
|---|---|---|---|
| WrongPosting expected_count | 93/125000 | 93/125000 | 231/125000 |
| Resolved expected_count | 1 | 1 | 1 |
| Posted expected_count | 122593/125000 | 122593/125000 | 119481/125000 |
| WrongPerArrival | 93/125000 | 93/125000 | 231/125000 |
| arrivals denominator | 1 | 1 | 1 |
| outcome posted rate | 122593/125000 | 122593/125000 | 119481/125000 |
| outcome returned rate | 2407/125000 | 2407/125000 | 5519/125000 |
| exception/rejected/unfinished/deadlock rate | 0 | 0 | 0 |

全値は exact で、非決定を平均した値ではない。sourceや担当者の名前から品質を推定しない。
登録条件付きの算術は93/122593と231/119481=77/39827だが、今回その追加指標を宣言せず、
WrongPerArrival を条件付き誤り率とは呼ばない。品質が宣言されても価格/事業価値は未設定のまま。

## P1-4: 待ちの区分・前工程・join

一案件で開始待ちは無く、通常の業務Taskが一つ以上走る時間の和集合がリードタイムと一致する。
posted の breakdown は in_service=17/14/9、それ以外（external、contest_route、queue の全4原因、
intentional、unattributed）は0。returned後の異議の1分を終端までの breakdown に混ぜない。
Clerk/Extractor/Approver/Ledger の queue の wait_total、length_max、length_time_average は全部0。
各通常Taskの wait と wait_causes は0。範囲の端を別経路から足した「総待ち」は作らない。

| Task predecessor_wait（分） | AsIs | ToBeP | ToBeA |
|---|---|---|---|
| entry | 0 | 0 | 0 |
| poV | 6 | 6 | 1 |
| receiptV | 9 | 6 | 1 |
| approvalV（作られた経路） | 14 | 11 | 6 |
| posted（作られた経路） | 16 | 13 | 8 |

P/A の checks は poV が3分、receiptV が5分で完了するため、join all の枝待ちは poV=2、receiptV=0。
AsIs に fork/join の行は無い。returned には二照合直後か承認後の2種類があるが、
どちらもすべての通常Taskが順に（または並行に）走り、queue 待ちは0（ADR 0048）。

## P1-5: 宣言した posted の P95（平均・実測ではない）

metric quantile.PostedP95 = quantile(0.95, case.Posted) と、
metric quantile.PostedReachedP95 = quantile(0.95, case.Posted given reached) を分ける。
postedの初到達時刻は各案固定の17/14/9。そこまでの累積質量は0、その時刻で
122593/125000 または119481/125000。どちらも19/20=118750/125000を超える。
従って全流入の左分位点も、到達した案件だけに条件付けた左分位点も17/14/9 exact。
非到達の質量を除いたから全流入の値が得られたのではない（ADR 0049）。

| P1 quantile | AsIs | ToBeP | ToBeA |
|---|---|---|---|
| PostedP95 | 17 | 14 | 9 |
| PostedReachedP95 | 17 | 14 | 9 |

## P1-6: O・L・保守・via の手列挙

Invoice の直接到達は通常 AsIs/P の Clerk[c1,c2]、A の Extractor（人は空）。
異議経路は全案 Clerk.recheck[c1,c2]。Approver の直接原本は空、間接は
Invoice/PurchaseOrder/ReceiptRecord。独立な追加根拠は PurchaseOrder と ReceiptRecord の2源で、
判定者が3でも3と数えない。approvalV の W-LIN-2 は1件、共通原因の呼出しは4。
enter の通常行使は1..1/1..1/0..0、extract は0..0/0..0/1..1。
ToBeA の intent は enter を kept_by_intent と印すが、実際の技能、演習根拠、
縮退経路の依存は unknown/unchecked のまま。行使0を削除候補にしない。

responsible a1 for (registered -> approval == pass) は原本に直接届かないという欠落を残す。
responsible s1 for Extractor.maintenance は保守の責務の宣言であり、s1 の作業を架空の0分にしない。
within と保守Taskを宣言しないので、保守の所要時間と実稼働量は検証しない。
Extractor via model doc-extract@v3 だけがAI寄与の追跡の起点となり、その出力entryから
poV/receiptV/approvalV、postの入力へ読取りをたどる。Actor名からAIと認定しない。
経路の追跡だけで実運用の採用・利用実績やその効果を確かめたことにはしない（ADR0023・0028・0034・0043）。

## P1-7: diff の数え方と hard / 目的

旧diffの8行は actor Extractor、relation Extractor.extract、relation Clerk.enter、
authority Clerk.enter、resource Extractor、time Clerk.match_po、time Clerk.match_receipt、visibility Invoice。
設計 v0.2 §12.6 の提案7件と同数にしない。並列化のtimeはCapabilityごと2行、
入力者のPermissionの喪失はauthorityの独立行。maintainは第9の差分型でなくannotation、
生産者の付替え自体はrelationとして計算しない（ADR 0031 最終追記）。
新P1では責務の宣言があるため責務の射影差が増えうる。件数を旧8へ強制せず対象を列挙して検査する。

hard は WrongPerArrival <= 1/1000。93<125<231 より AsIs/P satisfied、A violated。
AsIs/P の verify は satisfied なので admissible、A は excluded（職務分離のvacuousを救済しない）。
人作業目的は AsIs/P とも全経路範囲14..17で、確実な支配の厳密不等号が無い。
両案を非劣集合に残し、区間が重なる対を未確定とする。
別モデル quantile-compare にだけ PostedP95 の目的を追加すると、Pの上界14<AsIsの下界17、
人作業範囲14..17同士は上界17<=下界14を満たさないので、**両目的を同時に要求した確実支配は成立しない**。
したがってこのモデルの目的は posted P95 **だけ**とし、PがAsIsを確実に支配、非劣集合[P]。
これはC7の目的参照を取り込んで検査する。受入作成時の構文拒否を期待値へ取り込んでいない（ADR 0047・0049）。

## P1-8: 規則改定 B（全件免除の構造上の近似）

rules の ApprovalRule は r2026-04/approval_required=true、ToBeBだけがcfoのRuleChangeで
r2026-05/falseへ改定する。RuleChangeは変更前の版に結び、not_by [a1]。
by a1 の1変更は E-CON-0/invalid/3、RuleChangeを変更後r2026-05でだけ有効にする1変更も
自己授権の E-CON-0/invalid/3。正の改定は静的検査を通る。
Aは通常どおり承認するが入力者enterが無いのでseparation vacuous、root vacuous/5。
Bは二照合後に承認を呼ばずpostへ進む。Ruleはfalseなので開始できるが、approvalはnoneで
registered -> approval==pass の旧約束を破りroot violated/1、A→B diffも1。
低リスク分類による条件免除、免除案件の分布、実行中改定は足さない。

ADR 0052 の実装は、Rule の存在を理由に品質を撤回せず宣言どおり計算する。
Aは231/125000。Bの全件免除は承認の見逃し率を掛けず、
(4/5)(1/50)(1/5)(3/20)+(1/5)(3/20)(1/2)(2/5)=81/12500=0.648%。
正しい抽出477/500を足したpostedは6003/6250、returnedは247/6250。
Resolvedは1、全流入分母は1のまま。
**81/12500は設計 v0.2 §12.5 の条件付き免除 To-Be B の誤登録率ではない。**
設計の免除条件・免除案件の分布は未設定でnot_configured。
ここで計算したのは全件免除という別の構造近似の宣言値であり、同一視しない。
quality.rule行は削除するが、Ruleの版/値/改定/読む箇所とU、既存契約違反は残す。
JSON の出力だけでは全件免除の近似と設計の条件付き免除の区別は出ないため、この説明とモデルを併読する。
実装後に新54写しのうちRuleの3 stdoutを採り直した(採取元とSHA256はREADME)。
これはK3採取時点の3ファイルであり、後続ADR0051のp1-diff stdout/stderrとrule-diffの限定二語を合成した後は、新54中5ファイルが変更対象、他49と旧51写しは不変である。

## P1-9: BusyDay の宣言と拒否の手計算

到着は [0,480) を4分ごと、ceil(480/4)=120件、0,4,...,476。
業務上のhorizon960、計算clock960、metrics window960、1000反復。
drainの宣言上の起点は最終到着476で、期間末480ではない（ADR 0042 最終M2b/M2c）。
Invoiceはper_caseで案件ごとに難度を引く。120件版は旧commitsの台帳で、set CaseIdの機構を足さない。
従って二重登録の機構は未モデルの注記が残る。

元の方策なしの2人の割当を維持する。AsIs/Pは最初のenterをc1/c2のどちらが担うかが未解決、
Aは抽出後の二照合の開始と割当が未解決。有限のseedを引いてもこの選択は確率にならず、
simulate3案とsimulation compareはunsupported/4。到着列とscenarioの記録は検査するが、
完了件数・滞留・平均・P95・稼働率・安定性を実行済みの値としてassertしない。
共通の分析窓は休日/営業時間カレンダーではない。

## P1-10: Mutation の生成と未完了を分ける

旧モデルの10種は6 generated / 4 unsupported。Authority期限0の変異は同じgrantがcontestの
reversible_byにも当たるためinvalid_mutant/static_unsupported、runはinconclusive/mutant_incomplete。
期限切れを検出したとは数えない。同じ要約の多重利用は生成され、系譜源集合を増やさないことを検査する。

新単独AはWrongPerArrivalを宣言するので種7も生成され、残るunsupportedは
command_resent/command_resend_unavailable、stale_definition_write/rule_version_unavailable、
receipt_unknown/receipt_unknown_semantics_unavailable の3つ。
種7の全流入分母から除く定義の候補はposted/returned/exceptionの3終端。
各候補の定義の差をquality_denominator/comparison_mismatchで構造的に検出し、
変異体の比を再解析した値や改善率は出さない。通常の変異の未完了を種7で成功へ救済しない。
mutate全体はunsupported/4、run未完了を独立に残す（ADR 0046・0047）。

## P1-11: C8 レビュー後の独立受入（写しを読む前の期待）

裁定の各項目を下の手順で補う。新しいモデル・invocation・写しは足さない。
数値の期待は上の積・和・経路から導き、出力は欄の形を知るためだけに読む。

### F1: 単独 A、explain、simulation、目的セル

単独 A も同じ一案件なので、誤りは (4/5)(1/50)(1/5)(3/20)(1/10)
+ (1/5)(3/20)(1/2)(2/5)(3/10) = 231/125000。
正しい抽出477/500を足すとposted119481/125000、補数がreturned5519/125000。
経過1+max(3,5)+2+1=9、作業1+3+5+2+1=12。posted質量が19/20を超えるのでP95=9。
explainでも同じで、probability reached #1 は probabilistic_property として省略し、
counterexamplesは空。確率違反から一つの原因経路を作らない。
人作業の目的セルは、照合で戻る経路の14/8から、承認後の異議で1分増える17/11まで。
従って AsIs/P は14..17、Aは8..11、basis=path_range。
simulationでは開始者と開始順が未解決なのでU-SEM-0 successor actions、unsupported。
単独A・3案compare・BusyDayの3案compareそれぞれを直接検査する。
品質の推定はaborted、pointはnull、trials/samples/observationsは0。BusyDayは品質未宣言なので
品質自体を足さず、scenario名BusyDay・120到着と拒否を各案で照合する。
負の対照はWrongPerArrivalの一セル、架空の確率反例、目的セル、1案だけの標本値の各改竄。

### F2: Rule の通常品質と旧約束（ADR 0052 による移管）

宣言した性質の閉集合はWrongPosting/Resolved、cohortはWrongPerArrivalのみ。
結末はend.exception/end.posted/end.returnedとrejected/unfinished/deadlock。
WrongPostingのexpected_count・案件min/max・比のlower/upperはA=231/125000、B=81/12500。
Resolvedは1、reason=null、basis=exact。結末率は上のposted/returnedと他0。
arrivals分母count=1、案件ID1/arrival0、分子・定義の宣言と順序を保持する。
Aは承認してから登録するのでInvoiceControls #1 invariant satisfied、#2 satisfied、
#5 separation vacuous。Bはapproval=noneのまま登録できるので#1 violated、#2 satisfied。
負の対照はlower=0、各品質群の数値改竄・欠落、契約#1のsatisfiedへの偽装。

### F3: 差分の閉じた手列挙

P1のAsIs→Aでは通常callのenterがextractへ変わる。保守責務はextractに適用される。
Clerkは照合/異議に残る。source読取り・逐次/並行とADR0051の同名entryの生産者付替えを含めると次の10行。

| kind | target | change |
|---|---|---|
| actor | Extractor | added |
| relation | Extractor.extract | added |
| relation | Clerk.enter | removed |
| authority | Clerk.enter | removed |
| authority | Extractor.extract | added |
| resource | Extractor | added |
| time | Clerk.match_po | modified |
| time | Clerk.match_receipt | modified |
| visibility | Invoice | modified |
| relation | entry | modified (Clerk.enter → Extractor.extract) |

actor Extractorのmaintainはperson=s1、linkはExtractor、to_dutiesの保守責務ID2/s1へlinked。
enterのretireはnormal_pathから外しkeep fallback=true。単なる配列の存在では足りない。
entry行は既存全種類の後へ追加し、intent/注記が無いのでW-DIFF-1/W-DIFF-3が各1増える。完了diffにはrelation_state_producersとrelation_binding_identityの順の限定二語が付く。共有stateの書込み因果や改名対応を計算したとはしない。
Rule A→Bは承認callを外し、他のcallの逐次/並行と原本直接読取りは変えない。次の5行。

| kind | target | change |
|---|---|---|
| actor | Approver | removed |
| relation | Approver.approve | removed |
| authority | Approver.approve | removed |
| resource | Approver | removed |
| rule | ApprovalRule | modified |

Ruleのfromはr2026-04/true、toはr2026-05/false、amendmentはby cfo/under RuleChange。
負の対照は空配列、maintain欠落・s1リンクの破損、retireのkeep欠落、Rule値・授権の改竄。

### F4・minor 4: 開始前件で実際に止める対

inline一案件はapproval=none、po=receipt=pass、keys空でLedger.post(case.id)を直接呼ぶ。
requiresありならheld、開始0回・作業0・registered=false・commit無し、旧承認invariant vacuous。
requiresだけを除くと1分/作業1、空集合への挿入からcommitted・registered=true、invariant violated。
requires宣言が無くなる側はcapability_refusals欄そのものを出さない。宣言ありの空配列とは区別する。
Rule Bのamendだけを除く対もapproval_required=trueのままなのでpostはheld。
二照合pass経路はextract1+max(3,5)=6分、work1+3+5=9でexceptionへ行き登録しない。
amendありのBはpostの1分を足して7分/work10でposted、旧承認invariant violated。
実行する対ではchange ruleのintentを両側から除く（改定無し側だけに未実在の主張を残さない）。
この同一の下地で、対の差はamendだけ。Bの既存の静的contestはactiveな承認者を欠きviolatedなので、
amend無し側のrootもviolated/1（承認invariant自身はvacuous）となる。
初稿のsatisfiedは受入の誤りとして訂正した。ADR 0037の畳み方は、registeredという前件が
一度も真でなければvacuousと明記する。通過の合格へ数えてはいけない。
負の対照はheld側にcommitを偽装、時間/作業を偽装、amend無し側にpostedを偽装。

### F5・minor 7: AST 投影で許す差を閉じる

phase1/単独A/quantileは既存の一致に加え、rulesの五kernelと共通宣言もASTで比較する。
rulesで許す差: ApprovalRule/RuleChangeの追加、Ledgerの引数（keyのみ）・requires（Rule条件）・
reads（po/receipt追加）、Cutのquestion/include/LowRiskExemption defer、simulation/確率verifyの無し、
Posted性質/分位点2行/compareの無し、AのLedger実引数、B processとamend configuration。
requires/atomic/cancel/via・数値表・共通Actor・状態・責務は黙って除かない。
BusyDayは旧configurationsを基準に、Invoice per_case、question、Cut horizon2行、window60→960、
一案件boundsとsimulationをscenario（4分ごと120件/queue200/horizon960/1000反復seed41020）へ移す差だけ。
通常3process、五kernel、旧enum、commits台帳は同じ。各入力の確率1値を変えると一致helperはErr。
BusyDayのkernelはさらにP1のrecord結果highを旧enum highへだけ投影して直接一致させ、
五モデル間の確率表の共通部分をつなぐ。確率・given・writesはこの投影でも変更しない。

### minor 2・3・5・6: 追加の境界

二回postはP1-2の18/15/10分、work18/18/13、commit高々1と注記無しを三案で照合する。
共通原因はenterまたはextract、po、receipt、approvalの4呼出し。1つを欠落したコピーを拒否する。
not_by単独の対はRuleChangeにgrants cfo,a1としby a1にする。not_by [a1]を残せば
変更前版でgrantされてもE-CON-0/invalid3、not_byだけを除けば改定は通りBの旧約束違反/exit1。
整数の対はobserved n:0..3=0を持ち、given flag=true/falseともpass確率1の1分callを使う。
整数は通常型表の添字でない。条件付きkernelあり/無しの両方でsatisfied0、elapsed/work1、agencyあり。
agencyを消したコピーをhelperへ与えてErrとし、集合の場合だけの受入で代用しない。
READMEはI12.6-708を認定でなくviaの追跡と明記し、集合だけでなく有界整数の防御も説明する。

すべての負の対照は実出力を照合する同じhelperのResult::Errを要求する。
ここに全出力の全欄の意味を守ったとは書かない。独立参照の直接実行、production故障注入は別の義務。
