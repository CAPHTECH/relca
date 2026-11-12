# ADR0047 × ADR0048 — 実行前の手計算 (段階C)

期待はcollector/compareを実行する前にここへ固定する。既存116/43/79名は増やさず、
交差入力はsupport内のinline DSLで持つ。待ちはB7の五つのoverall目的でない。

## W: 通常のFIFOと両mode

A capacity1、work2分、到着[0,0]、FIFO case_id、両案は別processだが同じ本文。
case1はwork[0,2)、intentional[2,3)、End3。
case2はqueue[0,2)、work[2,4)、intentional[4,5)、End5。
service=2/2、intentional=1/1、actor_capacity=0/2、他のbreakdown=0。
Task wait=0/2、predecessor=0/0。Actor queue積分2、max1、時間平均2/5。
overall work4、peak1、named_people_workは人未宣言なので既存の境界を保つ。
Doneの各case確率1、expected_count2、DoneRate=2/2=1。
hard DoneRate>=1/2は網羅でS。work最小化は4対4、両案admissible、
sure dominanceなし、Pareto両案。標本はR4/confidence1/2/m4(2案×2case)でalpha=1/16、
成功4/4のCP下端(1/16)^(1/4)=1/2、上端1、hardは両案S。
標本のwork=4、Paretoはsimulation_estimatesで未計算。
初稿のR3/m2はcase数を落とした受入の誤り。R3ならm4、下端は(1/16)^(1/3)で
1/2未満、hardはIになる。第二回でこれを検出し、次の実行前にWのRを4へ変更した。

checkedの順 (網羅): configuration_selection,cut_compatibility,terminal_compatibility,
property_tables,terminal_tables,metric_tables,hard_filter,admissibility,pareto_sure_dominance。
標本は先頭3語の後にsimulation_clause,random_key_separation、末尾評価語は
hard_filter,admissibility,hard_cross_plan_bonferroni。
uncheckedの順 (網羅): ranking,weighted_score,differences,difference_intervals,
cross_plan_confidence,operation_coupling,terminal_meaning,
metric_differences,metric_ranking,metric_semantic_alignment,waiting_tables。
標本はrankingの後にparetoを残す。待ち無し対照は末尾waiting_tablesだけを外し、
配列全体をbyte文字列でも固定する。旧compare(no declaration)はhard_filterとparetoを
rankingの直後に戻す。waiting_tablesは宣言時だけ、ちょうど一度、既存末尾の後。

埋込みReportは同じ入力の単独コマンドと全欄一致。quality表は全行/全案/値/分母を照合。
metrics表は待ち宣言を外した同幅対照と同じkey/valueで、新しいqueue、dwell、
breakdown、wait_causes、predecessor_wait、join_wait、queue_wait_observedは出ない。
新しい待ちの数値は埋込みReportだけで読む。品質4行→metrics.waiting→
probability.schedulerの順、拒否でmetrics無しならmetrics.waiting無し。

## H: B6の手計算を待ち付きで再利用

既存cmp7-clockのDay30/14を使う(到着0,6,12、check4/post1、Wrong1/50)。
30ならWrongPerArrival=1/50、14なら最初2件Wrong期待1/25と第三の未知[0,1]から
[1/75,26/75]、ratio値はnull/undetermined。hard<=3/10はI、admissibility undetermined。
同じ14をclockへ移す対はtruncatedであり業務Cutと同じにしない。
CutOff表は網羅で宣言horizonから3case×2planの矩形、未観測を成功へ変えない。
全品質表は既存の独立horizon14 oracleを再利用し、待ち宣言で一行も変えない。

標本の地平対は既存D02の別process Good/Bad、両方正しいcheck3+post1を使う。
到着1件、R3、confidence1/2、horizon1/5。1ならWrong hits0/unknown3でCP[0,1]、
5ならhits0/unknown0でCP[0,1/2]。CutOff矩形は1case×2plan。
初回にcmp7-clock(標本句なし・両案同じprocess)を標本へ流した受入の誤りを訂正。
queueの閉じた欄はactor_capacity/resource/executor/combinedで、unattributedは
breakdown直下。初回のqueue.unattributed参照とterminating end.doneは受入の誤り。

## 負対照

Wのwork2→3という唯一の入力故障: End4/7、queue3、service3、work6、平均3/7。
旧2/5の手oracleは拒む。観測を期待へ写して直さない。
待ち目的はqueue_wait_observed/actor queueのwait_total,length_max,length_time_average/
terminal breakdown/task wait_causes/predecessor/joins/dwell等すべてunsupported U-SYN-0。
metrics.overall.work等五つの既存目的は従来のまま。hardもcohort以外の待ち参照は拒否。
wireのwaiting_tables欠落・重複・順序逆転、waits数値/any/dwell/lowerの改竄は
正常wireのコピーに一箇所だけ故障を入れて拒む。productionの意味はここで直さない。

五つのoverall目的の待ち併用対照は既存cmp7-overallの手oracleをそのまま使う。
直列3+5と並列max(3,5): work8/8、名前付き人0/0、peak1/2、
cost100×8+2+3=805/805、1到着なのでper_arrivalも805/805。
最小peakだけが厳密差で、AsIsだけsure dominance。並列join allのaは5-3=2待つ。

待ち無しの全byte対照は、d239a521のsrcを一時ディレクトリへ複製し
offlineでbuildしたPRE-A7 B7 binaryから4jobだけ固定する。新binaryを旧byteの
出典にしない。手計算の値/配列はこのmdとRustに別にあり、byte記録で導出しない。

## 旧A7×C6受入のB7追従

HQの字句Endはdoneだけなのでkind7はnot_applicable/no_matching_site、DoneRateの
skipped1、mutants0。SとWの字句順はrejected,doneなのでDoneRateに2候補、各detected、
report:null。待ち有無の同幅対照と同じ2行であり、実行の成功判定には使わない。
通常六種のhold/未完了/割当て無しとrunの分類は変更しない。
初回既存A7の6REDはkind7をunsupportedと固定した旧受入によるものだった。
