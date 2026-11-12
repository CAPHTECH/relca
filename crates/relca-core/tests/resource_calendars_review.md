# Q4 レビュー追従の受入と手計算

基点 9b8c12aa。根拠は ADR 0059 の採択契約と、Q4 レビューの M1 / m1 / m2 / m3 の裁定。
production、既存模型、snapshot は変更しない。この記録の値は区間と契約から計算したもので、実行結果から採取しない。
試験は core / 実 CLI の `resource_calendars_review`、共有の入力と独立照合器は `tests/support/q4_review.rs`。
旧 `q4_oracle.rs` の手値は保持し、M1 の property 行の照合だけを拡張する。

## M1: 全 wire の arrival と標本

ADR 0059 の C11/C12 と既存の独立 SHA 台帳を使う。seed 1 の4反復の支持列は第1、第1、第2、第2。
case1 の到着は [0,0,0,0]、Sample は mean/min/max=0、samples=4。
case2 は [0,0,4,4] なので mean=2、min=0、max=4、samples=4。
burst の一変更は第2列の4を1にするだけで、[0,0,1,1]、mean=1/2、min=0、max=1。
calendar の追加は到着の抽選を変えない。旧 lead / wait / hits / trials は全て不変。
`case_outcomes` と `metrics.cases` に加えて `properties[].cases[]` の両案件に、arrival の明示 null とこの厳密な Sample を要求する。
到着が常に0のcase1も、S1集約である以上 arrival=0 に戻さない。

負対照は手組みwireに対する単一故障である:

- property の case2 arrival だけを最初の支持列の整数0へ戻す。
- property の case2 arrival_time だけを削除する。
- property の case1 arrival の null キーだけを削除する。
- property の case2 Sample だけを最初の支持列の Sample0へ戻す。

各々を独立照合器が拒否する。production への故障注入を実施したという意味ではない。
既存 snapshot の q4-finite-arrivals もこの拡張 oracle を通るので、誤った property 整数を許さなくなる。
snapshot の採取と手計算の照合は別の検査である。

## m1: 停止済み案件に Queued Task が残る経路

### 模型と手値

`CANCELLED_QUEUE` は Actor Desk 物理1、常時表1、資源R物理1は [0,5) で表0、以後1。
短枝 a は1分、長枝 b はRを1単位使い2分。到着は [0,2]。
join any は a/b の ok を採用、残りは cancel_then_drain。

- 案件1: a=[0,1)、bは時刻0からqueuedだがR閉場で未開始。a採用時刻1でbは取消される。
  End1/work1、b starts0/work0。
- 案件2: a=[2,3)、bは時刻2からqueued。時刻3で取消。End3/work1、b starts0/work0。
- 単一変更で remaining drain にすると、案件1のb=[5,7)、End7/work3。
  案件2のaは時刻2でreadyだが、案件1のbがFIFO先行者であるため追越し不可。
  a=[7,8)、b=[8,10)、End10/work3、両案件のb starts1/work2。
- plain FIFO と observable/call_order/first_idle の両方で同じ手値を要求する。
  各 interpreter を手値へ別々に照合した後、Outcome 全体を比較する。

第2模型は既存 G形の Q1 retry 模型から、応答を1回の unknown（対は delivered）にし、
到着 [0,2]、cases2、plain FIFO にする。Ledger表は [0,1) が1、[1,4) が0、以後1。
案件1は [0,1)、応答1。案件2は時刻2にready、[4,5)、応答5。
unknown側は各案件 ReceiptUnknown（成功Endではない）、delivered側は posted のEnd1/5、work各1。
unknown で止まった案件1が、案件2の時刻4の開始を妨げないことを両 interpreter で固定する。
応答列 unknown→delivered だけを替える非no-opの単一変更対である。

### 到達しない根拠と限界

6構成（取消/ドレイン×2方策と、unknown/delivered）では両 interpreter の一致を観測する。
停止済みかつ queued が残る状態そのものを公開Outcomeから観測したとは主張しない。
以下は実装本体からの帰納的な根拠 [R] で、現在lowerが受理するDSLに限る。

1. engine の停止判定は `src/engine/arrivals.rs:190-208`。
   通常Endでcontestが無い場合、End前の各call/forkは `:2087-2111` の全通常Task Done条件を通る必要がある。
   call/forkのない分岐で新しいQueuedは作られない。if_none終端も同じDone条件の内側にある。
2. join any の採用自体は継続の解放ではない。取消前のQueuedは
   `src/engine.rs:9112-9115` でDoneになり、drainは終了まで待つ。
   contested Endの停止判定は全Task Doneを明示的に要求する（上記`:203-208`）。
3. Q1の停止は `src/engine/response.rs:224-252` のReceipt位置だけ。
   これは `src/engine.rs:3183-3195` の通常のcall待ちの次であり、
   その前に全通常Task Doneと通常Taskの除去（arrivals `:2087-2111`）を経る。
   responseをforkの兄弟に置く構文は `src/lower.rs:9237-9247` で拒否。
   contestとの同居も同`:5641-5650`で拒否されるため、別のQueued routeはここに混入しない。
4. 参照側も `src/reference/arrivals.rs:2263-2288` で全待機Taskのterminalを確認してから継続する。
   `src/reference.rs:2368-2372` のterminalにQueuedは含まれない。
   Q1停止前にも `src/reference/response.rs:230-243` で単独TaskがFinished/Refusedであることを要求する。
   contested Endの停止は `src/reference/arrivals.rs:39-65` の全Task terminal条件による。
5. range停止や計算切断は世界の探索停止であり、同じ世界の後続案件を開始判定する局所stopped状態を作らない。
   horizonも同じ業務観測の打切りで、停止済み案件のQueuedを残して後続を新規開始させる契約ではない。

したがって現在の通常DSLでは、問題の先行者集合の違いを発火させる停止済みQueuedへ至る経路を見いだしていない。
上記6構成の差分試験はこの境界を守るが、全状態の私有adapterによる不変量検査や、任意の偽造IRに対する証明ではない [U]。
fork内response、drain無しのEnd、動的な局所中断を将来認める場合はこの根拠を再評価する必要がある。
productionの先行者集合は policy::starts と同じく停止済みも含める必要がある。

## m2: 条件待ちの責務の決定支持は未導出

Reader.enter1分がInvoice原本を読み、Approver.approve(entry)1分が pass/fail の観測結果を返す。
その束縛の直後に wait until approval == pass、続けて Ledger.post1分。
責務は a1 for Approver.approve と a1 for Ledger.post の2行。
Ledgerへの引数無し（待ちだけの依存）と、approvalを引数に渡す（データ依存もある）を両方置く。

- passなら待ちは即時通過、post=[2,3)、End3/work3。
- failなら束縛済み結果は後で変わらず、時刻2以後のwriterも無い。postは開始しない真のdeadlock。
  reachable Endの証人は残るが、deadlockを成功にしないのでroot violated/exit1。
- いずれの責務も条件待ちを無視した確定的な決定支持を出さず、refusal.status=not_derived、
  conditionを識別する非空reason、effects=[]を要求する。単にuncheckedを足して既存effectsを残すのは不可。

単一変更対は wait until を `if approval == pass {} else { end returned }` へ置換する。
Ifはpassならpost、failならreturnedであり、従来の制御依存により Approver.approve の責務から Ledger.post の効果への
all_pathsの判定支持が導かれる。両引数版で refusal present/reason null/effect all_paths を維持する。

既存実装の `src/contest.rs:218-219` はlexical control dependenceの説明であり、
`:262` はWaitUntilを黙って無視する実装である。`src/responsibility.rs:1509-1511` もWaitとWaitUntilをまとめて無視し、
ADR0026を注記する。しかし ADR0026:41 は固定waitの規則であり、条件が永続に偽なら後続を止めるwait untilを
制御依存から除外してよい根拠ではない。上記投影でもこの相違を維持する。
この無視を正しい既存契約として追認せず、ADR0059の未導出への撤回に従う。

負対照は2責務それぞれについてstatusをpresentに戻す、reasonをnullへ戻す、effectsに確定行を戻す単一故障。
合計6対を手組みの正例から作り、全て拒否する。実装への故障注入ではない。

## m3: scenario.drain の宣言区間は全支持列の包絡

scenario DayにS1を移し、[0,0]/[0,4] を各1/2で宣言する。
宣言全体の最後の到着の最大は max(0,4)=4分。反復で第1列しか観測しなかったとしても0分にはならない。
horizon無しの仮定は `4min..all_terminal_or_clock`、または明示not_derivedとarrival distributionを特定する理由。
支持列を逆順にしても同じ4分。第2列だけ[0,9]へ変更する対ではmax(0,9)=9分。
全試験は両順序/一変更の3入力を最後まで実行し、失敗をまとめる。第1入力でpanicして残りを未実施にしない。
負対照は4分を最初の行の0分へ替えるものと、未導出の理由だけを削除するもの。

## 保証の範囲

REDは実装修正前の受入checkpointであり、全段GREENや取り込み承認ではない。
実行結果・試験名・hashは作業報告に別記する。S1の全交差、全私有状態、全snapshotの再採取を完了したとはしない。
