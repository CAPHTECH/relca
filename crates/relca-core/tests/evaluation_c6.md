# ADR 0046 × ADR 0047 — 取り込みの手計算 (Stage C)

期待は実行前に固定する。既存116/79の閉じた群は増減しない。入力はinline。
根拠は ADR 0046 の処理順・未完了分類の追記と ADR 0047 の種7の定義。

## D03: Dayと予算

eval-mut7の流入0,10、bounds2/2/30をDayへ移す。checkはposted4,14または
returned3,13、Wrong率1/50、Registered率23/25、Resolved率1。
readsもAuthorityもproducer/consumerも無い。生成順[0,2,8,9,1,4,7]のうち
先行する候補はholdとrefusalのguard各1、合計2。表示は従来の10種の順。
種7はArrivalのreturned/postedとResolvedのposted/returned、計4。
Registeredは状態条件でskipped1、予算を使わない。

| limit | ordinary実行 | 種7 detected | 種7 not_run | quality_denominator checked | run |
|---|---:|---:|---:|---|---|
| 6 | 2 | 4 | 0 | true | inconclusive/mutant_incomplete |
| 5 | 2 | 3 | 1 | false | inconclusive/mutant_limit |
| 2 | 2 | 0 | 4 | false | inconclusive/mutant_limit |
| 0 | 0 | 0 | 4 | false | inconclusive/mutant_limit |

種7はreport=null、比較2行のみ。not_runはreason mutant_limit、report=null、
assigned/otherは空。通常の変異体はDayと流入を保持。selector無しはoriginal
invalid E-CONF-0、run blocked、6種blocked、種7は旧unsupportedのまま。
quality_changesは常にunchecked。種7だけで通常品質の変化を比較しない。

runの検算: 初稿はlimit6のrunをcompleteと
していたが、guard削除はreturnedの唯一の字句終端を消す。Resolvedがそれを
参照するので再lowerのE-NAME-0、2体ともinvalid_mutant/static_invalid。
PRE-bcd69f38と現binaryで各1回同じ分類を観測した。種7のfinish数とは独立に
runはinconclusive/mutant_incomplete。これは受入の誤りでproduction REDでない。
静的失敗Reportにscenarioは無い。正常再実行のsummaryでは宣言metadataを保持し、
aggregationの観測期間は複製によって3..4→4..5(打切り4なら4..4)へ変わる。

## Summaryと未完了

mut2-summaryは呼出しe,p,a,post各1分、done4/rejected3。判定はp,aの2つ。
複製はe_dupを1つ加え、done5/rejected4、系譜はInvoice/PurchaseOrder不変、
groupの判定数2→2 (eはguardに使わない)。assignedの3行はheld。
手で挿入した複製をengineと独立参照へ別々に渡す。DoneRateは非決定の
区間[0,1]、horizon4で理由undetermined、clock4でtruncated、全完了では
値[0,1]のper_case_relaxation。非決定を一様確率へ変換しない。
Day horizon4なら元は完了だが複製はdoneが打切り、truncated=false。
summary行はinconclusive/assigned_check_unavailable、run inconclusive。
clock4/horizon10なら複製は計算切断、reason truncated、run inconclusive。
cohort Done/arrivalsを足すと種7はdone/rejectedの2候補。これらの構造比較は
実行しないのでdetectedのまま。普通の行の未完了をこれでcompleteへ改善しない。
horizon5の対照は複製も完了、summary detected、run complete。
horizon2は元のsummary自身が未完了、original inconclusiveで元のgateが停止。
六種はblocked、種7は旧unsupported、変異の試行は全種0。未完了の元を
種7の構造比較だけで完了や成功へ変換しない。

## 六種の共存、評価、表

既存C6モデルに品質を足しただけの非cohort対照は種7unsupported。
cohortを足した側だけ種7生成。六種の候補ID・順・通常Report・scenarioは
PRE-bcd69f38の4362job互換台帳で固定する。summaryの手の2→2も別途検査。
hard/目的/Pareto・CutOff矩形・案間horizon防御は既存evaluation_integration
と私有evaluation_testsの手の期待を再実行し、C6投入後も保持する。
kind7のreport:nullを旧六種へ広げない。閉じたwireはexpiry/summary演算子と
kind7を同時に許し、reportの欠落、未知演算子、heldの種違いを拒む。
SummaryのApproverをexecutors[p]としGrantのauthority_basisを足す混在入力:
元はGrantを保持、期限0変異はapprove終了3>0でviolated/detected。summaryは
同じ2→2/held、種7はdone/rejectedの2。品質はother_changesの閉じた欄の外。

## 将来の義務

A7(D05)とC7(D06)は今回の取り込み対象でない。待ちのuncheckedの順、分位点
の目的の移行はdeferredのまま。通常品質をRuleで撤回する現在の境界も変えない。

## 旧C6 T24の期待の移行

全段check初回が、C6のmutation_integrationのcohort付き品質入力でも種7を
unsupportedに固定した期待で停止した。ADR0047が明示的に閉じたC6 T24なので
productionの欠陥でなく受入の追従漏れ。元のTimely=done within4、比Rate/arrivals、
通常summaryの品質1→0とother_changesにqualityを入れない期待は保持する。
種7だけは字句終端rejected,doneの2候補でdetected/report:null/other空。
常時passでも字句在庫を実際の到達終端だけへ縮めない。coreとCLIを同じ手値へ直す。
