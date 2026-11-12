# ADR 0062 受入の手計算と接続境界

期待は契約・区間・有限な全順序から先に書いた。productionの新出力をコピーしていない。入力の正本はtests/support/q5_inputs.rs、数値と選択wireの正本はtests/support/q5_oracle.rs。実際の試験結果とsha256には別の実行証拠が必要である。

## 入力と観測の形

43 runtime模型、13 static模型、12個の宣言された厳密な一編集対。名前は閉じた定数列である。CLIは実binaryを起動し、stdout JSONの対象pathと実exitを検査する。stderrはそのJSONの全diagnosticsの表示と一致させる。Q7のuncertaintyなど無関係なroot欄は許容し、全出力byte比較にはしない。

全runtimeのpriorityを除いたsubstrateをparse/lowerする検査は、無関係な文法や型の誤りを先に除くためであり、新しい意味の成功を表さない。substrateは優先度を外して旧FIFOへ置換した明示の対照である。これは新しい期待値を旧実装から採取するoracleではない。

## P01〜P16

| ID / 入力 | 区間・手値・観測 |
|---|---|
| P01 start | 到着0/0/1、rank0/10/20。A容量1/所要2。case2=[0,2)、case3=[2,4)、case1=[4,6)。CaseId順End6/2/4、lead6/2/3、work各2、wait4/0/1。queueは[0,1)に1、[1,2)に2、[2,4)に1、積分5、最大2、観測終端6で平均5/6。within2はcase順0/1/0 |
| P01 start-tie | case3のrank20だけを0へ。t2にcase1/case3の両順。Endはcase1/3それぞれ4..6、case2は2。同順位のFIFO補完を許さない |
| P02 equal / explicit / fifo | 二件同着、rank5/5。省略と明示nondetでは各End集合2/4、各within2の確率包絡0..1。明示FIFOならcase1End2、case2End4、確率1/0。非決定を1/2としない。equal-simは未解決順でU-SEM-0 |
| P02 call-order / reverse | 一案件のfork、同じA容量1、short2/long3。明示FIFOのthen call_orderで字句順を固定。short先ならshort待0/long待2、逆ならshort待3/long待0。案件End5/work5は同じ |
| P03 people | 型Rのfirst/secondをGateの観測stateとrequiresで選ぶ。case1到着0、Gate.pick=[0,1)でfirst。case2到着2、used==0が偽なので未開始拒否second、work0。A.lowはready1、B.highはready2、同じ人pを使いDayは4から勤務。rank0/9なのでB.high=[4,7)、A.low=[7,9)。CaseId順End9/7、lead9/5、work3/3。人pのwork5、peak1。Gateのwork1を人へ足さない |
| P03 people-environment | Bのcontrolだけpolicyからenvironmentへ。t4でpolicyと環境の両順。低先ならA=[4,6)、B=[6,9)、高先なら上の区間。case1End6..9、case2End7..9。人workは5のまま |
| P04 calendar / later | A物理1、Dayは4（対は5）まで0、その後1。到着0/1、rank0/10、所要2。高=[4,6)、低=[6,8)、End8/6、wait6/3。境界を5へ移すとEnd9/7、wait7/4、work不変 |
| P05 external / fifo | concurrent1、到着0/1/2、rank0/1/9、所要4。初回case1=[0,4)、Heldはt4にcase3、t8にcase2。admitted0/8/4、End4/12/8、lead4/11/6、external0/7/2。外部だけFIFOならadmitted0/4/8、End4/8/12、external0/3/6。receivedは元の0/1/2。external合計9+service合計12=lead合計21 |
| P06 external-equal / fifo / sim | Held二件を同順位にするとcase2/3の受入は各4..8、End各8..12。明示FIFOならcase2先だけ。未指定のsimulationはU-SEM-0。initialの二件同着rank0/9では外部規律を宣言してもcase1が初回枠を取り、admitted0/2、End2/4 |
| P07 conflict / no-resolve | A容量2、holder共有、両方0開始/2終了、version_check、rank0/9。resolveありでcase2だけwon、case1lost。各work2、全work4、勝者総数1。開始は旧FIFOのままなので開始優先の代用でない。resolveを外すと両caseにwon/lostがあり、各won確率包絡0..1 |
| P08 conflict-equal / id | rank5/5、resolveのtie省略なら両勝者、同時刻に一人だけ書く。明示tie_by case_idだけ追加するとcase1won/case2lost。確率包絡は1/0 |
| P09 future / tied | Gateのfirst/second選択を同時到着+旧FIFOで確定。firstのGate=[0,1)、secondはt1に未開始拒否。両claimはt1開始。low所要2でt3commit、high所要3でt4stale拒否。CaseId順End3/4、work3/3（lowはGate1+claim2、高はclaim3）。high所要を2へだけ変えると両方due3、高がwon、低lost。workは3/2。ADRの純claim区間0開始の手値2/3へ、観測可能な役割選択のprefix1を明示して具体化した。未来の上位を待たず、既存staleを消さない |
| P10 stale | 三案件の到着0/1/4、rank9/5/0。Gate.pickのfirstがcase1、Gate.nextのsecondがcase2、残りがcase3。高claim=[1,5)、middle=[2,3)、低=[4,5)。middleのt3書込みで高だけstale、低はt4に開始するので新鮮。t5に上位の正常finishは先にconflict（書込み無し）、その後低がwon。Endはlost5/won3/won5、work5/2/1。高のstaleを消す誤りはcase1/3の勝者が逆転して検出される |
| P11 unrelated | Gate prefix1のあと、case1 lowはx、case2 highはyをそれぞれchecked/writesに持つ。両方=[1,3)、二つのfinish順は無関係のまま。状態invariant x==trueならy==trueはlowが先の経路で破れるのでroot violated。全finishをrank全順序にする誤りでは高yが常に先となり、この違反を失う。End各3、workはGateを含め3/2 |
| P11 ack | 一案件、Fast1とA.claim2をforkして両方t0開始。Fastをt1採用、取消要求のackはt2、claimの正常finishもt2。ack先ならdoneへの書込み無し、finish先なら書込みあり。A.done到達の包絡0..1、案件End2/result_at1/work3。resolveでackを抑えない |
| P12 clock / horizon | P05に到達性の問いを追加し、未完了を実際に問いにする。clock6ならcase1だけEnd4、case3は[4,6)で未完了、case2はHeld。case2admitted=null、拒否0、truncated/inconclusive。horizon6/clock20なら二件をcut_off、case2work0/case3work2、End/elapsed/completed_atはnull、truncated=false、到達性はinconclusive |
| P13 actor/resource/independent HOL | 一案件forkでfirst=A.blockedはRを2必要、Rの物理容量は1で永久に不能。last=A.small（uses無し）はActor先頭阻塞。last=B.small usesR1は共通uses先頭阻塞で、物理的には4の開場後に開始可能でも追い越さない。双方root violated、truncated=false、work0。B.smallのusesを外した独立形はt4に開始し5に終わるがfirstは残るのでdeadlock、work1。偽calendar救済と偽deadlockを両方観測 |
| P13 unattributed / physical | case1はGate1のあとBlocker.hold=[1,4)でRを保持。case2はt2到着/未開始拒否でforkがready2。A.bigはRが必要、A.smallは不要、A容量2。then call_orderによる先頭阻塞でsmallも[2,4)待つが、Actor枠も本人のRも人も阻害無し。big/smallはともに[4,5)、case2lead3/work2。small.wait2はunattributed2、actor/resource/executor/combinedは0。smallにusesR1を一つ追加したphysical対ではbig=[4,5)、small=[5,6)、small.wait3=resource3、unattributed0。先頭阻塞を物理阻害と推定しない |
| P14 三explain | startに期限4、externalに期限5、conflictにholder==freeのinvariantを足し、構造反例とverified replayを要求。偽Start/Admitted/Finishの直接注入は別の私有seamにある。公開explainの成功だけで偽経路拒否を検査済みとはしない |
| P15 retry / unknown | 二件同着rank0/9、A容量1、idempotentな集合キー。高case2の二試行=[0,1)/[1,2)、低case1=[2,3)/[3,4)。試行をCaseIdでなくbinding番号やattemptからrank付けし直さない。unknown→deliveredなら二回目は重複拒否を既知の元効果へ結び付け、CaseId順duplicate End4/2、work各2。unknown→unknownなら正常Endでなくreceipt_unknown、observed_until4/2、observed_work各2、root inconclusive、effectは保持。version_check+retryはpriority無しの既存対照でも理由retry_feature_combination/exit4を維持 |
| P16 static/label/unused/max/scenario | 表長・重複・欠損・方向・動的潜在式・tie併存・上限0・二上限不一致・非policy resolve・片側resolve・未選択scenarioの不正を13名で固定。Actor名/grainのみ変更で数値不変。表だけなら旧FIFOのEnd2/4。u64最大rankは符号反転せず上位となりEnd4/2。選択scenarioはmodel表を使う |

## 五つの壊したwire対と局所版

実装を壊した記録ではなく、hand oracleそのものが意味差を検出することを確かめる合成対である。

1. equalの一方のcompleted_atを2に固定し、省略tieをFIFOと推定する。
2. initialのcase1admitted_atを2にし、初回枠を高rankへ譲る。
3. staleの高case1のEndをlostからwonへ変える。
4. futureの低case1のcompleted_atを3から4へ変え、未来の高finishを待たせる。
5. unattributedのsmall待ちをunattributed2からactor_capacity2へ移す。

さらにpriority_policy全欠落、rank表の一値不正、局所版違い、未使用scopeのnullと欠落の混同を検出する。priority_policyは採択済みの正確な欄/型/順を照合するが、Report全体は閉じない。ledgerの/priority_policy/rulesがpriority-queues-v1へ一意に接続することを検査する。既存rootへ新欄を漏らさない。

## 実在する旧fixtureの手値対照

- fixtures/met-fifo/calc.md:4-10によるEnd8/13/18、work各8、overall24、cost78、exit0。
- fixtures/lim-concurrent-wait/calc.md:5によるEnd2/4/6、work各2、exit0。
- fixtures/cf-version/calc.md:3による両caseのclaimed/retry_needed、End1/work1、exit0。

旧模型の全文出力を採取/凍結せず、既存の手計算に対応するJSON pathとexitを実CLIで照合する。uncertainty欄が追加されてもこれらの期待は変えない。検査した三模型を全ての旧入力の同値証明と呼ばない。

## 私有再生seamの受け渡し

tests/support/q5_replay_seam.rsは三試験の本体。src/reference/arrivals_replay.rsのcfg(test)とpath指定のmod宣言から接続する。

各試験は同じ長さのrank表を逆にした別の明示方策の下で構造反例を作り、先にその方策の下でOkを確認する。固定した最初のStart/Admitted/FinishのCaseIdを確認し、元のrank表のModelへ同じStage列を渡すとIllegalTransitionを要求する。source spanもrank以外の型/状態も同じで、単に不正なtraceを両モデルで拒んだだけという空虚な負対照を避ける。参照のFrameやTaskや遷移の複製はない。

この手計算の作成時点ではファイルは未接続で、compile/実行とも未実施だった。接続と実行、および実際の再生での拒否にはそれぞれ別の証拠が必要であり、手値の存在だけで違法順の拒否まで確認済みとはしない。

## 残る非接続の内部義務

停止した案件のQueued先行者を直接作るprobe、公開IRのrank/class/targetの個別偽造、三Taskのchecked集合の非推移的交差、assumptions文言と全scopeの機械的相互照合、conform/Agencyの全交差はこの公開受入群だけの網羅主張に含めない。既存のcalendar内部試験と本群の公開HOL/未使用表/型roundtripはそれぞれ別の根拠である。追加が必要な場合も期待値を緩めて代用しない。
