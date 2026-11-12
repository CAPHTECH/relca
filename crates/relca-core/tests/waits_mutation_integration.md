# C6 と A7 の交差 — 出力を見る前の手計算

段階 C。ADR 0046 の再 lower・選択・系譜の保存・全種の未完了分類と
ADR 0048 の両 collector・queue・仮定・表の除外を結ぶ。
数値は分。fixture 群を増やさず inline の閉じた入力を使う。
元の fixture の旧い値・凍結した母集団は変更しない。

## H: 義務4の HQ、horizon 12

wait-horizon-queue の horizon 3 を12へ変え、Done と DoneRate を追加。
到着は0,0。A容量1、所要5、FIFO: 案件1 [0,5)、案件2 [5,10)。
lead=5,10、service=5,5、queue.actor_capacity=0,5、predecessor=0,0。
A queue integral=5、max=1、length average=5/10=1/2（終端で観測終了）。
cohort=2/2=1、metrics complete、切断なし、dwell/lowerなし。
reads、authority_basis、if、複数消費者が無いので6生成種は全て
not_applicable、残り4種は unsupported、run.complete、root unsupported/exit4。
元と全ての変異体は Day の selector と宣言由来のmetadataを保存する。
aggregation.observed_untilは実行由来なので、Sの複製では5→7（horizonなら6、
clock切断ならnull/truncated）となる。元との同値でなくmask対の同じ変異体と照合する。
selector を省く負例は E-CONF-0/invalid/exit3、metricsもwaiting行も無し。

## S: 複製で queue が変わる場合

mut2-summary の pass/fail を pass に限定、到着0,0、FIFO、horizon12、
clock20、Done と DoneRate、breakdown waitsを追加。各作業は1分。
元の E は [0,1),[1,2)、Clerk [1,2),[2,3)、Approver [2,3),[3,4)、
Ledger [3,4),[4,5)。lead=4,5、service=4,4、queue=0,1。
複製後の E の順は case1.e [0,1),case2.e [1,2),case1.e_dup [2,3),
case2.e_dup [3,4)（ready時刻優先）。Clerk [3,4),[4,5)、
Approver [4,5),[5,6)、Ledger [5,6),[6,7)。lead=6,7、service=5,5、
queue=1,2。Extractのqueue integralは1→3、averageは1/5→3/7、max1。
系譜は消費者aのsubject=[Invoice]、independent=[]で保存。
C5の判定はguardが読むp,aの2件で、eとe_dupはguardに読まれない。
group2→2、全3行held（ADR 0046の観測9と同じS直結の形）。
初稿では別のobserved-writer入力のR2の3→4を誤適用したが、変更前
bcd69f38のSのcheckの2件とADR 0046を確認して訂正した。
summary_reused はdetectedだが理由はこの系譜保存だけ。
waitingとqualityの変化は検出へ昇格しない。metricsの差はother_changesに
direction=changedとして輸送される。割当てはmetrics/qualityを読まない。

horizon6では元は両案件完了、複製は案件1が時刻6にdone（包含境界）、
案件2はLedgerの[6,7)を完了できずcut_off。metricsはcomplete、dwell/lowerを開示。
summary行はinconclusive/assigned_check_unavailable、runもinconclusive/
mutant_incomplete。clock6・horizon12の対では計算上のtruncatedであり、
summary行はinconclusive/truncated、新しい待ちScalarはnull/truncated、
dwell/lowerは撤回する。静的heldを検出として流用しない。

## W: 待ちだけの改善を検出にしない

C6 guard_horizon の else にwait2を追加、horizon8、Done、metricsを追加。
元はExtract1+Clerk1+wait2=4でrejected、service2・intentional2。
keep_thenの変異体はExtract1+Clerk1+Ledger2=4でdone、service4・intentional0。
hold_delayed と refusal_path_cut は割当て契約が無いので両方
undetected/no_assigned_check。DoneRate0→1もwaiting2→0も検出の理由でない。
唯一のWaitを消す変異なので、Case.intentional_waitの条件付き欄は欠落し、
doneのterminal.breakdown.intentionalは数値0を保つ（ADR 0048の語彙上の有無）。
waitを持つ両経路は engine と reference の独立計算で同じ手の値を要求する。

## 六種の運搬、拒否、閉じた照合

SにExtractのexecutorとauthority_basisを追加し、arrivalsもbounds.casesも1に揃えた単一案件対は
evidence_missing2、authority_expired1、hold_delayed1、summary_reused1、
refusal_path_cut1、original_access_removed2の計8体。全体の表は10行、
未対応4種は消さない。各reportのmetrics/quality/Day/仮定と、
新句を同じbyte長でmaskした対のresult/reason/assigned_changesは一致。
新句で状態数・終了・乱数を変えない。

simulateの句を欠くSはmetrics無しでwaiting無し、品質の4行だけを残す。
configurationを足した対のcompare/diffは旧metrics表を保ち、
waiting_tablesを開示する。これでproduction validate_waitsの受理経路も通す。
実CLIの全mutation wireで元と全reportを再帰的に検査し、
queue量の未知欄・仮定欠落・偽detectedの故障を拒む。

### 全種の分類を待ち付きで分離する私有 seam

公開の Evidence/原本 reads 削除は所要時間を延ばさないため、全種に
実際の horizon 超過が起きたとは主張しない。既存の mutation_default_private
接続から、上の8体を生成する単一案件 S の実 Report を基準にし、返す Report
だけを制御する。全6種それぞれについて、6 verdict × truncated の3値 ×
Agency有無の2値 = 36組。優先順は invalid→static_invalid、unsupportedかつ
Agency無し→static_unsupported、truncated無し→runtime_unsupported、
truncated真→truncated、残るunsupported→runtime_unsupported、
inconclusive→assigned_check_unavailable。これらは assigned/other とも空。
成功側の対照は static held の3行だけを summary の検出とする。
同じ8体を実際の列挙から run に通す seam は、inconclusive/unsupported の
各種で mutant_incomplete を要求する。合成 Report の公開到達性は検証しない。

静的な実入力の対照は C6 の RuleChange（通常grantとchange grantを共有）へ
待ちと品質を追加する。期限0の変異は restricted rule-change Authority の
既存の拒否なので invalid_mutant/static_unsupported、metrics無し、従って
metrics.waiting無し。元とsummary変異はmetrics有り。これをcore/実CLI両方で固定する。

Rule併記の仮定の順は既存のADR 0044 tail規約に従い、metricsの5行の後に
ApprovalRule.rule、authority.RuleChange.change.ApprovalRule.cfo、その後に
品質4行、quality.rule、metrics.waiting（網羅の場合）。新待ち句を取り除いても
Ruleの2注記は消さない。古いmetrics照合器はmetrics5行が全体末尾だと仮定し、
この合法なRule tailを拒んだ。受入側だけを最小追従する（orch許可受領）。
元/summary両Reportで .rule 注記を1行欠く、隣のgrantと逆転する、直前の
metrics.observationと逆転する、quality.ruleとquality.schedulerを逆転する
単一の負対照は各々Errを要求する。違う並びを一律に除外して通す射影はしない。

## 受入の訂正履歴

前役の初回ログ a7onto-cross-core-first は6成功/5失敗、secondは9成功/2失敗。
以下は production の欠陥ではなく受入/入力の訂正であり、故障校正のREDに数えない。

* all_six は初回で元がinvalidだった。現入力は到着列と cases をともに1へ
  揃えている。[R] 初稿の到着数2とcases1の不整合の訂正と読むが、
  初稿ソースは保存されておらず正確な編集前テキストは未確認。
* observed_until は宣言metadataではなく実行で決まるため、元5と複製7/6/nullの
  同値要求を撤回。宣言5欄は保存し、動的欄は同じ変異体のmask対と照合する。
* 最後の lexical Wait を消す変異では Case.intentional_wait は不在となる。
  値0を要求するのは terminal.breakdown.intentional。時間の算術2→0は変えない。
* S直結の判定数を観測writer入力の3→4から2→2へ訂正した（本文Sと
  固定した計算規則による）。この数は出力の一般化ではなく、
  guardに読まれるp/aの2判定という入力の構造に対応する。

[U] 初稿が最初の実行より前に書かれた時刻順は未追跡ファイルからは証明できない。
本役は上のイベント列・区間和・系譜を入力と照らして再導出した。追加した
私有matrixとRule拒否の期待は、その追加テストの初回実行前にこの節へ記した。

ここでの独立期待は、実行結果や故障校正の完了を表すものではない。
