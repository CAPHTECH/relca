# ADR 0053 取り込み前レビュー: 根の制約 A / B の手計算

裁定は統合前のレビューと2026-11-01の追加回答に基づく。受入の要件を以下に記す。
productionを変える前に期待を固定する。`deferred_review` のcore/実CLIで同じ手値を使うが、
engineと参照の結果は各々この手値へ照合する。相互一致だけを正解としない。
既存19構成/18ソースに、9模型のdefer有無18ソースを加える。計37構成/36ソース。
新fixture directoryは0、既存46群は不変。

## A: 先行理由は保つが、数値は保たない

`A.go` は2分。通常はpassのみ、Doneに到達、Neverには到達しない。
Left/Rightは同じ本文の**別process**で、simulationの操作鍵を共有しない。
全モデルに品質・分位点・hard 2行(Rate >= 0、Rate >= 1/10)・目的2行を宣言する。
非決定を含む2模型のsimulationは従来どおり拒否で、網羅の手値と混同しない。

| 模型 | defer無しの手値 | defer A の期待 |
|---|---|---|
| truncated | clock1 < finish2。Doneの案件別確率0..1、期待件数0..1。Rate.valueはtruncated、fallback0..1。3反復ではhits0/unknown3/trials3、比の子Sampleは分子0・分母1、各samples3 | 理由truncatedを保ち、以下の全数量をnull。hits/unknown/trialsは保持。子Sampleはmean/min/max null・samples0・not_configured |
| undetermined | horizon1 < finish2、clock5。上と同じ数値で理由だけundetermined。業務cut_offと計算truncatedを区別 | 理由undeterminedを保って全数量null。標本の子Sampleも上と同じ撤回 |
| zero_denominator | Done1 / Never0。比はzero_denominator。3反復の子Sampleは分子1、分母0、各mean=min=max、samples3 | 親のzero_denominatorはそのまま、子は両方null/samples0/not_configured。期待件数からの別の数値コピーを残さない |
| denominator_may_be_zero | nondet pass/fail。Done0..1、Never0..1、分母最小0/最大1。比はdenominator_may_be_zero。Doneの全流入分位点はbeyond_reached_mass・下界2 | 比の理由を保存、全ての期待/端点をnull。分位点の先行理由も保存し、下界2はnull |
| beyond_reached_mass | 同じnondetで、比はDone/arrivals=0..1、Neverの95%分位点はbeyond_reached_mass・下界2。Fmin(∞)=0 < 19/20 <= Fmax(∞)=1 | 比はnot_configured、分位点はbeyond_reached_massを維持、下界2はnull |
| 既存qual-clock.broken-short-clock + hard | 第1/2案件のWrong各1/50、第3案件0..1。期待件数1/25..26/25、Rateのfallback1/75..26/75。Rate>=0はsatisfied | defer Checkerによりfallbackも期待件数/案件別端点もnull、hardはinconclusive・interval/basis null。Coreの切断はそのまま |

最後から2番目の下界2はADR0049の「あるスケジュールでは到達」の場合。
確率1/2で到達し、どのスケジュールでも質量不足の場合は同ADRの別形で下界なし。
この二つを取り違えた初回の受入案は修正した。期待の契約を新出力へ緩めたものではない。

### 数値を残してはいけない位置の閉じた表

TはDとBの交差(opaqueもT)。既存のnull理由を上書きしない。
`bounds_with_undetermined` は既存schemaのnullable objectなので、**object全体をnull**にする。
新しい `{lower:null,upper:null}` wireは作らない。

| ID | 出力位置 | T時の期待 | 保つもの |
|---|---|---|---|
| A01 | quality.properties[].cases[].min / max(網羅) | 両端null | case_id/arrival、先行reason。数値だったcaseのreasonはnot_configured |
| A02 | properties[].expected_count.lower / upper / basis | 全てnull | 先行reason、宣言名/条件/within |
| A03 | properties[].expected_count.mean / min / max(標本) | 全てnull、samples0 | 先行reason(無ければnot_configured) |
| A04 | cohort[].value.lower / upper / basis、標本のpoint | 全てnull | 親のtruncated/undetermined/zero_denominator/denominator_may_be_zero等 |
| A05 | cohort[].bounds_with_undetermined | null | 行そのもの、宣言のdenominator |
| A06 | cohort[].value.numerator / denominator内のSample.mean / min / max | 全てnull、各samples0。親が先行理由つきでも再帰する | 子の先行理由、親のsamplesは構造観測なので維持 |
| A07 | quantiles[].value.lower / upper / basis / point | 全てnull | p/property/form、先行reason、標本observations/censored |
| A08 | quantiles[].lower_bound | null、先行理由による免除なし | 分位点行の在庫 |
| A09 | outcomes_per_arrival[].expected_countおよびrateのlower/upper/basis/point/mean/min/max | 全てnull、Sampleのsamples0 | kind/end、先行reason |
| A10 | evaluation.hard[].values[].interval.lower / upper、basis / point | interval自体null、basis/point null。保持hitsからCPを再構成しない | constraint/cohort/op/bound/行順。先行品質reason、source_reasonは重複を作らない |
| A11 | evaluation.objectives[].values[].lower / upper / basis / point、sample内mean/min/max | 全てnull、sample.samples0 | objective/direction/行順、先行reason |
| A12 | compare.tables.quality[].values[].valueのexpected_count / rate / value | 元Reportと同じnull射影 | key/定義/順序、reported/not_applicable |
| A13 | compare.tables.qualityのfield=bounds_with_undeterminedのvalues[].value | null | key/行/案の順序 |
| A14 | compare.tables.metricsの各数量コピー | Bの射影と同じ | key/定義/行/案の順序 |

quantityでない構造観測: properties[].unknown_count、標本casesのhits/unknown/trials、
cohort宣言のarrivals件数、quantileのobservations/censored、metricsのcounts/starts、
入力単価/capacity/window、旧Core terminalsとverifyは保持する。
目的欠損を下界で埋めず、支配を捏造しない。hardは保留状態で判定を止めるので、
理由の文字列がnot_configuredでないことは再推定の許可にならない。

## B: routeと共有する資源・人

| 模型 | defer無しの算術 | defer時の期待 |
|---|---|---|
| contest | 通常A.go=1分×1円、任意の異議Review.reply=2分×10円。start費0。終端lead/costは1/1で閉じる。観測workは1..3、観測費用1..21 | defer Reviewは通常callには無いが起動可能routeにはあるのでT。再レビューの裁定によりA自身work1/占有費1/start費0も保留。全体・Case合計と全時間量はnot_configured。Core終端は不変 |
| shared_resource | A.x1分/1円、R.y2分/20円。forkでDesk容量1を共有、SoloはAだけ。どちらから始めてもlead3/work3/cost21。A待ち0..2、queue length最大0..1、時間平均0..2/3。Desk占有3、Solo占有1 | defer RでT。A自身work1/費用1/開始費0も保留。**全Actor/Task/人/資源の時間費用をnot_configured** |
| shared_person | 上と同じfork、両Actorがpを共有。Deskは使わず、SoloはAだけ。lead/work/costとA queueの区間は上と同じ。Desk占有0、Solo占有1。qは未使用 | Tなのでpだけでなくqのwork/peakも撤回。未使用/非共有の資源も時間量は撤回。非D Actor自身のwork/費用も保留 |

人は割当方策とDの占有に依存しうるため、D非担当の人でもworkを残さない。
資源の保持時間も待ち/Dの占有に依存しうるため、Dが使わない資源でもoccupancy_timeを残さない。
これは精密な因果閉包でなくU4の保守的射影である。

## 歯止めと検査の範囲

- coreの独立参照/engineには別々に手値を適用する。両者の同じ射影の誤りは相互一致では検出不能。
- CLIは**同一Report**をwire-only/source-boundの両入口へ通し、答えの一致と受理を要求する。
  source-boundは選択processのend/commitとcontest宣言をASTから独立に結ぶ。
- 手組みwireは4先行理由×14単一故障、hardの4先行理由×4単一故障を拒む。
- 37構成からの93 invocationは現binaryのraw出力を両照合器へ通す。
  再レビューの8構成はcheck/explainの16 invocationを追加し、計109 invocationとする。
  旧観測byteと許可位置表はADR 0055で廃止した。手値は上の算術表から検査し、
  過去の採取値や現snapshotを意味のoracleへ転記しない。
- 現productionのREDは修正完了ではない。早いassert以降の未到達箇所を実行済みとはしない。
