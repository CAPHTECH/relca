# ADR 0060 U8前半の独立受入と手計算

採用済みU1/U4〜U8の前半。実装出力から期待を採取しない。
per_runとduration givenのengine/referenceはQ4後の別受入であり、ここでは書かない。
設計 v0.1 §12、設計 v0.2 §16、ADR0060のA1/A6/A7/A8/A9に対応する。

## 入力と手値

tests/data/sensitivityが入力の正本。全モデルは旧DSLだけで有限、明示の確率和1。
base/high/missは同じ問い、1分の単一Task、容量1、同じ観測方策とCut。

| 点 | q=誤入力 | m=見逃し | q×m | accepted=(1-q)+qm | rejected=q(1-m) |
|---|---|---|---|---|---|
| base | 1/50 | 1/5 | 1/250 | 123/125 | 2/125 |
| high | 1/25 | 1/5 | 1/125 | 121/125 | 4/125 |
| miss | 1/50 | 1/4 | 1/200 | 197/200 | 3/200 |

values.jsonの範囲はこの三点だけ。q=1/25,m=1/4の組1/100は未入力。
三点の探索確率extremaは1/250..1/125で、連続領域や独立区間の直積ではない。
各モデルの確率制約の閾値1/125はすべて満たす。所要/作業はすべて1分。
expected.jsonのsource/manifest SHA256は入力bytesへの直接のchecksumであり、実装Reportからの採取ではない。
CLIはこの固定hashにも照合し、親子が同じ架空hashを繰り返すだけの自己整合で済ませない。
clockを0にした点はinconclusive/truncatedで、extremaはnull/incomplete_points。
確率表の補数を削った一差分はE-PROB-0/invalidで、成功点のprefixだけを実行しない。

p0/phalf/p1は同じkernelの独立な二回抽選。事故は一回目yesかつ二回目no。
p(1-p)は0、1/4、0。endpoints.jsonは0と1の二点だけなので探索点最大0。
robust_guaranteeとcontinuous_region_checkedはfalseのまま。中点を明示すると最大1/4。

zeroは0質量のhit=true、1質量のhit=false。どの反復でもhits=0、n=4。
信頼水準7/8。予定行M=1なら片側alpha=1/16、CP上側は1-(1/16)の4乗根=1/2。
M=2なら片側alpha=1/32、上側1-(1/32)の4乗根を2の20乗格子で外向き丸めし607705/1048576。
閾値11/20では、(1-11/20)^4=6561/160000。
これは1/16以下で1/32より大きい。従って単独はsatisfied/0、二点suiteはinconclusive/2。
同じsourceの二つの宣言点も重複排除しない。M=2は予定点から数える。
子CIだけを広げ、元のsatisfiedを残す誤りも検出する。

zero-two-casesは0分と2分に2案件、各4反復でhitは常に0。
二点suiteならM=2case×1query×2point=4、alpha=1/64。
各caseの標本数は4であって8でない。上側格子は677849/1048576。
同一反復の案件を独立にプールした偽造wireも別の対として拒否する。

abortはADR0016のSHA256鍵にseed=0、rep=1,2,3を代入する独立な参照計算で、
A.cのresultはsecond,second,firstとなる。三反復目だけB.cの未解決非決定に到達する。
attempted3/completed2/unsupported。信頼水準の変更でdrawは変わらない。
abort-pairの予定Mは2query×1case×2point=4、片側alpha=1/64。
標本途中拒否で点数・行数・要求反復数を減らさない。部分標本から新しいCIを出さない。

## 三区分の固定部分と観測範囲

uncertaintyはrules/aleatory/estimation/modelの閉じた四欄。
各分類の閉じた鍵はstatus/reason/items。availableのreasonはnull、それ以外は機械的理由。
availableのitemsは空でない。referencesは各子の該当分類への具体pointer文字列の配列。
leafのitemの全payloadを本受入で網羅固定したとはしない。必須のkindと出所の参照/inlineの存在、
既存Cut/仮定/統計の接続、既知空と未検査を分ける分類を検査する。
モデル全体の不確実性無しを表すstatusは無い。

| 入力/入口 | aleatory | estimation | model |
|---|---|---|---|
| base check | available/null | not_applicable/exhaustive_analysis | available/null |
| serial-time check | not_configured/no_probabilistic_model | not_applicable/exhaustive_analysis | available/null |
| zero simulate | available/null | available/null | available/null |
| 入力未読verify | unavailable/input_unavailable | not_applicable/no_analysis | unavailable/input_unavailable |
| 構文不正 | unavailable/not_lowered | not_applicable/exhaustive_analysis | unavailable/not_lowered |
| conform | not_applicable/observable_trace_only | not_applicable/observable_trace_only | available/null |
| diff/compare/mutate/sensitivity root | references/aggregate_per_source | references/aggregate_per_source | references/aggregate_per_source |

verifyはsourceを読まない。explainはflattenされたleafだけ。子のhash/seed/Cutを先頭点で代表しない。
verify受入の実際の観測は既知path/不在pathでのraw出力の一致と分類であり、I/O呼出し回数のspyではない。
decodeの壊した対は実Reportをserializeしたcontrolから三欄の鍵・status/reason・参照を一箇所だけ変更する。
これは真正性や改竄防止の証明ではなくschema/自己整合性の検査。

## runner wireの受入で固定する部分

root.schema=relca-sensitivity-result/v1、mode=sensitivity_exhaustive/sensitivity_simulation。
scopeはkind=enumerated_models、point_ids、manifest_sha256、planned/evaluated/refused/not_run、
all_points_evaluated、continuous_region_checked=false、robust_guarantee=false。
予定数は三分類の和、evaluatedは分析器を呼んだ点であり完全探索の印ではない。
pointsのidは宣言順で一意、statusはevaluated/refused/not_run、子Reportはreport。
各source_sha256は実入力bytesのhash。拒否でも読めたhashを保持し、未読hashはnull。
summariesは配列でid=targetのid。exhaustiveの既知量はkind=finite_point_extrema、
status=available、reason=null、value={min,max}、point_ids=予定id。
不完全時はstatus=unavailable、reason=incomplete_points、value=null。
simulationではCIをfinite_point_extremaへ混ぜず、scalar集計value=null、reason=simulation_pointwise。
点別の統計値は子Reportで検査する。
これはADRの抽象的な欄名を受入で具体化したもので、独立に期待を書いた固定表である。

## 必須の五つの壊した対

1. valuesに未入力のhigh×miss点を追加した手組みwireは予定id/件数の完全一致で拒否。
2. endpointsのrobust_guaranteeをtrue、またはsummary.kindをworst_caseへ変えたwireを拒否。
3. zero-pairの片側alphaを1/32から1/16に変え、z1を未実行にしたwireを拒否。
   実runnerのabort-pairも予定M=4を検査する。
4. baseのkernel行から0.80:failだけを削った入力はinvalid/E-PROB-0。暗黙補数で通さない。
5. 点257、manifest1MiB+1、source4MiB+1、合計source68MiBの各入力をunsupported/4にする。
   successfulなprefixのchild ledgerを出してはならない。上限ちょうどのpoint256/source4MiBも対照にする。

手組みwireの校正はtest-onlyの独立validatorに対するGREEN。
productionの受入が通ったこと、production故障注入を実施したこととは区別する。
壊した入力のproduction拒否、手値、core/reference、実CLI stdout/stderr/exitは別々の試験。
現行の未知commandによるinvalid/cli_refusalはrunnerの適切な拒否とは数えずREDにする。

## 不変と残る範囲

旧DSLの手値・verdict/exit、serial-timeの8分/work8、clock切断、invalid確率和、
通常simulateのM=1/alpha=1/16/upper=1/2と既存SHA鍵をGREENの基盤として固定する。
raw出力全欄の歴史互換は要求せず、既存snapshotは後続の全段で実行する。
この回は全段・snapshot更新を行わない。per_run/duration givenのengine/reference受入は作らない。
