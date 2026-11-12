# ADR 0049 取り込みの手計算（実行前固定）

基点 bcd69f38。交差は inline 入力であり、閉じた qtl 67 入力を増やさない。
分位点は旧 verdict を変えない。同じ長さの宣言マスクとの全旧 report 比較を行う。

| 入力 | 手の経路・質量・期待 |
|---|---|
| T14 pooling と horizon4 | 4反復×2案件、完了時間3/6。H6なら中央値3・75%6。H4ならHit3×4,Censor earliest5×4。両中央値3、75%はnull/undetermined、all LB5 / given LB3。根S0 / I2 |
| T18 denominator H2→H1 | 到着0、終了2。H2は両中央値2。H1はearliest2、Dlo0なのでgivenはnull/undetermined/LBnull、allはLB2。根S0 / I2 |
| clock1 < H30 | 同じT18でclock1が先。earliest2は同じだが理由は両形式truncated。all LB2、given Dlo0なのでLBnull、根I2。Hを宣言しただけでundeterminedへ変えない |
| T25 censor-within | 到着0/5、Hit3とearliest2/latest3。期限3があればgiven p1は3に識別、無ければnull/undetermined LB3。allの手の期待は既存deferredをそのまま使用 |
| X-unident-g | 到着0/5、H6、Hit3とearliest2。中央値の楽観2・悲観3は同じ値でないため、両形式null/undetermined LB2。有限[2,3] exactでない。根I2 |
| record | 既存RECORDのpassの質量23/25、初到達1。all p1/2=1、given p1=1。Wrong質量1/50、初到達2。all p1/2はbeyond_reached_mass、given p1=2。recordのguardをlatent correctへ置換した対はspan付きE-POL-LATENT、実行しない |
| Held/H3 | 到着0/1/2、同時受理上限1、service5。実停止はH3なのでearliest4/3/2。all中央値LB3、givenはDlo0なのでLBnull。いずれもundetermined。H宣言があるだけでclock/live_tasksの切断をundeterminedへ変えない |
| Held/initial true | 同じ入力の初期値false→trueだけを変更。ADR0039の受領と受入の区別により、Heldも到着時点で初期化・受領済み。全3件Hit0、all中央値/given p1とも0。未到着だけが観測対象外 |
| version_check/atomic | 同時到着2件、keys空、cap2で両方version0を読む。一方がt1でcommitすると他方conflict、各実経路のhit件数1。しかし案件ごとのmin0/max1なので和のDlo0/Dhi2（同時の分布ではない）。all中央値とp1はbeyond_reached_mass/LB1、given p1はdenominator_may_be_zero/LBnull。version_checkを一行除く対では両方commit、3値とも1。simulationではshared finish順の非決定を拒否し3行ともaborted、既存の根とdiagnosticsを保つ |
| overflow | 同じ上限でon_overflow end overflow。case1はHit5、case2/3は既知miss。Doneのall p1/2はbeyond_reached_mass、given p1は5。Overflowのfirst-hitは0（受付時）、all p1/2とgiven p1は0。非決定を一様化しない |
| Held/Overflowのsample | 上3入力は所要・結果・FIFOが固定なので1反復×3案件で同じ手の値。Held(false)はN3/C3、Held(true)とOverflowはN3/C0。平均や反復別分位点にしない |
| atomic/Standing | 既存sov-atomicからApproverとClerkの結果をpass単値に固定した新入力。registeredはt3で初到達し両形式3。post-business routeのworkは初到達の時刻を遅らせない。対はallow=true→falseの一編集で両値null、allはbeyond_reached_mass、givenはzero_denominator。根は旧safetyのS0 |
| Rule × 2configuration × 2scenario × 両mode | 裁定により品質全体aborted、数値/LBはnull。Rule初期true→falseを通常0/不達の分位点へ変換しない。quality.ruleの開示を保つ。通常計算はADR0044 M2と同じ将来義務 |
| C6六種の輸送 | 元/再解析mutantのquantilesを同じ選択scenarioで運ぶ。独立手編集したsourceの手の値と比較し、assigned/other変更集合にはqualityを加えない。種7はB7待ち |
| C6×H4 | summary原入力をpass単値へ固定、1案件の終了4。元はS0、両分位点4。summary一複製で終了5となりH4では未完了、子はI2/truncated=false、all中央値undetermined/LB5、given p1はDlo0でundetermined/LBnull。C6分類はinconclusive/assigned_check_unavailableでありdetected/heldにしない |
| simulation拒否 | 句欠落2経路とstructural completionの拒否ではquantiles全行aborted/N0/C0/LBnull、quality.uncheckedに置く。既存のquality.estimation等の行はquantileの前、metricsが無い経路へ架空のmetricsを出さない。quantileを外した同長対照と旧全欄を一致させる |
| T24 private | case1@0,case2@10、true@11→false@12はcase1帰属。case2 self trace空でも共有状態を観測し、初到達11/1、q1=11。既存world adapter/両observer試験を再実行。公開DSLの再現とは呼ばない |

受入を実装出力に合わせて広げない。旧判定が疑わしければPRE-C7 binaryとADRを
先に照らし、裁定を求める。A7/B7の交差・目的・12語のwireは後続ontoまで保留する。

作成途中の訂正: Overflowをscenario内へ置いた初稿はADR0042 U10の静的拒否を
踏んだため、model共通のconcurrent宣言へ移した。Held初期値の初稿は受領と受入を
取り違えたので、実行の出力でなくADR0039の受領/初期化規則と両driver本体から
先に訂正した。CLIのwarningを空stderrとする初稿とOutcomeのSerialize仮定も受入の
実装ミスであり、productのREDへ数えない。
version_checkの初稿は同時のhit件数1を案件ごとのmin/max和へ誤って昇格した。
ADR0049のper_case_relaxationの定義へ戻して上のDlo0/Dhi2から訂正し、数値を
実装の出力から写す修正とは区別する。

## 7710e6b4 取り込み後レビューの手計算（実行前固定）

- R1/m1: `rule()` の Light/Later の scenario 宣言だけを除く。案件数1、
  scenario/arrivals/start_policy/admission_queue/concurrent_cases は全て無しなので、
  lower 後の `legacy_route()` は真。AsIs は Required の初期値 true、ToBeB は
  false だが、裁定78–79によりどちらも Rule の品質全体を撤回する。
  All(p=1/2, all_arrivals) と Given(p=1, reached_only) の順で、両値とも
  lower/upper/basis=null、reason=aborted、lower_bound=null。
  quantiles は unchecked に一度だけあり checked には無い。公開 report/実 CLI は
  quality.rule を開示する。同長マスクと旧 Outcome/report の全欄を比較する。
  この2選択を engine、独立参照、実 CLI check に別々に手値へ照合する。
- R2: 実 CLI の各 mode の選択順は手で
  [(AsIs,Light),(AsIs,Later),(ToBeB,Light),(ToBeB,Later),
  (ToBeB,Later),(ToBeB,Light),(AsIs,Later),(AsIs,Light)] と固定する。
  前半の四つが全積、後半がその逆順であることも assert する。
  check/simulate とも同じ撤回、sample は N=0/C=0、点null、reason=aborted。
- nit: rule_exhaustive_reference は参照の Outcome と同長マスクだけを読む。
  engine/public report は rule_exhaustive_engine が読む。参照の Rule abort の
  add_tree_quantiles 引数だけを通常 stop 判定へ戻す単独故障では、参照試験だけが
  RED、engine 試験と実 CLI は GREEN であることを校正する。
- legacy 単独故障では engine.rs の quality_legacy_with_bits の Rule return だけを
  外す。arrivals の guard は残す。legacy engine と実 CLI が RED、legacy 参照と
  scenario 経路の engine は GREEN と予測する。通常値0/不達は正解にしない。

これは受入と校正の有限の範囲であり、全選択状態や全 guard の保証ではない。

### 取り込み後受入の観測（production基点7710e6b4）

| 状態／単独故障 | 実行した受入と観測 | 作業木ログ |
|---|---|---|
| 基準GREEN | core52本、CLI75本（既存matcher65本を含む）全て成功、exit0。新legacyはengine/参照/実CLIで両configurationを実行 | 基準実行ログ（別途保存） |
| legacy Rule returnだけを外す | core rule_legacy_engine と実CLI legacyだけRED（exact0 vs aborted）。coreの他5本とCLIの他2本はGREEN、全体exit101 | legacy故障のログ（別途保存） |
| 参照add_tree_quantilesのabort引数だけをstop判定へ戻す | core rule_exhaustive_reference とrule_legacy_referenceだけRED（REFERENCE vs hand）。engine2本・sample2本・実CLI3本はGREEN、全体exit101 | 参照故障のログ（別途保存） |
| 復元後GREEN | Ruleを選んだcore6本とCLI3本が全て成功、exit0 | 復元実行ログ（別途保存） |

両故障は1件ずつ注入し、直後に逆patchで戻した。毎回
`git diff --exit-code -- crates/relca-core/src crates/relca-cli/src` はexit0。
故障patchはcommitに含めない。未変更productionの欠陥はこの受入では再現しなかった。
最初のCLI legacy試験はinvocationのcommandキーを受入側が書き忘れて落ちたため、
その著者誤りを修正した後のGREENと上の校正を記録した。productのREDには数えない。
