# P3a R1/R4 の独立手計算と接続契約

基点 2be18961。要求は REC-p3a の R1/R4 と P3a checkpoint 裁定。
追加 wire の選択は P3a 補足1により受入側で固定した。
出力・snapshot・旧 golden を期待値に写していない。
既存 Q1 入力は宣言だけを再利用し、二つの process/configuration を新しい入力で作る。
実行結果、故障校正、固定 sha256、未接続 seam の観測は、この手計算だけからは主張しない。

## 保存するものと意図的な変更

- サーバの実効果、受信結果、案件結末を分ける。unknown は正常 End でない。
- 再送は同じ案件と元 command の新しい Task。alias は Task/J/独立根拠ではない。
- 案間で数値 BindingId を照合しない。静的編集は元 command、動的観測は
  (case_id, command, attempt)、結末は (case_id, kind, end) に対応させる。
- compare/diff の一律 retry_feature_combination 拒否を外すが、contest と quantile は拒否のまま。
- compare の字句 End 在庫一致、diff の片側不在 null、Cut 内容差の開示は保存。
- 子の violated は root の violated に fold し、unsupported の子・診断を削除しない。
- 待ちの分解を check/simulate に接続するだけ。比較の待ち表は作らない。
- F/U/集中/責務の未導出を比較可能に変更しない。O/J/E/L は既存の限定内で比較する。

## R1 の経路算術

一つの Ledger.post は1分。G は空の keys に初回だけ挿入して commit し、同じ key は
rejected_duplicate を返す。modeled は各回 count を加算する。初回込み総試行2、backoff0。

|入力|実際の工程|既知の結末|unknown観測|once|root|
|---|---|---|---|---|---|
|g-resend|unknown@1、duplicate delivered@2|duplicate、elapsed/work2|なし|satisfied|satisfied|
|modeled-resend|commit@1 unknown、commit@2 delivered|なし。2回目は初回効果を閉じない|until/elapsed/work2|violated|violated|
|g-unknown|commit@1 unknown、duplicate@2 unknown|なし|until/elapsed/work2|satisfied|inconclusive|
|g-probability|D、UD、UU|posted1、duplicate2|UU@2|satisfied|inconclusive|
|k8-g-resend|Extract1、post1、再送1|done3、work3|なし|satisfied|satisfied|
|k8-g-unknown|同じ3作業|なし|until/elapsed/work3|satisfied|inconclusive|
|k8-g-clock|Extract1、初回post完了2、その後は計算clock2|計算切断|ReceiptUnknownへ変換しない|保留|inconclusive|

k8 の初期不良は1/50のまま。再送は Extract を再実行せず、誤登録の確率を二乗しない。
初回の post で registered が真になるため、registered within2 の確率は1のまま。
g-probability は独立喪失 p=1/10 なので D=9/10、UD=(1/10)(9/10)=9/100、
UU=(1/10)^2=1/100。ReceiptUnknown の完了時刻・elapsed・work・result_at は明示 null、
代わりに observed_until/observed_elapsed/observed_work=2 を輸送する。
二つの同じ案を比べると各列は上の同じ手値になる。exhausted failed は到達しなくても字句 End。
Afterだけには、初期registered=falseで必ず通らないif registered==true内のskipped callを置く。
実開始・時刻・費用・効果は不変だが、元commandのBindingIdはずれる。
この実行しないcallを消さず静的に検査し、BindingIdの数値が等しいことを対応条件にしない。

片側だけ response を外すと、その側には retry の字句 failed が無い。
compare は End 在庫不一致 invalid を保存する。diff は比較を実施し、unknown の from を残し
非 response の to は null とする。不在を観測済み0や確率0に変換しない。
simulation の p=0/1、4反復は乱数の並びに依存せず、各案件の posted/unknown がそれぞれ4。
unknown4は unfinished4の内数。未完了とunknownを足して8案件にしない。

## R1a DTO と静的差分の閉じた拡張

比較の exhaustive terminal value と diff の CaseValue に、既存の Q1 case terminal と同名の
observed_until、observed_elapsed、observed_work を追加する。値は整数 min/max。
既知完了では観測専用欄は非適用、ReceiptUnknown では三欄必須。
ReceiptUnknown は kind=receipt_unknown、end=null、完了四欄=null。
compareのReceiptUnknown行は、少なくとも一案にresponse宣言があれば在庫に残す。
responseがあるが網羅でunknownを報告していないセルはnot_reported/value=null。
response自体が無い側はnot_applicable/value=null。simulationで既知の未観測0回なら
reported/value.count=0であり、三者を区別する。TerminalStatusへnot_applicableを閉じた値として追加する。
プロパティ在庫には自動 receipt_resolution を含める。notes は既存 producer の規則と一致させる。
未知の鍵は拒否し、typed roundtrip で欄を捨てない。

静的 response 制御の差分は **既存 kind=meaning** にする。新 kind は足さない。
理由: result の採用、再試行、停止を変える意味の差であり、Actor の追加や単なる Task 所要時間の差ではない。
target は response.<元binding名>、同一commandの変更は change=modified。
from/to は次の閉じた object。未宣言側は null。古い meaning の bool payload は保存する。

|鍵|型・意味|
|---|---|
|capability|Actor.Capability|
|arguments|宣言順の安定名の配列。ここでは case.id|
|loss|下記のタグ付きチャネル|
|retry|null または max_attempts、when、backoff、exhausted|
|retry.when|receipt_unknown: bool と results: enum値名配列|
|retry.backoff|整数分。elapsedにだけ効く|
|retry.exhausted|字句のEnd名|
|idempotency|key parameterの名前、未宣言はnull|
|effect_scope|modeledまたは未宣言null|

loss の閉じた形:

- none: kind=none のみ。
- 非決定: kind=nondeterministic のみ。確率を作らない。
- 有限列: kind=sequence と receipts=[unknown,delivered等]。
- 確率: kind=probability、probability=厳密な有理数文字列、空でないbasis、
  independence=per_attempt_conditional_on_normal_finish。

この拡張は喪失方式、総回数、条件、backoff、exhausted、Capability、引数、機構の差を
元command単位で示す。Actor/Capability の relation 追加・退役は従来どおり別の行。
未宣言の制御や条件を既定値で補わない。内側の未知の鍵も拒否する。

私有比較 seam は comparison_projection の cfg(test) 子として
support/p3a_projection_tests.rs を path 属性で接続する。公開APIは増やさない。
二案件は到着0/4、unknown観測絶対時刻2/6、elapsed2/work2。metadataだけを既存reportから借り、
比較対象の在庫・数値・Receiptは手組みする。
kind変更、完了量非null、observed_work欠落、案件混同、案件在庫全欠落、receipt_resolution欠落、
notes偽造、別command、aliasの架空command追加、試行番号3を独立の一故障で拒否する。
simulationではunknown0、unfinished0、案件混同、結末行消失、自動性質消失を拒否する。
有効対が通る前の拒否を校正成功に数えない。

## R1b 部分Agency

ReaderがOriginalから作るEntryをCheck.runの入力にし、Checkは独立のEvidenceも読む。
非commitのCheck.runをQ1の有限再試行にし、その既知passの後にPoster.postの実効果を置く。
commit操作だけをJと呼ばない既存規則を保つため、Gのpost自身を判定に見立てない。
再送のJは二つだが、subject_sourcesは両方Original、independent_sourcesの和集合はEvidence一つ。
元command vのaliasを三つ目のJに数えない。
喪失無しでbad初期値が1/10のままなので、Checkの実Start範囲は再試行側1..2、旧call側1..1。
二つの実判定サイトのJ2→1とLのこの差はchanged、独立Evidenceは二倍にしない。
OはCheckの直接Evidence・間接Originalを保持する。
同じresponse側同士でもF/U/集中/責務のnot_derived(response_control)を「差なし」にしない。
左右反転とresponse/旧callの片側対で理由のownerを輸送する。
framing、effects.u、effects.concentration.single_actor、responsibility.refusal はnot_comparable。
O/J/E/L の既知項目は、等しければ行無し、異なればchangedであり、表全体をcomparison_mismatchにしない。
W-AGY-0、W-AGY-2、W-LIN-2、W-RESP-2 の未検査印を保持する。

## R1c 待ちの手値

postは1分、backoff3分、総試行2。半開区間を使う。

|受信列|区間|End/観測まで|work/in_service|intentional|queue|完了|
|---|---|---|---|---|---|---|
|D|post[0,1)|End1|1|0|0|1|
|UD|post[0,1)、backoff[1,4)、post[4,5)|End5|2|3|0|1|
|UU|同上|観測5、Endなし|work2、完了lead分割null|完了lead分割null|観測Taskの待ちは0|0|

流入は全て1。試行をarrivalsに足さない。UUはunknown1、unfinished1、completed0。
per_minute1 + per_start2なのでDの観測費用3、UD/UUは2+4=6。
aliasは実Taskではない。Dの二番目の宣言試行は未実行。全Taskの宣言在庫と実開始回数は別。
UUにhorizon dwellを捏造しない。計算clockと業務horizonとunknownは別の停止理由。
同じ手値をcheck、simulate4反復、および両独立interpreterの量に当てる。

### 二案件の私有履歴

Taskの待ちの原因と案件のlead分割を別々に照合する。
case1はarrival/admitted0、a1=[0,1)、backoffに対応する意図的待ち[1,4)、
a2.ready4/start(4+q)/stop(5+q)。case2はarrival/admitted4、blocker=[4,4+q)。
q=1/2の二つの手履歴を置く。case1はlead6/7、in_service2、intentional3、queue1/2。
半開区間なのでblocker.stopとa2.startが同じでも追加の一分は発生しない。

- actor_capacity:同じActor A、capacity1を共有する。
- executor:別Actor A/B、各executors=[p]。A枠は空でも共有Person pが塞がる。
- resource:別Actor A/B、各capacity1、uses Desk:1、Desk capacity1を共有する。

他の原因欄は0。各q・原因にcomplete/unknown/horizon/truncatedを別入力で与える。
unknownは終端無し・no_terminal、dwell無し。horizonは観測期間のlower dwellを持つ。
truncatedは分割量をtruncatedで撤回し、dwellを残さない。
2個のq×3原因×4停止=24履歴をengineとreferenceでそれぞれ独立に照合する。
履歴には正常な既存Task/WaitのIR在庫を使う。Q1の実イベントからこの履歴へ運ぶ配線は
上のUD/D/UU公開受入が担う。私有履歴だけでQ1配線完了と主張しない。

接続はcfg(test)のpath moduleだけ:
metrics.rsへp3a_wait_history_hand.rsをp3a_wait_handとして一度だけ読み、
engine/metrics.rsへp3a_wait_engine_tests.rs、reference/metrics.rsへp3a_wait_reference_tests.rsを読む。
両adapterは独立のHistory/Taskと実snapshot/projectを使い、原因の計算を共有しない。

## R4 p の有限点

G、総試行2、duration1、backoff0、同じknown end within2 >=99/100を固定する。
感度用入力では既知受信後の終端を同じdoneへ集約する。endのORをverifyに足す新構文は要求しない。

|p|D|UD|UU(未解決)|既知決着|期待Task数|
|---|---|---|---|---|---|
|0|1|0|0|1|1|
|1/10|9/10|9/100|1/100|99/100|11/10|
|1/5|4/5|4/25|1/25|24/25|6/5|

期待Task数は(1-p)*1+p*2=1+pの独立検算であり、既存網羅metricsに期待値APIを新設しない。
閾値99/100に対しp=1/10は等号を含むsatisfied、p=1/5はviolated。
receipt_resolutionは未解決経路がある点でinconclusive。rootはviolatedを優先する。
unknownを通常の失敗Endに置換しない。

EffectiveInputs の **response_channels** は元commandごとの配列。
各行の閉鍵はcommand、capability、loss。lossは上のタグ付き型で、同じチャネル仮定を記録する。
probabilityは0を含む厳密な有理数、basisと独立仮定は必須。pだけ・basisだけの変更はgate受理。
内部展開ID・正質量supportが変わっても同じ問いであり、元commandの制御は固定。
retry回数、条件、backoff、exhausted、閾値、deadline、Cutはcontract mismatch。
全点を実行前に検査し、拒否点の前に置いた点もevaluatedにしない。

所要時間1/2/3は三つの明示完全モデル点。within2の確率は1/1/0。
この有限点の結果だけであって連続区間・単調性・robust guaranteeは主張しない。
duration interval構文はU-SYN-0のまま。responsibilityと静的Authorityは消さず射影する。
durationだけの変更は同じ問い、責務者・grant先・basisの変更は問いの変更として拒否する。

## Phase 2持ち越し

m-1: common_prefix自体は配列で、項目内のnot_derivedオブジェクトをproducerは作らない。
項目pathの理由は欠落nullだけ。whole-tableのnot_derived理由はtable pathに置く。
calendar等の側の理由を .Manual.common_prefix:calendar として受理してはならない。
左右のderived cellとmissing-item/null、whole-table/multiple_case_interferenceは有効対。
m-2: fixture生成器のdiff_eはagency-diff-p2-1を生成する。旧版p1-1を再生成しない。
Python -BでAST上のdiff_e代入式だけを、空のdifferencesで評価して版を検める。
mainやfixture書込みは実行せず、コメントに新版文字列を足しただけでは合格させない。
U: ratesありmetricsなしのknown-failureと非重複2案件を両系でsimulateする。
拒否routeはclosed_at1/start無し/failed、実行中range停止routeはclosed_at無し/start1、
応答済み二案件はclosed_at3/7。ContestObservation全体のPartialEqも要求する。
usage: sensitivityとconformを各1回列挙する。verifyは引き続きunsupported/exit4。

## ECを使う小さな統合

keyed/plain試行の共通宣言を新しいsupport/p3a_ec.relcaに保存した。
Cut、責務、Authority、非決定結果は落とさない。1案件のみ。
在庫shortはaccept1+reserve1=2。Dの課金はt3、UD/UUの再送はt4。
GはUDのduplicateで既知決着へ進み、plainはUDでも最初の効果が未解決なので後続へ進まない。
known後のallocate1、confirm1、cancel1、ship1よりshippedはkeyed7..8/plain7。
allocate lostのrefund1よりstock_refundedは5..6/5。
CS10、Finance30を経たrejectは46..47/46、refund1なら47..48/47。
UUのobserved_until/elapsed/workは4。Gはcommit1、plainはcommit2。
charges==2の非決定min/maxはkeyed[0,0]、plain[0,1]。plainが常に二回課金する率ではない。
Finance rejectはresponse契約違反なので両子とcompare/diff rootはviolated/exit1。
plainのonce反例は二つの実attemptとcommitの証拠を保持する。
simulationは在庫等の非決定を一様化せずunsupported/exit4のまま。
EC全出力をoracleにせず、終端とこの反例の要点だけを照合する。

## 理由語と保証境界

|理由/分類|固定する意味|
|---|---|
|receipt_unknown|未解決案件結末。Endでも計算切断でもない|
|receipt_resolution|自動プロパティ在庫。未解決はinconclusive|
|response_control|F/U/集中/責務の比較未導出、警告の未検査理由|
|retry_feature_combination|contest/quantileとの未対応交差、U-SEM-0|
|sensitivity_contract_mismatch|問いの変更、E-CMP-0、invalid/exit3|
|null（degradation path末尾）|表または項目の欠落。既知の空集合ではない|
|multiple_case_interference|whole degradation tableの未導出理由|
|truncated|計算clock等の打切り。業務失敗やunknownに変換しない|
|no_terminal/no_samples|完了量の非適用。観測workとは別|
|per_attempt_conditional_on_normal_finish|明示した確率チャネルの独立仮定。実測の認定ではない|

設計書冒頭の実装なしという記述は現コードと一致せず、READMEが既に歴史的注記として区別している。
READMEのsensitivity/conformの説明とCLI usage一覧は一致していない。今回usageの受入で固定する。
旧Q1 crossing試験は一律unsupportedを要求するが、P3a checkpointがこの交差を意図的に変更した。
既存固定試験とこの追加受入は別の根拠として扱う。
