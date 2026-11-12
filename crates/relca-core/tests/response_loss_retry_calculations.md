# ADR0056 受入の手計算と単一故障の対

2026-11-02。作成時のproductionは7806d990のまま(文書commitを除く)。
この期待は実装出力を採取していない。手計算と試験の実行結果は別であり、実行の成否には別の証拠が必要である。
通常のfixtures群を増やさず、core/CLIのresponse_loss_retry targetから同じ閉じた手入力を使う。

入力: tests/support/response_loss_inputs.rs、期待: tests/support/response_loss_oracle.rs。
RUNTIMEは28模型、STATICは18模型。新しい模型の有効構文をproductionに通したとは主張しない。
既存の旧構文基盤、K8の障害なし対、比較器の負対照は新機能のREDと独立にGREENを要求する。

## 1. サービスの最小模型

CaseId=1、count初期0(0..2)、registered初期false、postは容量1・1分。
Gのkeysは初期空集合。Gのcommit枝だけでkeys挿入、registered=true、count+=1。
Gの拒否枝は書込みなしのrejected_duplicateで、正常finishだがcommitしない。
具体的非冪等はeffect_scope modeled、毎回registered=trueとcount+=1、毎回commit。
Rは同じ有限の状態を持っていてもeffect_scope宣言も構造的機構も無く、再送安全の問いはunsupported。
Actorの名前・grainでは三者を区別しない。

| 入力 | 観測/時間/work/状態の手値 | 変化を観測する契約 |
|---|---|---|
| g-resend | t1 commit1/count1、Receipt unknown。t2は拒否、受信duplicate。End duplicate、elapsed2、work2 | once satisfied、registered到達1、count2到達0 |
| modeled-resend | t1 commit1、応答喪失。t2 commit2/count2、受信committed。ただし最初の未知効果は未解決 | once violated、count2到達1、root violated/exit1。正常Endは無い |
| modeled-first-delivered | 第一Receiptだけunknown→delivered。t1に正常End posted、count1/work1、2回目を実行しない | once satisfied、count2到達0 |
| g-first-delivered | 第一Receiptだけdelivered。t1 posted/work1/commit1 | 再送が無くても上限を毎回消費しない |
| g-unknown | 第二Receiptだけdelivered→unknown。t2までwork2、count1、正常End0 | 専用ReceiptUnknown、root inconclusive/exit2、truncated=false |
| single-unknown | post一回でcommit1/count1、t1/work1、binding非公開、正常End0 | registered到達1、once satisfied、ReceiptUnknown1 |
| single-delivered | 同一模型でReceipt一語だけdelivered。t1 posted/work1 | 正常完了1、unknown0 |
| g-label | Ledger→Book、grain=document | 名称以外はg-resendと同値。時刻・once・Receiptの値は変わらない |

requires-refusalはregistered==trueを前件とし、初期falseなので未開始のrejected_duplicateを即座に返す。
時間0/work0、commit0、once vacuous、registered到達0によりroot violated。応答喪失は起こらない。
requires-allowedはこの前件のtrueをfalseへ一語だけ変え、t1の実効果とunknownが発生する。
これは前件の拒否をサーバ正常終了やReceiptUnknownと混ぜない対。

unknown_blocks_downstream_taskは7分のAfter.runをsuffixに足す。
unknownならAfterを開始せず観測1/work1で止まる。単語deliveredに変えた対は1+7=8分/work8。
実際のサーバ結果を一度でも業務bindingへ漏らすとこの対が落ちる。

## 2. V01-12 再試行相関

開始時のbad=trueが1/10、falseが9/10。一度選んだbadを再試行で引き直さない。
Kはbadなら確率1でfail、goodなら確率1でpass。応答喪失は明示none。
総試行2、fail時retry、最後はEnd failed。

| 経路 | 質量 | Task数 | elapsed | work |
|---|---:|---:|---:|---:|
| good→pass | 9/10 | 1 | 1 | 1 |
| bad→fail→fail | 1/10 | 2 | 2 | 2 |

したがって失敗は(1/10)×1×1=1/10であり、1/100ではない。
期待Task数/elapsed/work=(9/10)×1+(1/10)×2=11/10。
correlated-tightは確率上限だけ0.1→0.01。一つの閾値の故障でviolated/exit1になる。
backoffは0→3分の一変更。failedのelapsed=1+3+1=5、work=2、期待elapsed=9/10+5/10=7/5、期待work=11/10。
各経路のelapsedとworkを直接照合し、根拠の無い平均の出力欄は要求しない。

independentはkernelの定義だけを毎Taskのfail=1/10/pass=9/10の無条件分布に変えた、独立性を明記した検算対。
badの初期潜在は残すがKの結果へは作用しない。
経路はP=9/10(時間1)、FP=9/100(時間2)、FF=1/100(時間2)。okの時間とworkの範囲は1..2、failedは2。
この値を相関模型の期待へ写さない。旧構文へsuffixだけを除いた基盤でもこのKが有効であることを別に検査する。
known-fail/known-passはbadの初期分布だけを1/0または0/1へ変えた境界対。
known-failは時間2/work2で明示exhausted先、known-passは時間1/work1で止まる。

## 3. V02-14 後半と前半の保存

fixtures/k8-cut-r/model.relca、fixtures/k8-cut-g/model.relcaは無編集でincludeする。
前半の値は抽出1+post1=2分、work2、誤登録0.02=1/50。Rだけmechanism_unmodeled注記がある。

後半はそのpost callに応答喪失/再試行suffixを一つ付加する。抽出は再実行しない。
G: 抽出終了t1、初回commitとunknownがt2、再送拒否受信がt3。End doneはt3、work3。
誤登録=(1/50)×1=1/50、registered到達1、once satisfied。第一Receiptはunknownのままである。
R: 同じsuffixでU-SEM-0、response_loss_resend_mechanism_unmodeled、exit4。架空の二重登録数値は要求しない。
Rのat_most_onceを削るだけではこの門を回避できないことも別入力で検査する。

第二Receiptだけunknownへ変えたk8-g-unknownは実commit1のまま、観測t3/work3、正常End0、ReceiptUnknown1。
completed_at/完了elapsed/workはnull、観測量は別欄。registeredと誤登録の分析者用性質は依然判定できる。
clockだけ20→2にしたk8-g-clockは第二終了が時計外であり、truncated=true/exit2。
通信による最終unknownはtruncated=falseで、時計切断と違う理由である。

## 4. 非決定と根拠付き確率

Gの上限2で応答D=delivered、U=unknownとする。
D: t1 posted/commit1。UD: t2 duplicate/commit1。UU: t2 ReceiptUnknown/commit1。
Dの後の第二Taskを実行しない。既定non-detはこの全選択を残す。0.5で埋めず、Receipt行に確率を付けない。
g-defaultは明示nondetだけを省略し、同じ選択を残す対。

固定p=1/10を検算用の根拠・各正常終了を条件にした独立性とともに宣言したときだけ、
P(D)=9/10、P(UD)=(1/10)(9/10)=9/100、P(UU)=1/100。
合計1、既知の決着99/100、unknown1/100。UDはduplicateの受信であり、posted成功99%とは呼ばない。
期待Task数=9/10+2×9/100+2×1/100=11/10、実commitは全経路1。
pだけ0に変えると第一受信で停止、pだけ1にすると全Receipt unknown。
basisの削除、空文字、pだけ1.1にする対はそれぞれinvalidである。
simulateはnon-detをU-SEM-0で拒否。p=0/1の200反復の境界対は、決定的な期待200件を確認する。
中間pの標本を特定seedで99%に固定する試験ではない。

## 5. 指標と品質

single-unknown/deliveredに同じmetricsとquality宣言を足した対を使う。
料金はActor1円/分、post開始2円なので、観測費用1+2=3円。work1は消さない。
unknownでは全流入1・受領1・completed0・unfinished1・その内数receipt_unknown_count1・deadlock0。
deliveredではcompleted1・unfinished0・receipt_unknown_count0。
registeredの真値への到達は両方1。RegisteredPerArrivalは分母1を残し1/1=1。
品質のoutcomes_per_arrivalにreceipt_unknownという独立行を置き、その率はunknown対で1、受信対で0。
unknownをunfinishedと別に足して流入2件へ水増ししない。
未確定の業務完了を成功へ丸めず、登録の実効果の解析まで一律nullへ撤回しない。

## 6. 有限上限・交差・静的拒否

max-oneは上限1が初回を含むことを確認し、1分のunknownで止まる。
max-thirty-twoはbad経路が32分/work32で有限に終わる。33へ一語変えた対はU-SEM-0/retry_expansion_limit。
0はE-CON-0。sequence不足/余剰、exhausted/backoff欠落、偽のidempotency宣言、別集合を読む機構、開始効果併存を個別に拒否する。
fork retry、version_check retry、compare/diffの未対応交差はU-SEM-0であり、表を空にして成功へ通さない。
複数案件は到着0/4、各案件のpost2回。各elapsed2/work2、絶対終了2/6。
案件2を案件1のkeyで拒否したり、一方の既知Receiptで他方のcommandを解決したりしてはならない。

## 7. Mutation

明示response_loss none・retry無しの逐次callだけを対象とする。
command_resentは初回unknown/次delivered・総試行2・backoff0への一通信故障の編集。
Gのonceは保持するのでundetected/no_worseningとheld証拠、非冪等countはonce違反をdetected、Rはinvalid_mutant/static_unsupported。
Rの変異体でU-SEM-0/reasonを実際に確認し、未対応を検出に数えない。
receipt_unknownは一回unknownへ編集し、unknown1・正常End0・効果保持・完了非加算という保持をdetected/heldにする。
元模型と手編集の同じ手値を、変異体内Reportにも当てる。

未宣言、既に応答指定、既存retry、fork枝を各一個持ち、明示noneの逐次callも一個ある模型で、
各種のskipped_countsをundeclared_site1、existing_response_loss1、existing_retry1、fork_branch1と固定する。
skipped_sitesから独立に数え直して一致させる。一siteの重複計上も、単なるnot_applicableへの隠蔽も不可。
idempotencyだけの未宣言callも除外する。未到達siteのReceipt注入はno_assigned_checkでありheldではない。

時計1分で元の1回postは完了するが、再送の第二Taskは切断される。これはinconclusive、検出でない。
別の一時計故障対ではpostを3分、clockを1分とし、初期状態で破れるstate invariantを共通に持たせる。
元模型は違反witnessと計算切断をともに持つのでMutation生成を試せるが、Receipt unknownの変異体も完了前で切断する。
このReceipt種自身もinconclusive、assigned_changesにheld無しとし、専用例外で切断を成功へ丸めない。
予算対は到達しない129の宣言済みcallと一つの原本readsを持つ。
既存Evidence/original_access_removedの2試行を先に実行し、新再送の126試行までで128を使い切る。
残り再送3件とReceipt129件をnot_runで列挙し、run inconclusiveを要求する。

## 8. 検査の校正と未確認の境界

PAIRSの22対は、指定した文字列の1回置換で完全一致することを検査する。別の故障を同じ対に混ぜない。
比較器は手組みの専用outcomeを正として受理し、kindをEndにする、completed_atを既知にする、
truncatedにすり替える、観測workを消す、必要なnullキーを削除する各1変更を拒否する。
確率比較器は1/10を1/100へ、1/100を1/2へ変えた誤期待を同じ比較経路で拒否する。
これらは受入比較器の校正であり、productionに故障を注入して検出したという報告ではない。

公開Outcomeの手値はengineとreferenceへ独立に当てる。新Receipt内部状態の全履歴、
偽造IR、Hashの不足、replay改竄、全scheduleの直積は公開APIだけでは網羅していない。
残る実装側の非公開検査はresponse_loss_retry_obligations.mdに列挙する。
