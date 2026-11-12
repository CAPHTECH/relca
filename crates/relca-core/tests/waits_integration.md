# ADR 0048 × ADR 0043・0045・0044 の取り込み受入

期待は新collectorの実行前に手計算として固定した。数値と根拠を以下に記す。
時間の単位は分。公開モデルを各 interpreter に独立に与える。

* T03（external）、T09（contest）、T17（compare）の Agency・義務・警告は
  統合済み変更前 b6951fb binary の同幅の空白対照から固定する。
  これは既存出力の互換oracleであり、新しい待ちの数値を採取するものではない。
* HQ: 同時到着2件、capacity1、service5。H3では観測完了0、未知2、網羅期待件数0..2、DoneRateは
  undetermined、保守端0..1。metricsは観測量なのでcomplete。
  案件1のdwellはservice3、案件2はqueue.actor_capacity3。
  queue total3、max1、average3/3=1、total/averageだけlower。
  clock3ならqualityはtruncated、新しい網羅Scalarは全null/truncated、dwell/lower無し。
* simulation H12は4回すべて完了5/10、DoneRate1、H3は4回すべて未知2。
  平均queueは5と3、観測時間平均は1/2と1。未完了を失敗に数えない。
* Rule true: prep[0,3)、wait[3,5)、finish[5,10)。lead10、service8、intentional2。
  Afterのfalse: prep[0,3)、finish[3,8)。lead8、service8、intentional0。
  ChangeのcfoはRule改定権者であってAのdispatch候補ではない。
* F36: definition、denominator、form、scheduler（simulationならestimation）の
  4行を既存順のままwaitingより前に置く。waitingはquantile/probability.schedulerより前。
  正常と早期拒否を分け、report.rsのwaitingを先頭へ置く故障で意味のREDを要求する。
  取り込み時の裁定: E-SIM-0でmetrics自体が出ないsimulation句欠落（scenario有/無）の
  2経路ではwaitingも出さず、B6の拒否開示である品質4行だけを残す。
  F36の順の義務は両方が出る通常経路と実行中nondet拒否に掛ける。

固定母集団は変更しない。waits_names.json/rsの43名だけを明示に除外する。
C6は未統合につき保留。B7 D05、C7 validate_waitsの交差はそれぞれの取り込み義務。

追加の既存経路の手計算:
rec-approvalはextract1+approve2でreturned3、lowのreview3後ならreturned6。
postedはさらにpost1なので4又は7。並列も明示waitも無くlead全体がservice。
agy2-jueはenter1+approve1でreturned2、post1を足してposted3。
cf-version-atomicは2件とも時刻0で開始、finish1で片側commit・片側conflict。
各案件の可能なdone/conflict_endはともにlead=service1、queue0。
lim-overflowはqueue capacity1のreceipt層で1件だけ受け入れ2件overflowed、
受入案件run[0,2)のlead=service2。overflowはTask待ちを作らないのでqueue0。

## 取り込み後レビューの手oracle（940166ea、実行前に固定）

根の制約は r7 裁定の a7 取り込み後レビュー。待ちを計算していない経路で
metrics.waiting を出さない。旧い metrics.window 等まで消す契約ではない。
品質を正常に lower できたsimulationの拒否では definition → denominator → form →
estimation の4行を連続して残す。Rule ならその直後に quality.rule。
parse/lower 失敗は品質の定義を確定できないので品質行も待ち行も無い。
入力の対照は breakdown waits の15byteだけ同幅空白にする。

* R1: auth-revocation の G は q へ A.run を授権し p による撤回を記録する。
  window20/JPY と Done/DoneRate を足すが simulation/scenario は置かない。
  明示の simulation 句が無いので invalid/E-SIM-0/exit3、attempted0、metrics無し。
  authority.G.revocable_by=person:p、unchecked の dynamic_authority_revocation と
  旧い metrics.window・metrics.currency・metrics.rates・metrics.lead_time・
  metrics.observation は保存する。待ちの数値は作らず waiting 行のみ禁止。
  revocable_by を外す兄弟、品質無し、Rule併記、各 waits無し対照も別に固定する。
  waits無しの raw stdout/stderr/exit は変更前940166eaのraw文字列で凍結し、
  新しい期待値を出力から採取する用途には使わない。
* R2: HQ sample に verify terminating end（別対として recoverable end と
  almost_sure end）を足す。simulation は構造的完了を問えないので
  unsupported/U-SEM-0/exit4、attempted0/completed0、metrics無し。
  品質4行は上記の順。waiting 行は無い。completion無しは既存F36正常sampleの対照。
* m3: Rule/quality/waits の3宣言は通常の Before/After の実行を残すが品質は
  aborted へ撤回する。網羅は definition/denominator/form/scheduler/rule、
  simulation は definition/denominator/form/estimation/rule を連続ブロックにし、
  その後に waiting。Ruleの真偽から品質値を捏造しない。
* sweep: parse・lower失敗と simulation_input_failure は metrics/quality/待ち行無し。
  通常/実行中nondet拒否の metrics宣言有りは metrics有りで品質4行→waiting。
  metrics宣言を丸ごと外す通常/実行中拒否は品質4行だけでwaiting無し。
  非公開の防御return（lowered error、Actor metadata不正、verify空、集計積overflow）
  と driver が metrics/template を返さない末尾経路は tests/support の私有受入。
  有効な品質定義を保持したIRを入力し、品質4行（網羅の4行目はscheduler、
  simulationはestimation、Ruleならruleを直後）と
  metrics/待ち行無しを要求する。接続前は実行済みとしない。
  7経路×品質有無×Rule有無×waits有無×revoker有無の112probe。
  cfg(test)への接続と実行が必要で、substrateのlower成功だけではguardの被覆としない。

### n1: diff unchecked の閉配列（出力の採取でなく静的な検査義務から列挙）

通常の同じCutで全静的検査済みなら旧9語と未検査CHECKED語は無い。
metrics表は旧量を報告するが差分・順位・意味の揃いは検査しないので、
`[metric_differences, metric_ranking, metric_semantic_alignment]`。
waits有りなら直後に waiting_tables を1語足す。

Cut不一致では comparison_mismatch だけを検査済みとし、旧語を
`[time_intent, resource, meaning, authority, visibility, contest, preserve,
override, maintain, actor, relation, intent_claims, retire_loses]` の順に残す。
metrics比較自体が始まらないため metric_* 語は足さない。
非legacyの未射影案件表は case_outcomes/case_properties、Agency差分は
agency_differences で未検査とする。waits有りの配列は上記13語の後に
`[waiting_tables, case_outcomes, case_properties, agency_differences]`、
無しは `[case_outcomes, case_properties, agency_differences]`。

contest + response の legacy入力では義務表と異議実行表も比較外。
同Cutの配列は metric_* の3語の後に
`[obligation_tables, contest_runtime_tables, waiting_tables]`（waits無しは末語無し）。
Cut不一致の配列は上記13語の後に
`[responsibility, obligation_tables, contest_runtime_tables, waiting_tables, agency_differences]`
（waits無しは waiting_tables のみ無し）。これで waiting を diff の末尾へ
誤って移す故障も落ちる。compare の末尾規則を diff へ流用しない。
responsibility は contest に伴い責務の構造が存在するのに、Cut不一致で
authority の静的検査まで進めていないため未検査に残す。

さらに同Cutのlegacy contest+response入力で、responseの期限だけをu64::MAXへ
変える対を置く。Approver.finishは時刻2なので2+MAXがoverflowし、実行は
unsupported/U-SEM-0/exit4で中断する。metricsはabortedで存在するが比較表は空、
terminalsの射影も未検査。waits無しの閉配列は
`[metric_tables, obligation_tables, contest_runtime_tables, terminals]`、
waits有りは `[metric_tables, obligation_tables, contest_runtime_tables, waiting_tables, terminals]`。
これでWとterminalsの相対位置も独立に固定する。

初回観測の受入側の訂正（意味実装のREDとは区別）: Authorityのwire値の接頭辞
person:、品質撤回のセルの場所 value.reason、contestの上記responsibility語、
diffのstderrのfrom/to/diff接頭辞を実装本体と既存契約で確認して訂正した。
これらは新しい待ちの期待を出力から写す変更ではない。

以下の手計算の期待値は、実行観測そのものではない。

### 段階Cの観測（940166ea、意味実装は未変更）

受入側の旧wireの前提の訂正後、coreの50定義は1回実行して42 GREEN/8 RED。
追加の実CLI31定義は1回実行して23 GREEN/8 RED（旧75定義はこの回ではfilter外）。
最後に追加したterminalsの相対順の対は、core/実CLIを各1定義ずつ実行して両方GREEN。
両モードのRule順と手列挙10配列、raw10jobの旧byteは一致した。
各REDはmetrics無しでのW残存（7入力）か品質4行より前のW（1入力）であり、
構文エラーや不適切なsubstrateではない。各REDにwaits無しのGREEN対照を置いた。
私有112probeは有効なsubstrateのlowerだけを確認し、guardの接続/実行はしていない。
stageCの受入をproductionの修正済みや全経路の意味検証済みへ読み替えない。
