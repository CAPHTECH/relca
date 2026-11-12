# Receipt unknown 回復の独立手計算と受入契約

基点 051b38d6。正本は ADR 0065 の 2〜8 節と採択追記。
実装の出力を数値の oracle にしない。時間は分、占有は人・分。
以下は模型の宣言からの計算 [R]。到着、所要、結果、割当、commit 定義を変えれば外れる。
PSP の実際の正確性、CS の規則、工数、権限の実在は未確認 [U]。

## 1. 模型と保存する意味

新規の `support/r2_inputs.rs` に模型を置く。既存 fixture と snapshot は変更しない。
Psp は機械、CS=[c]、Finance=[f]。charge は G の atomic check-and-insert。
実課金は `charged_truth` / `charges` と Psp.keys で表し、これらを回復 process の
保護対象にする。通常業務が読むのは query の status と CS の decision だけである。
現行言語の latent 書込みは未対応なので、mutable 真値を latent にして別の未対応機能を
要求しない。`hidden_truth` は潜在読取り拒否用の読取専用初期値である。
保護した observed は通常の観測へ自己申告するための抜け道ではない。

単価なしは not_configured。query/resolve は commit でない。
通常の受信経路では Shop.ship_known(pay) を1分、回復経路では別の
Shop.instruct_ship(decision) を1分実行する。literal enum を実引数に渡さない
既存の binding 規則に従う。unknown の pay はどちらの条件にも公開しない。
Psp.refund は別の実効果であり charge を巻き戻さない。
判断の finish を判断記録とする。R05 の追加記録はそれを上書きしない。

R03 は on block なし。未使用 query Capability の専用結果源も通常の
result nondet に置き換え、旧構文だけの対にする。CS 等の未使用宣言と契約は保持する。
全模型の FIFO は既存 `policy fifo tie_by case_id`。新しい方策 DSL は要求しない。

## 2. R01〜R09 の工程和

charge [0,1)、query [1,3)、判断 [3,6)、出荷 [6,7)。
返金側は起票 [6,7)、承認 [7,8)、返金 [8,9)、任意の記録 [9,10)。
resolve は時刻・占有・work の増分0、元 Receipt は unknown のまま。

| ID | 計算 | 証拠 / 判断 / resolve | 最終観測時刻 / work | End elapsed / work | charge / refund / 総commit | completed / unfinished / unknown |
|---|---|---|---|---|---|---|
| R01 | 1+2+3+1 | 3 / 6 / 6 | 7 / 7 | 7 / 7 | 1 / 0 / 1 | 1 / 0 / 0 |
| R02 | 1+2、照会 unavailable | なし / なし / なし | 3 / 3 | null / null | 1 / 0 / 1 | 0 / 1 / 1 |
| R03 | 1、handlerなし | なし / なし / なし | 1 / 1 | null / null | 1 / 0 / 1 | 0 / 1 / 1 |
| R04 | 1+2+3+1+1+1 | 3 / 6 / 9 | 9 / 9 | 9 / 9 | 1 / 1 / 2 | 1 / 0 / 0 |
| R05 | R04+1 | 3 / 6と10 / 10 | 10 / 10 | 10 / 10 | 1 / 1 / 2 | 1 / 0 / 0 |
| R06 | 1+2+引継1 | なし / 判断なし、引継4 / なし | 4 / 4 | null / null | 1 / 0 / 1 | 0 / 1 / 1 |
| R07 | 1+wait2+2+3+1 | 5 / 8 / 8 | 9 / 7 | 9 / 7 | 1 / 0 / 1 | 1 / 0 / 0 |
| R08 | 1+duplicate1+2+3+1 | 4 / 7 / 7 | 8 / 8 | 8 / 8 | 1 / 0 / 1 | 1 / 0 / 0 |
| R09 | 1+2+3+起票1+否認1 | 3 / 6 / なし | 8 / 8 | null / null | 1 / 0 / 1 | 0 / 1 / 1 |

R08 は同じ案件・keyで2試行。二回目のcommit0、handler入口1回。
R01/R04/R05/R07/R08 の receipt_resolution は完全網羅で satisfied。
他4行は inconclusive、truncated=false。queryや試行を流入へ加えない。
全行で arrived=received=1、ChargedPerArrival の分母1、真値到達の期待件数1。
未解決でも既に到達した charge 真値を撤回しない。

root の既存 fold も固定する。返金操作を呼ばない R01/R07/R08 には
未活性の返金 invariant/separation/authority_basis があるため vacuous/exit5。
receipt_resolution=satisfied を root=satisfied と誤読しない。
R04/R05 は全契約が活性で satisfied/exit0。未解決4行は inconclusive/exit2。
これは ADR の root通常fold に従う入力依存の値で、回復機能の成功率ではない。

## 3. 人の占有と訂正一覧

Psp=[q] は Actor 単位の割当である。queryだけでなくchargeとrefundにもqを使う。
人の占有はTask区間の和で、Actor workにもう一度加算しない。

| ID | 機械Pspの c / f / 人分和 | Psp=[q] の q の式 | q / 人分和 |
|---|---|---|---|
| R01 | 3 / 0 / 3 | charge1+query2 | 3 / 6 |
| R02 | 0 / 0 / 0 | charge1+query2 | 3 / 3 |
| R03 | 0 / 0 / 0 | charge1 | 1 / 1 |
| R04 | 4 / 1 / 5 | charge1+query2+refund1 | **4 / 9** |
| R05 | 5 / 1 / 6 | charge1+query2+refund1 | **4 / 10** |
| R06 | 1 / 0 / 1 | charge1+query2 | 3 / 4 |
| R07 | 3 / 0 / 3 | charge1+query2、wait占有0 | 3 / 6 |
| R08 | 3 / 0 / 3 | charge2+query2 | 4 / 7 |
| R09 | 4 / 1 / 5 | charge1+query2、返金なし | 3 / 8 |

訂正した箇所は2行。ADR 0065 の5.2節にあったR04/R05のq=3と和8/9は
refund1の数え落としである。2026-11-11の裁定により、模型を変えずに
q=4と和9/10へ訂正する。ADR採択追記11.1節にも記録し、原文は残す。
R01と機械Pspの3行の値に訂正はない。

## 4. 待ちと二案件 QueryDesk

R01/R04/R05: 全Task待ち0、lead区分 in_service=7/9/10、他0。
R07: in_service7+intentional2=lead9、work7。
ReceiptUnknownの完了leadとその分解はnull/no_terminalであり、horizon dwellでない。

二案件同着、Psp容量2、QueryDesk容量1、queryだけuses1、FIFO case_id順。
両charge=[0,1)。同時刻1の終了と次の開始は全順序を展開するため、
case2のqueryがcase1のcharge終了の管理より先にQueuedとなる順もある。
FIFOはまだQueuedでない作業を予約しない。以下の両順序を計算する。

| 最初のquery | case1 query / CS / 出荷 / End / query待ち | case2 query / CS / 出荷 / End / query待ち |
|---|---|---|
| case1 | [1,3) / [3,6) / [6,7) / 7 / 0 | [3,5) / [5,8) / [8,9) / 9 / 2 |
| case2 | [3,5) / [5,8) / [8,9) / 9 / 2 | [1,3) / [3,6) / [6,7) / 7 / 0 |

CS容量2、別々の人c/q、両Shop容量2。各案件のEndはmin7/max9、
query待ちはmin0/max2、in_serviceは7、queue.resourceはmin0/max2。
どちらの順序でもwork各7、全体14、charge commit計2である。
Psp work=charge2+query4=6、CS work=6、Shop work=2、和14。
人c/qは各3、Finance0、Pspは匿名なので人分和6。
Desk占有=2+2=4。先頭案件はlead7=in_service7+queue.resource0、
後続案件はlead9=in_service7+queue.resource2。先頭がどちらの案件でも同じ式となる。
Psp queue積分=1人相当×[1,3)長2=2、観測終了9なので両順序とも平均長2/9。
同時刻3は半開区間。別案件のkey・証拠参照を交換したwireは拒否する。
原案のcase1先行固定は誤りであり、同時刻の順序を残す訂正である。

同時刻順序の他の手値の棚卸し: R01〜R09とT01〜T07、確率D/UD/UU、
人付きPsp対はすべて単一案件の逐次依存で、終了前に後続を開始できない。
T01/T02/T07の期限一致は包含境界であり、任意の別Taskの先行を仮定しない。
T04/T05/T06の停止時刻はquery/判断の区間内部にある。二構成比較と感度の各点も別実行である。
この棚卸しではDesk以外に同時刻の別案件の終了・開始順を固定した手値はない。

## 5. T01〜T07

| ID | 計算と判定 | 占有の再計算 |
|---|---|---|
| T01 | R01のEnd7。到着+7を含めfulfilled、+6はviolated。resolve6で治さない | R01と同じ、c3、Psp work3。人付きならq3、和6 |
| T02 | CS判断finish6。到着+6はfulfilled、+5はviolated | R01と同じ |
| T03 | R05のrefund commit9≠End10。C2 by9はviolated | c5/f1、Psp work4。人付きならq4、和10 |
| T04 | clock2。charge1+query[1,2)1=観測work2。証拠なし、truncated | Psp2、CS0。人付きならq2。完了workのpublic極値はnull/truncated |
| T05 | horizon2、clock20。同じ観測work2、cut_off、truncated=false | T04と同じ物理占有。ただし業務打切りであり計算切断ではない |
| T06 | clock4。charge1+query2+CS[3,4)1=観測work4。証拠3あり、判断/resolveなし | Psp3/CS1。人付きならq3/c1、和4 |
| T07 | 到着4。charge[4,5)、query[5,7)、判断[7,10)、resolve10、End11。elapsed/work7。到着+6=10を含めfulfilled | R01の平行移動なのでc3、Psp3。人付きならq3、和6 |

いずれもunknown起算C5の実装ではない。到着起算の暫定契約である。
T03の人付き占有は3節の訂正を適用したR05で、期限追加による別の占有変更ではない。
T01〜T07の元表の時刻/work/分類には新たな食い違いを見つけていない。
cutの物理workは私有観測seam、完成指標の撤回は公開Reportで別々に検査する。

## 6. 確率の独立対

二試行G、独立喪失p=1/10、backoff0。
D=(1-p)=9/10、UD=p(1-p)=9/100、UU=p²=1/100。質量和1。
D: charge1+通常出荷1=t2/work2/Task2。
UD: charge1+duplicate1+通常出荷1=t3/work3/Task3。
UU available: charge1+duplicate1+query2+CS3+出荷1=t8/work8/Task5。
どれもcharge commit1、query/判断/出荷commit0、最終unknown0。

- E(work)=E(elapsed)=18/10+27/100+8/100=215/100=43/20。
- E(Task)=18/10+27/100+5/100=212/100=53/25。
- E(attempt)=1+p=11/10。
- E(query work)=p²×2=1/50、E(CS work)=p²×3=3/100。
- Psp E(work)=11/10+1/50=28/25。Shop E(work)=1。
- 人付きPspならq=28/25、c=3/100、和115/100=23/20。refundなし。

UU unavailableはcharge2+query2=t4/work4、未解決。D/UDは不変。
completed=99/100、unknown=1/100、E(observed work)=180/100+27/100+4/100=211/100。
Psp E(work)=28/25、Shop E(work)=99/100、CS=0。和211/100。
人付きPspならq=28/25、c=0。完了条件付き時間を全流入時間と呼ばない。
p=0/1/10/1/5の有限点でunavailable最終unknownは0/1/100/1/25。
この対にもADR手値との食い違いを見つけていない。
simulationはこれらとの完全一致を要求せず、固定4反復の確定的な端点対を別に使う。
同反復の案件を独立標本として水増ししない。

## 7. 診断と負対照

ADR 0065 の7.1節の順に固定する。messageはreason単独またはreasonにコロンを
続ける形。自由文の包含一致やexit4だけで別gateの拒否を検出したことにしない。

| 条件 | code | reason |
|---|---|---|
| 新形の欠落・閉鎖なし | E-SYN-0 | invalid_recovery_syntax |
| 未知の参照 | E-NAME-0 | unknown_recovery_reference |
| 証拠型・既知性・支配・元Ackの値読取り | E-TYPE-0 | recovery_evidence_mismatch |
| latentの条件読取り | E-POL-LATENT | 既存理由を保存 |
| 保護observed状態の漏洩 | E-TYPE-0 | recovery_provider_state_leak |
| 重複handler/resolve、解決済みstop、未解決end | E-CON-0 | invalid_recovery_contract |
| 正確なG照会対応契約を満たさない | U-SEM-0 | recovery_query_unmodeled |
| B、新response等、contest/quantile | U-SEM-0 | recovery_feature_combination |
| 字句query33、上限超過 | U-SEM-0 | recovery_expansion_limit |
| 対応外の文法 | U-SYN-0 | 既存理由 |

公開IRはU-SEM-0/invalid_recovery_ir、referenceは独立SemanticAbort。
wireはdecode error。正当な末尾unknownはinconclusiveでありunsupportedでない。
N01/N04/N05はDSLと私有IR/実行/再生の両方、N06/N12/N13は厳密wire、
N02/N03は潜在と保護状態、N07はwriterと実履歴、N08は通常契約、
N09は専用結果源、N10/N11は交差拒否、N14は感度の全点実行前gateに対応する。
各正対を先に受理してから故障対を渡す。現HEADの構文拒否はこれら意味gateの校正ではない。

## 8. 独立受入が固定する新wire

ADR U10に基づき以下を固定する。旧Receipt集約の鍵は変更しない。
局所版 `response-recovery-p3a-1`、bundle `relca-conclusions-p3a-recovery-1`。
handlerなしではresponse_recoveryは省略またはnull、handler有りの実行ではobject。
全objectはclosed、下記全鍵が必須。nullableは欠落と同じでない。

| object | 必須鍵と型 |
|---|---|
| response_recovery | rules:string、coverage:complete/partial/sampled、scope:string、paths:array |
| paths[] | id:非空string、commands:array |
| commands[] | case_id:正整数、command:string、provider:string、capability:string、saved_key:CaseId整数、original_site:Span、queries:array、resolutions:array、tasks:array、unresolved:bool、stop_reason:閉じたenum |
| queries[] | id:非空string、binding:string、site:Span、at:整数、status:三値、evidence:nullable string |
| resolutions[] | query_ref:string、site:Span、at:整数、fact:committed_once/not_committed |
| tasks[] | binding:string、capability:string、executor:nullable string、result:nullable string、started_at:nullable整数、finished_at:nullable整数、site:Span |

Spanは既存の半開UTF-8 byte start/end。scopeはr2_hand.rsのSCOPEと完全一致。
stop_reasonはend/receipt_unknown/truncated/cut_off/state_range/semantic_abort/deadlock。
canonical順はjoint pathを正規化した順、commandsはcase_idと元site順、query/resolve/taskは
時刻とsite順。同じ観測列のschedule重複は正規化してよい。idの綴りに実装内BindingIdを要求しない。
異なるjoint path間の参照は禁止。同じpath内でも別case/command/key/providerの証拠を使わない。
evidenceは確定queryのidと同じstring、still_unknownなら明示null。
resolutionはquery_refを同じcommand内の先行queryへ結び、factをstatusと照合する。
unresolvedと末尾分類、case_outcomes、receipt_resolution、coverageを照合する。
tasksはhandlerで実際に作られたquery/業務Taskであり、元試行やresolveの架空Taskを足さない。
途中cutのTaskは開始/終了のnullを保持し、未観測query結果を発明しない。

集約Rangeを経路へ継ぎ合わせない。pathsはjoint path単位の観測で、成功率表ではない。
Report読戻しはsource/PSP真正性の証明でなく自己整合だけを検査する。
/response_recoveryをledger subject、/response_recovery/rulesをcomponentとして登録する。
比較rootは実際の子ledgerへ結ぶ。局所版更新をpackage版更新に読み替えない。
回復入力のresponse_analysis.rulesはresponse-p3a-recovery-1。
checkedへexact_g_query/recovery_evidence_matching/recovery_incomplete_caseを足し、
uncheckedの旧5項目にnon_idempotent_recoveryを足す。
handlerなしのresponse-p2-1の範囲をそのまま一般照会対応へ書き換えない。
回復Explainのversionはinvoice-counterexample-p3a-recovery-1。
Explainの新event kindはreceipt_query/resolve_effect、commandとlocationを保持する。
通常Task finishのstatus3/decision6と別に出し、referenceで再生する。

## 9. 私有seamの接続（まだ実行済みではない）

非公開試験の接続にはsrc/lib.rsにcfg(test)とpath属性のmodule宣言だけを足す。
`#[path = "../tests/support/r2_private.rs"] mod r2_private;` をcfg(test)にする。
同じcrate rootにcfg(test)の `r2_test_adapter` moduleを実装する。公開APIを広げない。
関数は以下。返すValueは実物の内部観測の薄い射影に限り、期待値のコピーを返さない。

- project(system:&str, scenario:&str) -> Value
- validate_ir(system:&str, fault:&str) -> Value
- resolve(system:&str, fault:&str) -> Value
- replay(fault:&str) -> Value
- cut(system:&str, scenario:&str) -> Value
- history(system:&str, scenario:&str) -> Value
- probability(system:&str, available:bool) -> Value

systemはengine/reference。各々の実物を呼び、動的意味関数を共有しない。
projectは空/存在集合、開始後挿入、別案件keyのみを手組み状態で正常終了まで動かす。
空集合not_committedはEC到達枝でなくこの射影単体のoracle。
validate_irはlower済み正対から指定の一欄を破壊し、実engine入口と独立reference入口へ渡す。
key_deleteはrefundからG集合の保存keyを削除する実効果を手組みIRに加える。
DSLの対はrefundのwritesへ保護G集合を加え、宣言上の別writerを拒否する。
未対応のcase.id効果式を混ぜて、構文拒否をwriter拒否の校正にしない。
resolveは未解決payとotherの2commandを手組みし、payだけ閉じotherを残す。
replayは実queryとresolve遷移を記録/改変してreference再生器へ渡す。
cutは停止までの実Task区間を観測。historyは本模型の実commitと人物区間を数える。
probabilityは質量つき実経路から実Task/workを足す。経路数やschedule数を確率にしない。
adapterは不正IR拒否、query projection、resolve照合、再生検査を各々外すproduction故障で
assertionが落ちることを実装時に校正し、supportの固定shaを変えない。
未接続の6テストをGREEN/REDに数えず「未接続」と明記する。

## 10. CLI/goldenと残る検証

新CLI targetはraw stdout一行JSON、stderr全診断とprefix、exitを検査する。
r2_semantic_golden.jsonは上の工程和とroot foldから手で書いた意味射影の完全一致表。
未対応の現binary stdoutをgoldenへ写していない。
これはADR0055の全raw stdout/stderr snapshotの置換ではない。
実装後の現行raw snapshotへの登録・更新は別途行う。既存snapshotは本作業で変更しない。
受入のREDは機能通過でも、全負対照の実行済みでもない。loop内では最初の失敗より先の
subcaseは実行されないため、検証記録ではnamed testの観測と意味gate未実施を区別する。
