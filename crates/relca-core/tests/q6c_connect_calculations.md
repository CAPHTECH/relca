# Q6c 接続の手計算と閉じた契約

基点165b7804。ADR0064の8・9節とQ6cの裁定・補足1から、実行前に決めた期待。
実出力・snapshot・goldenは意味期待に使わない。分は整数、確率は有理数。
実技能、実負荷の測定、保持根拠、SLAを認定しない。

## 保存と意図した変更

既存のduration、開始順、qualityの分母、旧retention.normalのprocess全体の意味を保存する。
Q6a/bで実行済みの意味をcompare/diffとwork_profilesだけのsensitivityへ接続する。
mutate/conform/contest/S1/retryとdegradeのsensitivityは開放しない。
未導出を空の完全表へ変換しない。個人の成績で所要時間を推測しない。

## 模型と算術

- W: 旧Desk.legacyと新Desk.runはともにduration2。新runの資格はqだけ。
  Pはqueued_same_actor_at_start、qの[0,1)でLow、[1,infinity)でHalf。
  到着1件で他Queuedは0なのでLowだけを使い、bad=1/10、仕事2、elapsed2。
  legacyもLowを使うので確率・時間は同じ。Halfは未選択でも実効selector表の一部として記録する。
- W2: 旧側ではなくPを使うrunから、境界2のQを使うDesk.alternateへ変える。
  Qは[0,2)/[2,infinity)。q=0でLow、bad=1/10と時間2は変わらないが構成の帯は異なる。
- S: Wのrunだけの模型。kernel確率だけLowの1/10を1/4にすればbad=1/4。
  kernel値だけLowのbad枝のbad=trueをfalseにすればbad=0。
  profile/kernelのbasisだけ変えてもbad=1/10。固定durationは全点2。
  given、全branchのresult/writes、確率0のbranchも宣言として記録する。
  資格q→p（表のexecutorもpへ）、境界1→2（全帯を連続に変更）、load queued→noneは
  正当な別模型だが同一感度の契約ではない。問い/processの変更も拒否する。
  資格[q,p]→[p,q]の順だけは同じ集合で、actorのfirst_idle順を変えない。
  given-finishの既存Pはanonymous/load none、flag=false行が0:hit=trueと1:hit=false、
  flag=true行が1/2ずつ。条件・0質量枝・書込先も省略しない。
  Q7の表現規約に従いenumのbool値は文字列true/false、EffectiveInputsの変数は
  case.observed.flag/case.observed.hit、uncertaintyではflag/hitの既存規約を保つ。
- D: Q6b A7を使う旧Ifと新degrade Manual。Probe1の後、通常Fast1で2、縮退Clerk3で4。
  common_prefixのProbeは1..1。通常Fastと縮退Clerkは各側のmay/must、各1..1。
  Oは通常Summary、縮退Doc。旧Ifにはdegradation表が無いので側別比較はnot_comparable。
  旧retention.normalはprocess全体で、Clerk/Fastは0..1、Probeは1..1のまま。
- D変更: ClerkをOther2へ置換すると縮退は3、Clerk0..0、Other1..1、DocのOは空。
  requiresを持つClerk側はnot_derived/requires。derivedとの差を差なしにしない。
- C: coverage(0,false)ではProbe1→Fast1→Finish2=4、縮退は1+3+2=6。
  到着9では通常9+1+1+2=13、縮退9+1+3+2=15。
  through11の半開calendarの外でStartが必要なので両側not_derived/calendar。
  prefixのProbeは9で開始できるので1..1のderivedを保つ。
- I19: 元の旧模型が各modeで実行できることを先に確認する。資格だけ/成績だけを足すと
  q6_feature_combination/U-SEM-0/unsupported/exit4。
  比較でも片側または両側にS1/retryを置いたものを拒否する。
  下流の別エラーや旧retry_feature_combinationだけでは代用しない。
  retryの旧対照には未接続の開始policyを入れない（旧Q1とQ6aの既存規約）。
  比較のS1は両選択に共通する流入宣言、retryは一方のprocessだけ/両方に制御を置く。
  比較を拒否する際、I19を含まない側まで拒否することは要求しない。

## 採用したwire（orch補足1による具体化）

既存の型付き構成差分の形を延長する。未知の鍵を拒否する。
diff.differencesとcompare.tables.configuration[].differencesで同じ行を使う。
compare.tables.configurationは全unordered plan pairを要求順で一度ずつ列挙する。
pairの鍵はfrom/to/differencesだけ。from/toはconfiguration名。
新kindはqualification/performance/degradation。行の鍵はkind/target/change/reason/loses/from/to。
annotationsは既存規約どおり空なら省略。reason/losesはこの模型ではnull。
selected processで字句上使うCapability/routeの集合を比較し、未使用宣言は差にしない。

| kind | target | from/toの型 |
|---|---|---|
| qualification | Actor.capability | 資格PersonIdの正規順配列、非使用/宣言無しはnull |
| performance | Actor.capability | work_profilesのProfileと同じ閉じたobject、非使用はnull |
| degradation | route名 | binding/valueを持つconditionとproducerの閉じたobject、無しはnull |

帯の値はprofile.rowsのexecutor/lower/upper/kernelで明示する。新しい期待時間欄は作らない。
performanceのduration_effectはnot_supported。上の選択使用差は宣言表全体の差ではない。

Agency差分は既存agency-diff-p1-1のMeaningRowを延長し、viewpoint=degradation、
item=normalまたはdegraded、call=route名をidentityに使う。
derived同士の値は新表のside object全体（status/reason/ends/observation）。
一方が表無し/未導出ならstatus=not_comparable、from/toは既存のstatus/reason object。
reasonには側と表を含める。語彙は次のとおり。

| 状況 | reason |
|---|---|
| 旧from側に表無し | from.agency.degradation:null |
| 旧to側に表無し | to.agency.degradation:null |
| from側の側別表未導出 | from.agency.degradation.<route>.<side>:<元reason> |
| to側の側別表未導出 | to.agency.degradation.<route>.<side>:<元reason> |
| 既知の側 | null（status=derived） |

既存retention.normalのscopeはwhole_process_paths、新dependencyはstructural_guard_paths。
新表の無い旧側へscopeを捏造しない。既存assumptionsのscope行を照合する。

## sensitivity実効入力の閉じた追加

Q7 EffectiveInputs.capabilitiesの既存kernelは通常kernel専用として保つ。
成績源ではperformanceを追加し、単にkernel:nullで済ませない。
performanceの鍵はprofile/basis/load/snapshot/duration_effect/rows。
rowsの鍵はexecutor/lower/upper/kernel。kernelはQ7のKernelそのもの
（name/basis/given/rows、行conditions/branches、枝probability/result/writes）。
従って実効kernelへの経路はperformance.rows[].kernelであり、全selector行に非null。
通常kernelとperformanceは排他。旧模型にはperformanceを捏造しない。
Q7 uncertainty.aleatoryにはCapabilityとkernel名ごとに全実効kernelを一度ずつ記録する。
kernelのspan以外は手入力の全条件・値・有理数と完全一致。
model.itemsにはwork_profilesへの参照、estimationは網羅ではnot_applicableのまま。
感度の問い変更の拒否は既存sensitivity_contract_mismatch（E-CMP-0、invalid/exit3）、未接続degradeは
sensitivity_feature_combinationの外側理由とq6_feature_combinationの内側理由を分ける。

未接続modeの診断は集合全体を照合する。各拒否の診断はU-SEM-0/errorかつ
q6_feature_combinationの1件だけ。mutateではoriginalに1件、rootは空。
conformはreason欄もq6_feature_combinationであることを要求する。
追加のU-LEDGER-0等を許す集合にはしない。spanは診断の位置であって理由ではないため、
その数値を別のエラーの代用にはしない。
degradeの感度拒否の子診断はQ7のpoint prefixとcontract projection prefixを含む
既存の完全なmessageまで固定する。外側理由はsensitivity_feature_combination、
内側理由はq6_feature_combination: degradation sensitivity not connected。

## record私有seam

recordの正例はOk(Some(完全表))、新宣言無しはOk(None)。新宣言があるのにparse失敗、
actor/person/profile/kernelの範囲外、sourceのexecutor集合とIRの不一致はErr。
正当なDSLはlowerと両入口のvalidatorが先に拒否するか、整合するIRを作るため、
この防御契約は公開IRを一箇所ずつ壊して直接recordへ渡す私有seamで固定する。
エラー文字列は自由だが空は禁止。Ok(None)やOk(不完全表)は不可。
正常なparse/lowerでは資格の人は既知かつActor内、profile/kernelの参照は有効であり、
sourceとIRのexecutorは同じ宣言から作る。従って意図した不整合は正規DSLでは作らず、
IRのactor/qualified/permission/profile/row-person/row-kernel/source-executorsの7単一破損と
source parse失敗をseamへ渡す。試験用に公開APIを広げない。

## 検査範囲

新core/CLI targetと新seamだけを実行する。古いgoldenや全workspaceは実行しない。
CLIの旧模型stdout/exitは同じsource/selectionの公開core入口の直列化と完全一致比較する。
Q6c補足2でこの対が承認された。旧模型にはQ6理由語が無く、手値のverdict/exitも要求する。
旧mutateのrootは未対応種を残すunsupported/4、originalはsatisfied。
conform/contest/S1/retryの旧対はsatisfied/0である。
これは現在の二入口の保存検査で、旧binaryとの歴史的byte互換を復活させない。
新意味の正しさは上の手値で別に固定する。
