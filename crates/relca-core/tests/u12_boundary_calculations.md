# U1/U2 の独立した手値と受入境界

基点 c3fba0f8。設計 v0.2 §5.6・§7.2・§22.3、v0.1 §6.1・§6.2、
構成差分、資源、Agency、開始不能の境界を扱う。
以下は開始条件・字句構造・固定所要時間から計算した期待で、CLI の採取値、
snapshot、製品の Agency/engine/lower から生成した期待ではない。
受入に product evaluator を呼ぶのは実測側だけである。

## U1: 12 模型

共通: cases 1 / live_tasks 3 / clock 10。Doc は source document。
A.x は 1 分、B.before/after/pick は 1 分、B.y は 2 分。
すべての Capability は Doc を直接読む。結果は pass 一値、branch_shared の
B.pick だけ pass/fail。capability 宣言順は A.x / B.before / B.after / B.pick / B.y。
Actor 容量 0 は合法で、資源獲得・空き実行者の選択より前に開始条件を満たせなくなる。
requires の拒否も開始条件が揃ってからなので、容量 0 の拒否経路は実行されない。

actor_capacity_infeasible を AC、resource_infeasible を R、state_guards を SG と略す。
数値の範囲は静的 L、時刻/作業量は業務終端に到達する経路の指標。
行に AC/R と書いたときは status=not_derived と理由があり、normal/reachable を出さない。
他の F/L 行は status=derived。F の一部が未導出でも block は derived、complete=false。

| 模型 | 手で追った経路・業務結果 | F | L の主な値 |
|---|---|---|---|
| one | A容量1、x開始0/終了1、done、satisfied/0 | done true | x=1、他0 |
| zero | one の容量だけ0、開始0件、deadlock、violated/1、終端なし | done AC | x AC、他0 |
| unused | A容量0だがxを呼ばない。beforeだけ開始/終了、done=1、work=1、satisfied/0 | done true | before=1、x含む他0 |
| dead | pickはpassのみ。j==failの腕だけxとend never。通常はdone=1、work=1、satisfied/0 | never false、done true/read_only j | pick=1、x含む他0。充足不能腕の容量0は撤回の根拠にしない |
| sequential | before完了1→x待ち。afterへ進めず、violated/1、終端なし | done AC | before=1、x/after AC、他0 |
| fork_all | xとyを生成。yだけ開始して2分作業、joinは閉じずafter未開始。violated/1、終端指標なし | done AC | x/after AC、兄弟y=1、他0 |
| shared_person | A/Bは各容量1で同一executor p。xとyをfork all。どちらの順でも1+2=3、work=3、satisfied/0 | done true | x/y=1、他0。一時的な人待ちを永久不能としない |
| resource_only | A容量1、R容量0、uses1、call_orderあり。deadlock、violated/1 | done R | x R、他0 |
| both_call_order | A容量0かつR容量0、uses1、call_orderあり。資源の既存理由が先 | done R | x R、他0 |
| both_precise | A容量0かつR容量0、call_orderなし。既存の資源blocked解析が先に経路を止める | done false | 全0。既存の正確な資源結果をACへ塗り替えない |
| requires_zero | A容量0でrequires ready==true else fail、ready=false。実際には拒否せずdeadlock、violated/1 | done AC | x AC、他0。旧Lの0が開始条件を解いた証拠にはならない |
| branch_shared | pick=passならx待ち→y/end blockedへ進めず。failならy→after→done=1+2+1=4、work=4。deadlock枝もあるのでroot violated/1、reachable doneはsatisfied | blocked AC、done true/selector j all_contexts | pick=1、x/y AC、after=0..1、before=0 |

branch_shared の y は同じ Capability に二つの呼出し位置を持つ。一方の経路で
容量0 call の完了を前提とするので、Capability 一行全体を AC へ撤回する。
これは追加裁定で明示した保守性であり、他腕だけの別Capability B.after や
fork の兄弟 B.y に未導出を伝播する規則ではない。

O は字句上の reads のまま。unused は Doc に届く Actor={B}、dead/逐次/fork/
shared_person/branch_shared は {A,B}、他は {A}。shared_person だけ people={p}、
その direct={Doc}。間接/contest はすべて空。P は no_proposal_contract のまま。
commit を持たないので、既存 refusal と新しい effects の行はすべて空。
この受入だけで非空 J/U/E の全モデルの不変性を証明したとはしない。

Core は no_resource_overcommit=satisfied。上の deadlock 模型は
W-DEADLOCK-0 が1件であり、deadlock を終了・失敗endへ変換しない。
resource_only/both_call_order は arrivals [0] を持ち、reachable 性質の案件別欄も
case_id=1 / arrival=0 / violated として固定する。

## U1 の試験分割と負対照

- green_u1_* は正例・未使用・死枝・兄弟・prefix・別腕と既存資源理由を検査する。
- red_u1_f_* は5つの模型の終端行、red_u1_l_* は8つのCapability行を別testにする。
  現productionにはACが無いので、13 testは理由の不一致で失敗するのが受入先行の状態。
  これを should_panic/ignore で成功へ変換しない。実装後は同じ名前のまま通る必要がある。
- strict_row は reason を先に比較し、その後にJSON行全体を比較する。
  理由の別語化、statusの偽装、余分なreachable/normal、行の改名/欠落を負対照で拒む。
  手組みの正例を一か所ずつ壊すもので、製品ソースの故障注入を実施した主張ではない。

## U1 追従と V1: join any の後続精密化と理由の優先順

`any_source(0)` / `any_source(1)` は A の容量だけが違う一対。
A.x は1分・fail一値、B.y は2分・pass一値で fork し、pass を join any で選ぶ。
残りは cancel_then_drain、続いて B.after が1分、end done。
両 Capability に協調取消の ack 上限1分を明示する。

- 容量1: x は時刻1で fail、y は時刻2で pass。after は時刻3で完了する。
  work=1+2+1=4、elapsed=3、result_at=2、satisfied/exit0。F done は derived/reachable=true、
  L x/y/after は各1..1。
- 容量0: x は開始されず、時刻2の y の pass で queued x を取り消せる。
  after は時刻3で完了する。work=2+1=3、elapsed=3、result_at=2、satisfied/exit0。
  V1 の裁定で F done は derived/reachable=true、read_only=[y]、selectors=[]、
  framing.complete=true、actors=[]。L x 自身は AC のまま、y/after は derived/1..1。
  x/y の join_any_cancellation は true、after は false。x は静的 L の保守性を維持する。
- if none の done への短絡は y が必ず pass のため実行上到達しない。
  旧 U1 の F done/L after を AC とする受入を、この V1 の手値に置き換える。
  これは到達している Core の採取値から期待を作る変更ではなく、未開始の兄弟が
  Queued→Done に取消されて drain が解けるという開始/取消の意味からの変更である。

理由優先の3模型は、観測 ready=true への状態ガード内で A.x を呼び、
ガードの後で B.after を呼ぶ。A.x は1分、uses R:1、到着[0]、call_orderあり。
F done と L A.x の行の理由は次の手値になる。

| A容量 | R容量 | F done / L A.x の理由 | Core exit |
|---|---|---|---|
| 0 | 0 | resource_infeasible | 1 |
| 0 | 1 | actor_capacity_infeasible | 1 |
| 1 | 1 | state_guards | 0 |

いずれも行は not_derived、framing.complete=false。
状態ガードを除いた正確な静的値へ置き換えず、元の state_guards がある同一行で
resource、Actor、state の優先を固定する。3模型のうち先の2つは時刻0の ready=true に
より x を要求して開始不能となる。最後は x1分+after1分で done に達する。
期待の生成に製品の導出コードを呼ばず、core Report と実CLIをそれぞれこの手値へ照合する。

## V1〜V4 の追加交差と新しい RED の境界

設計 v0.2 §5.6・§10・§22.3、v0.1 §6.1・§6.4、ADR 0009・0025・0034・0036・0043。
次の境界を固定する。variable-match と、uses なし・
別 Actor の call_order の any も精密化を必須とする。未実装なら RED のままであり、
ACI を受ける分岐を照合器へ足して通してはならない。

新模型は cases1/live_tasks4/clock20、A.x=1分、B.y=2分、B.after=1分。
A.x の結果は fail、B.y は pass を基本とし、違うものは表に示す。Actor は匿名枠。
V1/V2/V3 は reads/commits なし。O は空の sources/people、P は従来の未導出、
legacy refusal/lineage/groups と effects.rows は空であり、Core と合わせて完全一致で守る。

### V1: 開始・取消からの手計算

容量0の x は要求されるが開始せず、正常結果も作れない。
時刻0に y を開始、時刻2に pass を公開し、cancel_then_drain が Queued x を
Done にする。x の取消に ack 待ちや作業量はなく、after が2から3まで動く。
したがって done=3、work=2+1=3、result_at=2。容量1対は x の作業1が加わり work4。
どちらも reachable done/safety=satisfied、exit0、計算切断なし。

| fork の形 | Core の手値 | F done / no_match | L x / y / after |
|---|---|---|---|
| Match、A容量0 | 上記3/3/2、exit0 | true・read_only y / false | AC / 1 / 1 |
| Match、A容量1 | 3/4/2、exit0 | true・read_only x,y / false | 1 / 1 / 1 |
| Variable、y=pass,fail、A容量0 | pass なら3/3/2、failならdeadlock。root exit1、reachable doneはsatisfied | true・selector y all_contexts/blocked=true / false | AC / 1 / 0..1 |
| Drain、A容量0 | yは2分で完了するがxはQueuedのまま。終端なし、deadlock、exit1 | AC / false | AC / 1 / AC |
| NoMatch、x/yともfail、A容量0 | y完了後も採用なし。全兄弟が終わらずif noneにも進めない。exit1 | false / AC | AC / 1 / 0 |
| ZeroOnly、x=pass/y=fail、A容量0 | xから結果を作れないため採用なし、終端なし、exit1 | AC / false | AC / 1 / AC |
| All、A容量0 | yは2分作業するがjoinが解けない。exit1 | AC / 行なし | AC / 1 / AC |
| State、空のif ready=trueをfork前へ追加、ready=false | 動的にはMatchと同じ3/3/2、exit0 | AC / state_guards | AC / 1 / AC |
| Requires、xにrequires ready=true else fail、ready=false | 容量0なので実際は拒否もできず、yの採用で取消。3/3/2、exit0 | AC / false | AC / 1 / AC |
| ResourceOrder、R容量1、x uses2/y uses1、call_order | 先頭xが資源不足、同資源yも追い越せない。終端なし、exit1 | R / R | R / R / R |

Match/Variable の容量0自身は AC を維持し、join_any_cancellation=true を維持するという
追加裁定を明示的に受けた。Variable も取消枝の静的行使フラグは x/y=true、after=false。
保守側の Drain/All では x/y/after の同フラグは全て false。
NoMatch/ZeroOnly/State/ResourceOrder は従来の字句上のフラグ x/y=true、after=falseを維持。
Requires の x は静的には拒否の記号経路なのでフラグfalse、y=true、after=falseを維持する。
このフラグを実際に開始・取消した数と同一視しない。

Variable の F は y=pass で done、y=fail で blocked の一文脈なので y が all_contexts。
F actors={B}、complete=true。x の架空の pass/fail を selector や read_only に入れない。
状態/Requires/資源不足の保守側は今回精密化しない。Variable の no_match=false は必須で、
容量0の未完了兄弟を全枝終了とみなす故障を拒む。

### V2: 状態だけの死枝と既知結果の死枝を分ける

ready のガード内に x、合流後に after。ready=false は then 未取得・x開始0で done=1/work1。
ready=true/cap0 は x待ちでdeadlock、after開始0、終端なし、exit1。
ready=true/cap1 は x1+after1で done=2/work2、exit0。
静的 F done は cap0なら AC、cap1なら state_guards。
L x も同じ理由、after は cap0なら AC、cap1なら1..1。yは呼ばないので0。
初期値を動的に観測しても、静的な未知ガードの証拠を勝手に強めない。

B.y=pass の後、v==fail の腕だけの x は既知の充足不能。F done=true/read_only v、
L x=0/y=1/after=1、done=3/work3/exit0。yをpass,failにする単一変更では、
fail側はx待ちでdeadlock、pass側だけdone3。F done と L x/after は AC、y=1、
root exit1だが reachable done はsatisfied。この対で死枝除外の効力を確認する。

混合式 ready==true AND v==fail は、現行では条件全体を緩和するためF done/L x/afterが AC。
同じ論理を入れ子のifに分けると、内側v==failは既知に偽なのでF done=state_guards、
L x=0/after=1。両方のCoreはdone3/work3/exit0。混合式の過剰撤回は未修正の限定として固定。
容量0側の腕が end blocked で終わり、else後だけafterがある対は、blocked=AC、done=SG、
L x=AC/y=0/after=SG。別腕の後続をACIへ汚染しない。

### V3: resource を一つも持たない call_order

arrivals=[0]、then call_order を追加しても、別Actorでusesなしのyをxは永久阻塞しない。
Match/容量0のdone=3/work3/result_at2、精密なF/LはV1と同じ。forkの字句順をy,xにしても同じ。
この二つのF/Lも必須RED。容量1対はdone3/work4/result_at2、全L1、F true。
Allの容量0はyだけが開始しjoin待ち、容量1はmax(1,2)+1=3/work4、result_atなし。
逐次prefix y→x→afterは、容量0でyだけ完了後に停止、容量1では2+1+1=4/work4。
同Actor A容量0のx/zのAllでは両方開始0、全後続停止。B.yはnot_called/0。
資源なしの全行にresource_infeasibleを出してはならない。開始可能な兄弟やprefixもACへ巻き込まない。
新しい流入経路ではroot terminalsでなくcase_outcomesの案件1の終端・時刻・workを完全照合する。

### V4: 非空の通常/atomic J/U/E

Doc→Clerk.enterのentry→Approver.approve(e)のpass/fail→guard→Ledger.post(v)のcommits。
所要は各1分。failならreturned2/work2、passならposted3/work3、exit0。
Clerk0/Approver0は後続へ到達せず終端なし/exit1。Ledger0はreturned2だけ残りpass側deadlock/exit1。
F/Lの該当行はACだが、静的E=Ledger、J=v/Approver、U=v all_paths/bypasses0、roots=eの全行は不変。
Jの入力の根拠源はDocのみ、新しい根拠の直接読取りはないため、legacy lineageの
subject_sources={Doc}/independent_sources={}、groupはjudgements1/actors1。
atomicでlegacy refusalを飛ばす対ではlineage/groupsも空のまま。

Ledger0のguardだけを削除した単一変更はCoreが同じviolatedでも、J stops=none/bypasses1、
U=[]、W-AGY-0追加となる。EやJ自体は残る。これが非空Uの負対照。
判定をpass一値にしてpass腕をreturnedにする死effect対では、Eは字句上のLedger、J/U=[]、
refusal.reachable=false、postのL0。EがderivedでもStart/commitの実績認定ではない。

atomic対はLedger.keysの空集合にkeyを挿入し、既存ならrejected、他はcommitted。
commitsあり/容量1と0で、非空J/U/Eとstate_refusers={Ledger,Ledger.keys,on_normal_path=false}は同じ。
commitsだけを除いた対はE行なし、state_write_effectsをuncheckedへ残す。状態書込みを勝手にEへ増やさない。
担い手のpeopleは空、unattributed_actorsにActorを残す。single_actor=false、single_person=unknown、
single_generation_root=true/roots=e（死effectだけfalse/空）。Authority認定や動的実効性を主張しない。

### 照合と故障の検出欄

- F/Lは全行・全blockを完全照合し、reasonは最初に比較してREDの原因を特定する。
  completeは全行Derivedと一致、actorsは手で導いたselectorのActor集合に一致。
- Coreのverdict/exit、性質、切断なし、終端のelapsed/work/result_at、診断のcode/message/severityの
  multisetを独立手値へ照合する。CLIはraw stderrが診断順の正準表示とbyte一致することも守る。
- 対は容量、remaining、結果集合、call_order追加、fork順反転、guard削除、commits削除の
  具体的な非noop編集をsource文字列の完全一致で確認する。存在しない編集を故障対に数えない。
- 手組みの正常期待を同じstrict_rowへ通してから、ACIへの戻し、actors削除、架空のx selector、
  no_match到達の捏造、AC行へのnormal混入、取消flag偽装、0..1の下端を1にする改竄、
  J/U/EへのACI混入、担い手削除、U空配列、生成根付替えを一つずつ拒む。
  これは照合器の負対照で、製品ソースへの故障注入ではない。
- 受入先行ではV1精密化とV3-any交差の9試験が各targetでRED、それ以外はGREENを要求する。
  REDをignore/should_panic/条件分岐で成功へ変換しない。全段gateは実装後の仕事として残す。

### V1 レビュー追従: 後続の再阻塞・三兄弟・If・適用外

設計 v0.1 §6.1 の生成・開始・終了の三段階と、取消の要求後も解放まで待つ規則、
v0.2 §10 の取消と効果の区別に基づき、
以下を実行前に求めた。Queued の容量0枝は開始も正常結果も作らず、採用後の取消だけで
Done となる。これは逐次の容量0 call の完了を肩代わりしない。
全模型は cases1/live_tasks4/clock20。表の時間・work・result_at は分。

1. **精密な fork の後で再び停止する対。** x=A.x(容量0、1分fail)、y=B.y(2分pass) の
   Any/CancelThenDrain の後に post=B.after(1分pass/fail)。post=pass なら end immediate、
   fail なら C.stop(1分pass)→B.tail(容量1、1分pass)→end done。C の容量だけ0/1の一編集で比較する。
   t0〜2にy、Queued x取消、t2〜3にpost。両容量で immediate は elapsed3/work3/result_at2。
   C0のfail側は stop の開始0でdeadlock、tailの開始0、doneなし。reachable done=violated、root exit1。
   C1はstopがt3〜4、tailがt4〜5でdone elapsed5/work5/result_at2、root exit0。
   F immediateは両方 derived/true、selector post all_contexts、read_only y、actors=[B]。
   C0のF done/L C.stop/B.tailはACI、C1はF done true(同じselector/read_only)、L stop/tail=0..1。
   L A.x=ACI/取消flag=true、B.y=1/true、B.after=1/falseは両方維持。
   no_match=false、F completeはC0=false/C1=true。未導出行にpayloadを残さない。
2. **容量0が二つの三兄弟。** 上の基本MatchにC.z(容量0、1分pass)を追加し、fork/tieをx,y,zにする。
   正常結果があるのはyだけ。t2にx/zを未開始取消し、afterはt2〜3。
   done elapsed3/work3/result_at2、exit0。F done=true/read_only y、no_match=false、actors=[]/complete=true。
   L x/zはACI/flag=true、y=1/true、after=1/false。容量0のzがpassでも架空の読取りに含めない。
   C.zの容量だけ1にした対はt0〜1のz=passで採用し、yの協調取消ackがt2なのでafterはt2〜3。
   done elapsed3/work4/result_at1、exit0。Fはread_only y,z、L z=1/true。他のLは同じ。
3. **既知結果のIf腕の内部。** B.select(1分pass/fail)のvで分岐。pass腕に基本Matchとafter→done、
   fail腕にB.other(1分pass)→other。pass腕はt1〜3にy、x取消、afterはt3〜4なのでdone=4/4/3。
   fail腕はother=2/2/null。exit0、F done/otherはselector v all_contexts、doneだけread_only y、
   no_match=false、actors=[B]/complete=true。L select=1、y/after/other=0..1、x=ACI。
   x/yだけ取消flag=true。別腕だけのB.otherとend otherにACIを漏らさない。
   同じ模型でremainingだけdrainにした負対照はpass腕が停止し、otherだけが残る。root exit1、
   F done/L afterはACI、F other/L other/selectは同じderived、yは0..1/flag=false。
4. **精密化を外す二つの一編集。** 基本MatchのA.xだけduration fixed 1min→0minとする対、
   および未使用B.unused(1分pass)を追加した対のunusedだけにrequires ready==true else failを加える対。
   後者はobserved ready=trueを両側に持つが、processに状態ガードはない。
   どちらも実行するy/afterは変わらずdone=3/3/2、exit0、x未開始/no_match未到達。
   duration0、未使用requiresの各適用外ではF done/L afterは旧解析のACI、no_match=false、
   x=ACI/flag=true、y=1/true、afterのflag=falseを維持。unusedはnot_calledでL0/flag=false。
   正時間の元模型とrequiresなしのunused対照はF done/L afterがderived。
   未使用宣言まで確認する実装の保守境界を受入し、実行不能・unsupportedとは読み替えない。

F/Lは全行・集計・取消flag・余分なpayloadまで既存strict_rowで照合し、Coreの時間、work、
result_at、verdict/exit、診断の多重集合も手値へ照合する。共有supportをcore/CLIの両入口で使う。
上記の容量・remaining・duration・requiresの一編集をsource完全一致で検査する。
手組み正常期待を受理した同じ照合器が、後続ACIの消去、直後のderivedの撤回、zの架空read_only、
他腕へのACI漏れ、適用外のderived化という単一改竄を拒むことも確認する。
これは照合器の故障対照であり、productionの変更・故障注入ではない。

## U2: 四つの模型、各三構成、七つの拒否形

Invoice は source document。per_case 指定/省略と、Rule改定あり/なしの直積4模型。
Left/Right は P、Third は別の Q を選ぶが、両processは end done だけ。
従って時間=0、work=0、reachable done=satisfied、両側exit=0、Agency差分0。
Ruleなしは構成差分0。RuleありではRightだけ flag=true/v1→false/v2 の改定を適用する。
差分はrule Ruleのmodified一行。Ruleを改定することをinstance変更と取り違えない。

lower 後の写像は全構成で、per_caseなら Invoice→Some(PerCase)、省略なら Invoice→None。
Ruleがある模型では Rule→None が加わる。Rightのeffective_version/値/amendmentも
同時に検査し、実際には改定が適用されていない模型を不変性の証拠にしない。
CLIのinstance仮定はper_caseならInvoice.instance=per_case一行、共有なら無し。
この入力には局所状態が無いので共有既定仮定も不要。二ファイルを別々にdiffしても、
各ファイル内の左右は同じinstanceであり、二ファイル間のinstance差を比べたことにはならない。

| 拒否形 | 診断 | product exit |
|---|---|---|
| configuration内instance | U-SYN-0 / unsupported syntax: instance | 4 |
| configuration内Actor宣言 | U-SYN-0 / unsupported syntax: actor | 4 |
| モデル直下の同名Actor二重宣言 | U-SEM-0 / duplicate Actor declaration | 4 |
| instance sharedという未対応の綴り | U-SYN-0 / unsupported syntax: Actor instance | 4 |
| 一つのActorにinstance二回 | U-SEM-0 / duplicate Actor instance | 4 |
| 普通のInvoiceをRuleとして改定 | E-CON-0のchange grant / amend requires a rule Actorの二診断 | 3 |
| Ruleに存在しないinstance欄を改定 | E-NAME-0 / undefined rule value | 3 |

診断のcode/message/severity/span/順序を完全一致で検査する。構文拒否のReportには
既存W-CUT-0が加わるが、parserの直接戻り値にはReport側の追加警告を要求しない。
spanは手書き入力の原因tokenのbyte位置/句の長さから計算する。正準stderrは
CLIのfrom/to/diff順と接頭辞を守り、異なるsideの同じ診断も消さない。

負対照はfrom/toそれぞれのinstance値改竄、仮定行削除、架空の構成差分、exit偽装、
各拒否のcode/message/severity/spanの単独改竄。全体がunsupported/4であるだけでは通さない。
同名Actorの左右差が現入口から到達不能であることを守る受入であって、
将来の二IR比較にinstance射影を実装したという意味ではない。
