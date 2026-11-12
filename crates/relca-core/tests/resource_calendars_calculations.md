# Q4 / ADR 0059: 受入の模型・手計算・接続の範囲

基点af24fd8e、設計 v0.2 §21・§22.3。採択済みADR0059の意味の受入である。
この数値の正本は以下の区間と算術であり、新実装のstdoutを採取していない。
受入時点のproductionは新構文未実装なので、REDを意味合格と呼ばない。
ledgerは既存の出力であり、calendarの手計算とは別に検査する。

## ファイルと入口

- `support/q4/*.relca`: 手書きの10基礎模型。新しい本番fixture群の自動列挙やsnapshot集合は変更しない。
- `support/q4_inputs.rs`: 33 runtime（旧構文固定列対照2を含む）と14 staticの閉じた47入力。派生は明示した置換だけ。新意味の期待を製品から読む処理はない。
- `support/q4_oracle.rs`: 手値から作るpartial report oracle。Scalar/Sample/nullは閉じた完全一致、caseは件数を閉じる。Task/Actor等は名前で一意に照合し、表の順を恣意的に仮定しない。
- `resource_calendars.rs`: 公開core結果、engineと独立referenceそれぞれの手値、全Outcomeの差分、S1の各反復とSHAを検査。
- `../../relca-cli/tests/resource_calendars.rs`: 実CLIのstdout JSON・全stderr（診断ミラー）・exit。既存ledgerの閉じた形も検査する。
- `../../relca-cli/tests/support/q4_legacy_raw.py`: 既存golden6件のraw三チャネルだけを再利用。新goldenを採取しない。

Scalarはmin/maxを既約な非負有理数の文字列で表し、reason=null。計算不能はmin=max=nullと理由。
終端時刻/elapsed/workのOutcomeは既存の整数Range。Sampleはmean/min/max/samples/reasonを全て固定する。

## 保存する観測

- Task.waitはready→start。wait until中はTaskがまだ生成されない。Task.wait_causes.condition=0。
- 実条件待ちはcase.breakdown.condition。calendar待ちはready後なのでTask.wait_causes.calendar。
- 案件の区分和=lead time、Taskの原因和=Task.wait、unattributed=0を数値で確認する。
- 物理capacityとschedule枠を区別。continue中の閉場はDone/資源解放/人の交代を起こさない。
- 営業開始を次事象として時計に入れる。表の隙間は休み0でも24時間利用可能でも補わない。
- 総work、通常枠占有、超過枠、料金用時間外、相対期限は同じ経路/反復から読む。
- horizonとclockは終端/指標の扱いを分け、未完了を成功/失敗にしない。

## C01: 開場（open / open-later）

到着0、所要2、容量1。表は0〜5が0、5以降が1。ready0/start5/End7。
work2、lead7、wait5。calendar原因5でintentional/conditionは0。
Actor queueは[0,5)が1、積分5、最大1、観測終端7で平均5/7。
scheduled capacity timeは[5,7)の2、通常枠占有2、時間外0。
開場境界を6へ動かした対はstart6/End8、wait6、work2不変。

## C02/C13/C14: 継続と費用（continue等）

容量1、到着[0,1]、所要5、開場[0,3)と[6,21)。
案件1=[0,5)、案件2=[6,11)。leads5/10、work各5、総work10。
案件2 wait=[1,6)の5を端点で分ける:

| 区間 | 原因 | 長さ |
|---|---|---|
| [1,3) | actor_capacity | 2 |
| [3,5) | 占有と閉場のcombined | 2 |
| [5,6) | calendar | 1 |

案件2の区分和はservice5+queue5=10。固定waitに移す量は0。
通常枠占有は案件1の3+案件2の5=8、超過枠2。
観測終端11の表容量時間も3+5=8、scheduled utilization=8/8=1。
既存utilizationは10/(1×window20)=1/2のまま。

- `continue`: 通常率2、時間外率未設定。通常成分16でも時間外成分とtotalはnull/not_configured。
- `explicit-rate`: 時間外率3を一つ足す。8×2+2×3=22。
- `free-overtime`: 明示時間外率0なら16。未設定をこの値にしない。
- `continue-earlier-close`: 閉場開始3→2。End/work不変、通常7/時間外3、case2原因actor1/combined3/calendar1。

## C03: 交代（shift / shift-later）

Actor物理2/表2、実行者宣言順[p,q]。pは[0,3)、qは[3,21)だけ勤務。
到着[0,3]、所要5。p=[0,5)、q=[3,8)。pは3〜5も同一Taskを保持する。
人ごとのwork5/5、peak各1、p勤務外2、q勤務外0。Actor枠超過0。
Actor表容量時間=2×観測終端8=16、実占有10、通常枠占有10。
料金用時間外はpの勤務外2なので、Actor枠超過0と同一視しない。

q勤務開始だけを6へ動かす対ではpをqへ移管せず、q=[6,11)、case2lead8/wait3=combined2+calendar1。
[3,5)はpがbusyかつ勤務外、qも勤務外でcombined2。[5,6)は63行のp=[0,5)の終了後なので両人とも勤務外だけでcalendar1（ADR 0059の原因分類）。
この因果寄与を数量化した値ではなく、採択した原因分類である。

## C04: 全資源（resource / resource-later）

Actorは常時1、Terminal表は[0,4)が0。uses Terminal:1、到着0、所要2。
start4/End6。Actor/Terminalとも実占有2、開始前は作業0、Task.wait4=calendar4。
Terminalの表容量積分は[4,6)の2。5へ開場を動かす対はEnd7/work2。
公開量で時刻・作業量は守るが、未開始の私有予約だけを直接読むadapterはまだ無い。
別Actorの競争による部分予約の故障注入はADR M05の後続義務として残す。

## C05: 取消ack（ack / ack-later）

A短枝[0,1)、B長枝は所要5。t1にAを採用、Bへ取消要求。
Bはt1から閉場でも解放せず、ack_within1ならack2/End2。
A work1、B work2（通常1/時間外1）、総work3。

ack_within2は**正の遅延1か2**であり、固定2ではない。
したがってack/End2..3、Bwork2..3、B時間外1..2、総work3..4。
草稿の「End3だけ」は上限を固定遅延と読んだ誤りだったのでADRを訂正した。
期待を新実装へ合わせた変更ではなく、既存取消契約へ照合した訂正である。

## C06: 業務打切りと計算切断

`horizon`: C01にhorizon6、clock20。start5、t6でwork1、Endなし。
cut_off、bounds.truncated=false、reachableはinconclusive。
dwellはcalendar5+in_service1=6、until6、bound=lower。

`clock`: C01のclockだけ6へ。未完了をEndへ変えず、bounds.truncated=true。
網羅の新容量時間/時間外量は理由truncatedのnull。horizonの確定観測量とは別。

## C07/C10: 条件待ちとカレンダー待ち

草稿のcase.id==1による分岐は現DSLが持つ整数比較ではない。
新たにCaseId整数比較を導入せず、既存のrequires/startとFIFOで同じ時間模型を作る。

Boardは物理/表2、requests=0、ready=false。setはrequires requests==0 else waiter。
通った1案件だけがstart requests+=1、4分後finish ready=trueでownerを返す。
後続案件はt0のrequires拒否でwaiterを受ける。requires拒否はTask作業0、finish/commitではない。
requestsの加算は可換、readyは明示last_write_winsで既存共有writer検査を通す。
これは静的writer規則を黙って緩めた模型ではない。

- `condition`: FIFO case1=ownerでEnd4/work4。case2はcondition[0,4)、Desk task ready4/start4/End6/work2。
  Task.wait0/Task.condition0、predecessor_wait4。case2の和はcondition4+service2=6、unattributed0。
- `condition-later`: set4→5。case2 condition5、Task.wait0、End7/work2。
- `condition-open`: Deskは[0,6)閉場。case2 condition4、Task ready4/start6なのでcalendar2、service2、End8。
  Task.wait2=calendar2、Task.condition0。案件の和は4+2+2=8。
- `condition-open-later`: 開場7でcondition4不変、calendar3/Task.wait3、End9。案件の和4+3+2=9。
- `condition-sim`: clear無し。4反復とも上の同じEnd、case2 condition Sample4、Task.wait Sample0。

## C08/C09: 初期条件・deadlock・同時刻競合

- `condition-true`: 到着0、初期ready=true。wait untilは1即時遷移で通過しEnd2/work2、condition0。
- `condition-deadlock`: 初期ready=false、writer無し。時間を捏造せずdeadlock、Endなし、work0、root violated。
- `condition-race`: C07のownerがset終了後clear1分。clear startでready=false、Desk所要1。
  t4の具体的な二つの順は以下（管理の0時間段も経過時間には足さない）:
  1. set finish(true) → waiterのWaitUntil通過 → clear start(false) → Desk start → t5で両完了。case2 End5/work1。
  2. set finish(true) → clear start(false) → waiter評価(false) → t5 clear終了 → 以後writer無し。case2 deadlock。
  case1は両方End5/work5。reachableの有無だけでdeadlockを隠さず、rootはviolated。
- `condition-race-sim`: structural reachableをprobability queryへ置換してからsimulate。
  構造的な問いを拒否する既存経路を「条件競合の検出」と数えない。遭遇した未解決非決定はunsupported。

## C11/C12: 完全到着列と独立SHA

共通: N=2、所要3、容量1、FIFO、4反復/seed1/confidence1/2。
明示分布は半分[0,0]、半分[0,4]。無限到着の切捨てではない。

| support | End | case2 lead | case2 wait | lead≤4のhit |
|---|---|---|---|---|
| [0,0] | [3,6] | 6 | 3 | 0 |
| [0,4] | [3,7] | 3 | 0 | 1 |

解析参照値E(lead2)=9/2、E(wait2)=3/2、P(hit)=1/2。
`fixed-burst`と`fixed-spaced`はこの**各固定列だけ**を旧構文のGREEN対照にする。新分布の網羅検査の代用ではない。

到着用siteはcase=0/site=arrival.schedule/n=1。各反復のキー末尾以外の形は既存relca-sim-v1。
Python標準hashlibとテスト側SHAにより、製品を呼ばず次のdigest先頭64bitを固定した:

| rep | digest先頭16桁 | 第2列選択 |
|---|---|---|
| 1 | 1ab95de28da79f2e | false |
| 2 | 3a037e898da85e28 | false |
| 3 | 89722ae085a119fa | true |
| 4 | b2b842ec0c312694 | true |

したがって実到着2は[0,0,4,4]、mean2/min0/max4、samples4。
leadは[6,6,3,3]→mean9/2/min3/max6、waitは[3,3,0,0]→mean3/2/min0/max3。
hits2/unknown0/trials4。arrivalは集約で**必須null**、arrival_timeは必須Sample。
countの全流入は各反復2、observed_total8。生き残った案件だけを分母にしない。
queryの閾値0はroot satisfiedの入口を単純にするためで、正しさはhits/時刻の手値で別に検査する。

`arrivals-burst`: 第二列だけ[0,1]に変更。実到着[0,0,1,1]、mean1/2。
case2 lead[6,6,5,5]でmean11/2、wait[3,3,2,2]でmean5/2、hits0。
同じseedで別のTask用乱数siteを一つ消費する実装を許さない。

`arrivals-calendar`: 同じ分布に[0,5)閉場を足す。全反復case1=[5,8)、case2=[8,11)。
case2 lead[11,11,7,7]→mean9/min7/max11、wait[8,8,4,4]→mean6/min4/max8。
case2のcalendar原因[5,5,1,1]→mean3/min1/max5、actor原因は常に3、condition0、hits0。
平均到着2で一回実行する置換では、この原因分布を再現できない。

## C15/C17: retryと部分枠

- `retry`: G形のCaseId冪等登録、所要1、喪失[unknown,delivered]、最大2/backoff0。
  初回[0,1)のcommit/keysは保持。次の開場4まで新試行を開始せず、第2試行[4,5)は重複拒否。
  receipt at1 unknown、at5 delivered、End duplicate5、work2、at_most_once satisfied。
  `retry-later`は開場5、End6、それ以外の効果・work・backoff0は不変。
  Q1の既存retry+breakdown未対応を解禁しないため、この模型にmetricsを付けない。
- `drop`: 物理2、表[0,1)が2/以後1、同時到着2件/所要3。
  両方[0,3)、work6、no_resource_overcommitはsatisfied。
  容量時間2×1+1×2=4、通常枠占有4/超過枠2。
  Task単独なら全区間に枠1があるため料金時間外0、通常率2でtotal12。
- `drop-zero`: 低下後だけ0。容量時間/通常枠2、超過枠4、料金時間外4、通常料金時間2。
  時間外率3を明示し、2×2+4×3=16。`drop-zero-missing-rate`ならtotal null/not_configured。

## 壊した対と校正の意味

| 故障 | 入力/観測対 | 受入への接続 |
|---|---|---|
| 省略を休日0と推定 | open→gap、0容量区間一行だけ削除 | productionの静的拒否、E-CAP-0/invalid_calendar_partition |
| schedule低下をovercommit | dropの手書きwireでverdictだけviolatedへ | 独立oracleは拒否、runtimeのdropが実productを手値へ接続 |
| 未設定率を0円 | continueのtotalを通常成分16の既知値へ | oracleは拒否、continue/explicit-rate/free-overtimeで実productへ接続 |
| 最初の標本arrivalを流用 | arrivalsのcase2 arrival nullを整数0へ | oracleは拒否、S1のruntimeと独立反復検査で接続 |
| calendarをintentionalへ | openのcalendar5をintentional5へ移す | oracleは拒否、openのruntimeで接続 |
| conditionの二重計上 | condition-openのTask.wait2を6へ | oracleは拒否、case和/Task和/condition0でも接続 |

これはsynthetic wireによるoracleの校正であり、productionの新collectorを一時改造して故障を落とした記録ではない。
未実装productionへの故障注入を実施済みと書かない。

## 追加境界・未接続義務

- `STATIC`の14入力は、on_close/suspend/区間隙間・重複/clock被覆/対象・名前/未選択scenario/潜在/確率和/basis/件数/未対応modeを個別分類。
- S1のmutate/compare両mode/diffも公開core入口から機械的な拒否理由を要求する。
- Agencyの未導出、conform.not_checked、旧比較表への新量の自動流入禁止は公開core検査に接続。
- scalar/nullのpresenceとcase閉包はoracleで守るが、Calendar型を偽造する内部adapter、資源の内部予約、全replay辺/履歴Hash/Eqの単独故障は別の根拠を必要とし、この受入だけでは検証済みとしない。
- 全てのS1×quality/quantile/defer/contestの直積、全能力上限256/10000の境界、DST等の範囲外構文、全CLIの全mode/selectorの直積を検証したとはしない。
- 新syntaxを除いたsubstrateのparse/lower成功は模型の型/権限/構文の下地確認だけ。新しい意味のGREENではない。
- 既存raw対照6件はserial-time、parallel-time、wait-intentional、resource-contention、sim-statistical、serial-time.broken-missing-capacity。
  三チャネル全byteを既存goldenに照合する。snapshotから新時間指標の意味期待へ値を移していない。
