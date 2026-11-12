# ADR 0063 受入の手計算

基点は9d24fc44、裁定はADR0063。期待は実装の出力から採取していない。
Q9の新構文を除去したsubstrateを別に検査し、未実装REDと既存言語の誤りを分ける。
全公開結果はJSONの必要pathを照合し、Q7のuncertainty追加をbyte差として拒まない。

## C: 解析的な母模型

Judge.decideはjが0..1。結果failで時刻1にreturned。
Reviewer.replyは別人r、起票率p=1/4、時間D=2が3/4、D=4が1/4。
独立性は対象到達を条件にしたper_case_per_clause。発生と時間の根拠はいずれも検算用仮定。
対象から期限3なので絶対期限4。routeは開始1、応答3または5。

| 枝 | 質量 | 応答 | route作業 | route待ち | route費用 | 終端work |
|---|---|---|---|---|---|---|
| 不起票 | 3/4 | なし | Taskなし | Taskなし | never_created | 1 |
| 起票・短時間 | 3/16 | 3 | 2 | 0 | 10×2+2=22 | 1 |
| 起票・長時間 | 1/16 | 5 | 4 | 0 | 10×4+2=42 | 1 |

質量和1、流入当たり遅延1/16、起票条件付き遅延1/4、条件付き所要平均5/2。
期待route作業5/8、期待全体作業13/8、期待全体費用27/4、期待閉鎖時刻13/8。
これは理論値で、checkの既存metricsは全体work1..5・費用0..42、Reviewer work0..4の極値。
遅延の正質量があるのでcheckのcontest_responseはviolated。pを微小にしても変えない。
静的contestは常に構造を検査。p=0は追加裁定によりinvalidで、非起票の枝へ丸めない。

## 独立の有限乱数oracle

seed=1、16反復。鍵はADR0016のsha256-counter-v1、case=1、n=1から。
起票siteはcontest.Flow.clause1.occurrence。CDFはdeclined(3/4)、filed(1/4)。
時間siteはduration.Flow.contest:1、n=1で2分終了(3/4)かsurvive(1/4)、survive後n=2は4分終了(1)。
SHA256の先頭8byteをbig-endian整数とし、閾値との比較を整数で行う。
このoracleはproductionを呼ばず、Rust側でも標準sha2だけで再検算する。

表の0は不起票であって、0分のrouteを作るという意味ではない。

| 反復 | C p=1/4 | A1 p=1/2 | A11 clause ordinal2、contest添字1 |
|---|---:|---:|---:|
| 1 | 0 | 0 | 2 |
| 2 | 0 | 2 | 2 |
| 3 | 2 | 2 | 2 |
| 4 | 0 | 0 | 0 |
| 5 | 0 | 0 | 0 |
| 6 | 0 | 4 | 0 |
| 7 | 0 | 2 | 0 |
| 8 | 2 | 2 | 2 |
| 9 | 2 | 2 | 0 |
| 10 | 0 | 0 | 2 |
| 11 | 2 | 2 | 0 |
| 12 | 0 | 0 | 0 |
| 13 | 0 | 4 | 0 |
| 14 | 0 | 0 | 0 |
| 15 | 0 | 0 | 0 |
| 16 | 0 | 0 | 0 |

Cはfiled4/target16=1/4、通常応答4、時間平均2、全体作業平均(16+8)/16=3/2、費用88/16=11/2。
長時間が一つも観測されないのでsimulationの契約はinconclusiveであり、checkの違反を消さない。
A1はfiled8/16=1/2、短6・長2、時間平均20/8=5/2、全体作業36/16=9/4、費用216/16=27/2。
A1では遅延witness2があるのでsimulationもviolated。
ordinal対は先頭に非contestのat_most_onceを加えた模型。起票鍵はclause2、時間鍵はcontest:1のまま。
filed5、全て2分。clause番号とcontest添字の誤った同一視を検出する。

### 対象到達で条件付ける対

Judgeのkernelをfail/pass各1/2にする。鍵kernel.Flow.d。
returnedとなる反復は2,3,5,6,7,11の6本。
その中でCの起票鍵がfiledになるのは3,11の2本。起票率は2/6=1/3であって2/16でも4/16でもない。
通常応答は2本、service2/queue0/response_elapsed2。
非対象のposted10本で起票鍵やroute時間鍵を引かない。

## A1〜A15と対の観測

| 行 | 入力 | 変化と手値、主な試験 |
|---|---|---|
| A1 | c→half | pだけ1/4→1/2。上の支持・標本の差。core c_a1_check、a11_both、c_a1_nonuniform、a1_halfとCLI c_bernoulli/a1_changed |
| A2 | deadline→late | p1/D2、対象1/応答3。期限2なら絶対3で一致、期限1なら絶対2で遅延。時間を変えず契約だけ変える |
| A3 | tiny→tiny-no-authority、c→zero | p=1/1000000も正支持で遅延違反。Grant削除で構造violated。p0はE-PROB-0/exit3、理由contest_probability_zero_use_cut_or_excludes |
| A4 | missing/empty/duplicate | 発生率・発生根拠・時間根拠・独立仮定の各欠落、空根拠、未知独立、重複rates、p>1。分類はq9_inputs::STATICの閉じた11行 |
| A5 | legacy-fixed、legacy-unreached | rates無し。checkのwork1..3/cost0..22、runtimeの不起票/応答3の2結末、satisfied/exit0。simulateは対象到達でunsupported/exit4、未到達の旧contestは拒否せずinconclusive/exit2 |
| A6 | c→defer、fixed→defer-fixed | opaque deferだけ追加。構造件数と宣言pを保存し、率valueと全時間Sampleはnot_configured。実請求書のVendor.contest_rateとResubmissionも残す |
| A7 | deadline→clock | clockだけ20→2。時刻1に業務End・起票済みだが応答3は未観測。filed16/target16と率1は既知、unresolved16、時間はtruncated、root inconclusive |
| A8 | fixed→deadlock | Reviewerの匿名capacity0、通常応答なし。route_failed16、時間no_samples、deadlock違反。差はcapacity制御で、実行者の無いActorにpermitを残す別エラーは除去 |
| A9 | two-spaced→two-burst | 起票p1/D2。到着0/4で対象1/5、応答3/7、待ち0/0。到着0/1で対象1/2、応答3/5、待ち0/1。serviceは双方2 |
| A10 | calendar→calendar-later | 前者Reviewerは[0,2)稼働/[2,6)停止/[6,21)稼働。continueで一本目は1..3、二本目は6..8、待ち1/応答所要3。開場7へ変えると二本目7..9、待ち2/所要4、期限3超過を16反復で観測 |
| A11 | ordinal、conditional-target | 上の非対称pと独立hashbank。合成site/ordinal、反復別drawsと作業を両interpreterへ照合 |
| A12 | same-tick、two-clauses | 同時刻のfinish又は同じ人のroute開始に残る非決定を標本化しない。前者j/j2の2案件同時End、後者同じ案件の二route。初回拒否のprefix件数はnull/aborted |
| A13 | fixedの実wire→単一故障 | 分母・件数・負数・route/cohort・sample数・時間・null漏れ・未知欄・重複IDの11変異。独立照合とBusinessMetricsのdecodeを分ける |
| A14 | no-metrics、compare/diff、Q7接続 | metrics無しは新表無し。根拠4行はanalyst-declared-not-validated、uncertaintyに限定を保持、ledgerは局所版を参照。子Reportの輸送のみで新目的や差分を作らない |
| A15 | mixed / mixed-unreached / route-nondet / normal-call | 同じ対象の旧率無しclauseは遭遇で全体拒否。未到達旧clauseは率付きの標本を阻害しない。routeの多値結果も未解決のまま拒否。同じReviewer.replyを通常callで先に0..2に実行する対は、対象3/異議応答5、全体work5/cost44、終端cost22。先の通常終了を異議の応答に流用しない |

二案件の全体作業は各反復6、16反復。案件32本を独立trial32へ変えない。
二案件の実行順を暗黙に決めないため既存の明示policy fifo tie_by case_id then call_order assign first_idleを入力する。
一方A12はその方策でも残る同時finish又は方策未指定のroute開始を意図的に残し、unsupportedを要求する。

## 公開wireと判定の境界

- metrics.contestsはrules/basis/unit/scope/rows、各行は既存static IDとcase_id。
- filing_fractionのvalueは既約文字列。分子filed、分母target_observed、未知/保留時は理由つきnull。
- service_time/queue_wait/response_elapsedは同じ通常終了cohort。Sampleのmean/min/max/samples/reasonは全欄一致。
- 対象/起票/応答の三つの件数恒等式を別々に検査する。構造件数を既知0と未知nullで区別する。
- p1でもruntime契約の標本は無違反ならinconclusive。静的contestはstructuralのsatisfiedのまま。
- 請求書Aの誤登録231/125000は閾値1/1000を超えるためroot violated/exit1。品質・費用の保留を合格へ使わない。
- whole-output byte互換・旧manifest再採取は行わない。期待を新実装の出力で更新しない。

## 実行していない私有境界

手組み16履歴の集約器直接投入、公開IRの率欄の改竄、効果率/Q1/条件付き時間の全交差、
モデル対照でない全てのpriority tie列挙、production故障注入は、この公開受入だけでは実行しない。
内部試験の義務はcontest_rates_obligations.mdで分ける。未実装での失敗を意味論の確認済みとしない。
