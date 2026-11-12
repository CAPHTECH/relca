# ADR 0064 Q6a 受入 — 手計算と観測契約

基点 78b71606。ADR 0064 と Q6 checkpoint 裁定の P4/P5 が正本。
本節の値は新実装を読む前に算出した。全て模型の仮定であり技能の測定ではない。
時間の単位は分、作業量は作業・分、確率は無次元、期待件数は件/反復。
費用単価は全入力で未設定。0 円とせず not_configured を要求する。

## 手計算（A1〜A6、A10、A12〜A14）

| ID / 入力名 | 開始・終了・割当と算術 | 一変更の対と検出 |
|---|---|---|
| A1 qualified | Desk の宣言順 p,q、資格 q のみ。候補集合を先に絞るので q が 0..2 を担当。work=2、p=0/q=2 作業・分。permitted=[q] で satisfied | forbidden: permit=[p] のみへ変更。q をそのまま割り当て permitted が violated、exit1。資格/permit を交差すると隠れる違反。専用診断 code は無く contract permitted の性質id/verdictで検出 |
| A2 shift | q の勤務 [0,3) は0、[3,21) は1。p は全時間 idle でも未資格。開始3、終了5、wait=3、work=2。結果 ok は確実 | empty: 資格を [] にする一変更。将来も候補不在、開始0回/work0、終端なし。deadlock 検査が violated/W-DEADLOCK-0。勤務開始で p を補わない |
| A3 people | p,q とも資格あり、load none。p の bad=1/10、q の bad=2/5。first_idle により p、P(bad)=1/10、終了2/work2 | reverse: executors 宣言順だけ q,p。q を選び P(bad)=2/5。集合の並びや成績を割当順位に使わない。unassigned: assign のみ削除、網羅 min1/10 max2/5。simulate は未解決割当 U-SEM-0/exit4、均等混合1/4にしない |
| A4 burst | 匿名容量1、到着0,0,0、FIFO。開始0,2,4、終了2,4,6、開始直前の他 Queued=2,1,0。各 bad=1/2,1/2,1/10、E[bad]=5/10+5/10+1/10=11/10件。全流入3で割り11/30。work合計6、各work2、wait0,2,4 | lighter: 高帯 bad の kernel 参照だけ Half→Quarter。各1/4,1/4,1/10、E=5/20+5/20+2/20=3/5件、比1/5。時刻は不変 |
| A5 held | concurrent cases1、外部 FIFO。到着0,0,0、受入/開始は0,2,4。各開始で他案件は Held であり Queued でない。q=0,0,0、E[bad]=3×1/10=3/10件、比1/10。終了2,4,6、work合計6。外部待ち0,2,4はelapsedから消さない | concurrent 行だけ削除した burst が対。11/10件へ。Held を backlog に数える実装は held の値で落ちる |
| A6 later | 到着0,1、容量1、duration4。最初は時刻0に q=0 で Low を凍結。時刻1の新 Queued で選び直さない。二件目開始4もq=0。bad=1/10+1/10=1/5件、全流入比1/10、終了4,8、wait0,3、work合計8 | simultaneous: 到着列だけ0,0。初回q=1で1/2、二件目q=0で1/10、E=3/5件、比3/10。終了4,8、wait0,4。終了時選択の故障は later を3/5件へ誤変化させる |
| A10 clock | 単一案件、duration4、clock3。開始q=0だが終了未観測。truncated/inconclusive/exit2、品質比はnull reason=truncated、期待件数は保守的区間0..1件で同じ理由。正常 bad=false で埋めない | horizon: 時計10を確保し業務 horizon3。cut_off/inconclusive/exit2、品質比はnull reason=undetermined、期待件数は0..1件。計算truncatedと別。work観測済み下界と完了workを混同しない |
| A12 complete | people は二人×load none一行、burst は anonymous×帯[0,1),[1,infinity)。根拠は fixture assumption（未検証） | basis空、p行欠落、帯gap、overlap、未知kernel、資格のActor外/重複、匿名資格、時間分布併存、確率和不正をそれぞれ一変更。具体分類は support の STATIC。静的拒否は実行をしない |
| A13 priority | 容量2でも資格qは同時1作業のみ、勤務開始3。rank0,10ならcase2が3..5、case1が5..7、work各2/合計4、wait case1=5/case2=3 | priority-reverse: rankだけ10,0。case1が3..5、case2が5..7、work4不変。pの空きで開始しない。qを取り置きせずpriorityは開始可能時点で解決 |
| A14 refused | 資格q、勤務/枠/資源は利用可、requires false、else denied。結果拒否は時刻0、開始0回、work0、kernel乱数0。badへの質量0は観測した拒否から導く（未入力分布の補完ではない） | admitted: requires条件だけ true。qが0..2、開始1回/work2、正常終了だけ kernel siteを1回消費。取消ackは別のprivate観測でkernel消費0を固定 |

A4〜A6の期待件数は期待の線形性の和で、案件を独立な標本とは仮定しない。
shiftの待ち3分の唯一の原因はcalendar。有資格qは勤務外、Actor枠/資源は空き、
pは資格候補から除くためexecutor待ち0、combined0、unattributed0とする。
品質の件数と全流入比を別々に検査する。非決定の残る A3 は per_case_relaxation の区間。
確率を宣言しない A1/A2 は結果okの一値非決定であって、一様乱数の模型ではない。
verify の問いは各入力で明示し、quality 宣言だけで root を satisfied にしない。
simulation は標本であり、safety の無違反は inconclusive。試験では解析値を標本値として比較しない。

## 校正と未接続の扱い

M1〜M7/M9/M10 が担当。M8 は Q6b の範囲。
M1/M2 は qualified/forbidden、M3 は shift と assign無し/explain、
M4 は burst/held/単一案件、M5 は later と private selector 状態、
M6 は private draw 記録と refused/admitted、M7 は people/burst と実 simulate、
M9 は clock/horizon/defer、M10 は wire 単一故障で検出する。
内部seamの検証済み範囲は、接続と実行の根拠で確認する。入力の表だけでは実行済みとしない。
この手計算は、すべての交差の実行や固定shaと単位別の接続を確認したという主張ではない。

## 標本の独立 oracle

seed=1、16反復。正常終了の site は通常 kernel.Flow.r、known-none は既存 Q1 の kernel.Flow.r.attempt1 を保つ。案件番号は1起点、n=1。
ADR0016の relca-sim-v1 のSHA256先頭8byteを big-endian 整数 x とする。
bad確率を n/d と書くと、okが先のCDFなので bad ⇔ x×d ≥ (d−n)×2^64。
比較は整数だけで行う。qualified/executor/帯を選ぶ乱数は追加しない。
開始・requires拒否・取消ackでこの鍵を消費しない。

次の16bit maskは bit(r−1) が反復rのbad、最下位から読む。
Python標準hashlibで独立に算出し、Rust sha2 の同じ契約で全bitを再検算する。
Relcaの乱数器や集約器を期待計算に呼ばない。

| 模型 | case1 mask | case2 mask | case3 mask | bad合計/16反復 → 期待件数標本、全流入比標本 |
|---|---|---|---|---|
| people | 0821 | — | — | 3/16件、3/16 |
| known-none | 4200 | — | — | 2/16=1/8件、1/8（Q1 attempt1 の鍵） |
| reverse | 2d21 | — | — | 6/16=3/8件、3/8 |
| burst | 2d31 | d38e | 0200 | (7+9+1)/16=17/16件、17/48 |
| lighter | 0821 | 408c | 0200 | (3+4+1)/16=1/2件、1/6 |
| held | 0821 | 400c | 0200 | (3+3+1)/16=7/16件、7/48 |
| later | 0821 | 400c | — | (3+3)/16=3/8件、3/16 |
| simultaneous | 2d31 | 400c | — | (7+3)/16=5/8件、5/16 |

たとえば burst の反復別bad件数は
1,1,1,1,1,1,0,1,2,2,1,1,1,1,1,1。
mean=17/16、min=0、max=2、samples=16。
case48本を48独立trialとしない。解析値11/10と標本17/16を同じ値として比較しない。
known-noneはQ1既存のwaiting breakdown拒否を避けるため、その模型だけbreakdownを宣言しない。
Q1の既知結果接続の受入であり、新しくQ1×waitsを開く要求ではない。
Q6 実装時の orch 裁定により、known-none の既存 Q1 契約の期待を訂正した。
check は完全探索で receipt_resolution=satisfied、root satisfied/exit0 のまま。
simulate は receipt_resolution=inconclusive が自動付与され、root inconclusive/exit2。
metrics の Task binding は既存の内部名 @response:0:1 を保持する。
標本の鍵は relca-sim-v1|seed=1|rep=r|case=1|site=kernel.Flow.r.attempt1|n=1。
hashlib.sha256 の先頭8byteを big-endian 整数 x とし、x×10 ≥ 9×2^64 を独立に16回評価すると、
bad は反復10と15だけで mask=0x4200、件数標本と率標本は2/16=1/8、min0/max1。
production の観測から期待を採取していない。解析の確率1/10、時間2、期待件数1/10は不変。
根拠は report.rs の simulation の receipt_resolution 自動付与と、既存 job response-g-unknown の receipt_resolution。内部 Task 名は既存の Q1 サイト命名を保つ。

### 固定するselectorと既存given条件の評価時点は別

given-finishではWriterが0..1にflag=trueを書き、Samplerは0..2に実行。
selectorは開始時にprofile Pのanonymous/load none→Kを選んで保持するが、
K given[flag]の条件行は従来どおり正常終了時刻2に読む。
flag=trueの行はhit=true/false各1/2なのでP(hit)=1/2、期待Hit=1/2件、比1/2。
given-laterはWriterのdurationだけ3分へ変更。Sampler終了2ではflag=falseのままなので
hitの正質量は0、P(hit)=0、期待Hit=0件、比0。0質量枝の型も静的には検査する。
join all後の時刻は正2分、対3分。verifyの閾値1/2により正satisfied、対violated。
別の通常kernelの手値対照でも同じ1/2と0を要求する。
この既存評価時点の保護は新selectorを開始時へ凍結する要求を緩めない。

### 分位点との最小接続

people/reverseはともに全案件が時刻2にdoneへ達するので、Doneの全流入medianは2分。
Badの質量は1/10または2/5で正、到達条件付きの到達時刻は全て2分なので、
Bad given reachedのp=1分位点も2分。成績の差を時間の差へ変換しない。
peopleのdurationだけ3分へ変えた単一故障対は両方3分になる。
この3入力のquantile期待は出力から採取せず点質量の逆CDFから求めた。

## 受入側で具体化した wire（裁定による）

work_profilesは次の閉じたキーを持つ。未知キー、欠落、重複、型不一致を拒否する。

| object | キーと意味 |
|---|---|
| work_profiles | rules=work-profiles-v1、qualifications配列、profiles配列、bindings配列 |
| qualificationsの行 | capability=Actor.cap、actor、executors（宣言順、匿名はnull）、qualified_executors（集合の正規順、欄省略はnull、明示空は[]）、permitted（資格と交差しない既存許可集合、匿名はnull） |
| profilesの行 | name、basis、evidence_status=analyst_declared_not_validated、load=noneまたはqueued_same_actor_at_start、snapshot=start、duration_effect=not_supported、rows |
| rowsの行 | executor（PersonId文字列またはanonymous）、lower（queuedは非負整数、noneはnull）、upper（queuedの最終infinityはnull、noneもnull）、kernel（参照名） |
| bindingsの行 | capability、profile（資格/成績の別を保存する参照） |
| ledger.derivation.components | at/nameが /work_profiles/rules、version=work-profiles-v1 の行ちょうど1つ |
| uncertainty.model.items | kind=model_context、at=/work_profiles をちょうど1つ。宣言を実技能の計測に格上げしない |
| uncertainty.aleatory.items | 既存kind=kernelの形で各実効kernelをcapability/nameの組ごとに1つ。profileの選択帯の全kernelを省略せず、未使用の表は実効分布へ混ぜない |

匿名・省略・無限上端を表すnullは欄の意味によって定まる（推定値の欠損ではない）。
数値の欠損は既存quality/metricsのreason付きnullをそのまま使う。
宣言行はprofile/qualificationの順で保存し、qualification集合だけPersonIdで正規順とする。
実行時のqを新しい集約欄に追加することは要求しない。
未使用performance宣言はprofilesに残すがqualifications/bindingsに架空の使用を作らない。

診断の複数候補はorch裁定により一つへ固定した。
qualification_mismatch/performance_coverage/performance_contract_mismatchはE-TYPE-0、
空basisはE-CON-0、確率和はE-PROB-0、未対応交差はU-SEM-0。
全static入力の分類はq6a_inputs::STATICに閉じる。

## 入力の境界と故障の単位

RUNTIMEは25構成、CONDITIONALは上記givenの2構成、CAPACITY_CONTROLSは2構成、
STATICは27構成。これにquantile_sourceの明示した3選択と旧言語のsubstrateを別に置く。
この件数は有限な受入の構成数であり、意味論の網羅率ではない。

容量の正対照はprofile256個（各2帯、selector計512行）と、単一profileの4096行。
負対照はprofile宣言を1つ足して257個、または最後の帯を一つ分割して4097行。
負対照も各表の分割は完全であり、故障は宣言容量の超過だけ。
いずれも業務上の案件数は1で、計算のtruncatedとは区別して静的unsupportedにする。

STATICのうちoutside-actorはpeopleにxを宣言した有効な土台から資格q→xを変更、
anonymous-qualifiedはpeopleにqを宣言した匿名Actorの有効な土台へ資格欄を追加する。
version-checkはbadを観測stateにした有効な土台へversion_checkを追加する。
これらの土台の設定は故障ではない（未知名や潜在チェックの別エラーを混ぜないため）。
wrong-contractはQ40の上書き先集合全体を{bad}から空集合にする一変更。
一枝だけ上書きを消して集合の和が不変な模型を契約不一致と決めつけない。

公開wireの16変異は各々、同じbase（peopleまたはburst）の正常なsubtreeから一箇所だけ変更。
実Reportへの投入も同じbaseの正常Reportを先に受理してから行う。
peopleのReportへburstの表を入れて別の不整合で拒否されることを、帯検査の成功と数えない。
これとは別にledger規則版、実効kernelへの参照、model仮定参照の3変異を検査する。

資格だけのsimulateでは16反復すべてpのwork0、qのwork2、資格選択の乱数0。
requires拒否はStartが無いのでtask.waitは0でなくnull/reason=refused、
別欄refusal_waitが0分、refused_countが1。ADR0026の既存の欄の意味を保つ。
初稿の共通schedule照合でwait=0を要求した箇所はこの区別に訂正した。
safetyを宣言したこの模型はinconclusive/exit2で、無違反を網羅証明にしない。
performance模型のsimulateは確率の問いがP(bad)≤1なので区間上端≤1でsatisfied/exit0。
これは当該確率の統計的支持であり、別の安全性の証明ではない。

privateのreference集約、直接IR不整合、selector Eq/Hash・clone/view/store/replay、
取消のdraw/event観測、細分したbacklog母集団のassertは固定したが、接続前は未実行。
JSONを受け取るclosureは新しい公開APIを仮定せず試験をコンパイルするための接点。
各interpreterの実際の観測を渡し、固定した期待を返すadapterで代用しない。
