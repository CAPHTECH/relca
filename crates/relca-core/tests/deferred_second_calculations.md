# ADR 0053 再レビューの手値

裁定は当時の再レビューのmajor 1 / minor 1〜3に基づく。受入の要件を以下に記す。
時間の単位は分、費用はJPY。期待は実行前に次のように固定する。

| 模型 | deferなしの算術 | Tの手値 |
|---|---|---|
| immediate | R.y=2、A.x=5を時刻0に開始。RのpassでAを即時取消。A.work=2、A占有費=2、A開始費=3、R占有費20+開始費4、合計29、work4、lead2 | A/Rのwork・占有費、A.x/R.yの開始費、Task.workを含め全時間費用がnull。startsは各1 |
| horizon | R.y=2の後にA.x=5を開始。horizon3で切る。A.work=1、占有費1、開始費3、R占有費20+開始費4、観測合計28、work3、cut_off1、completed0 | 同上。horizon3とarrival0は入力として保持。dwellもnull |
| unfinished_route | Judge=1の後に任意のcontest route=5。horizon2で閉じる。routeを起こす/起こさない両枝、Review.work=0..1、占有費0..10、通常終端費1、観測費1..11 | defer Judgeでも非D Reviewのwork・費用を保留。通常終端は1件。route開始の分岐はCoreのまま |
| shared_resource / shared_person | 既存join all模型。A.work=1、A占有費1、開始費0、R.work2、合計21、work3、lead3 | Aも非Dであることを理由に数値を残さない |
| if_none | A.xのfailが1分で確定しjoin anyのif none end returnedへ。returnedのcontest route Review.reply=2がBに入る。defer Review | wire/source照合器の両方が同じ正しい保留を受理。if none endを消した解析で非D Aの数値を要求してはならない |

## 指標の全欄表（BusinessMetrics DTO）

表の量欄を機械で走査し、Tでは数値なし・既存適用不能理由またはnot_configuredを要求する。
量の未知欄はpanic。全行の件数はdeferなしの対と完全一致。入力・行識別子は数値量ではない。

| 行族 | 全て保留する量欄 | 保存する欄 |
|---|---|---|
| Actor | work, occupancy_cost, peak_tasks, utilization, queue.wait_total.stats, queue.length_max, queue.length_time_average.stats | actor, capacity, bound |
| Capability | start_cost | capability, starts |
| Task | work, wait, predecessor_wait, refusal_wait, wait_causes.actor_capacity/resource/executor/combined/unattributed | binding, capability, starts, refused_count, never_created_count, cancelled_before_start_count, not_started_count |
| Person | work, peak_tasks | person |
| Resource | occupancy_time, peak_units, utilization | resource, capacity |
| Case | work, observed_cost_total, first_start_wait, external_wait, intentional_wait, terminals.lead_time/cost_per_case/breakdown, joins.branches.join_wait, dwell.until/breakdown | case_id, arrival, counts, terminal kind/end/count, join fork/mode/binding, bound |
| Breakdown（terminalとdwell） | external, in_service, contest_route, queue.actor_capacity/resource/executor/combined, intentional, unattributed | なし |
| Terminal集約 | cost_per_case | kind, end, count, completed_cases_total |
| Overall | observed_until, work, named_people_work, peak_tasks, occupancy_cost, start_cost, observed_cost_total, observed_cost_per_arrival, queue_wait_observed, intentional_wait | counts, business_valueの既存未設定 |
| Header / Rates | なし（入力） | rules, status, window, currency, time_basis, lead_time_scope, observation_scope, statistical_unit, estimate_note, rates, checked, unchecked |
| Quality | 確率・期待件数・比・分位点の数値（前回の表を継承） | hits, trials, unknown, unknown_count, observations, censored等の構造件数と定義 |

Coreのterminals.elapsed/work/result_at、verify、AgencyはWorkload指標ではなく従来どおり。
minor 1は手でcase.min/maxだけnullへ変える。deferなしReportはtruncated/undeterminedとも拒否、
deferありcheckedのReportは受理。非nullの従来0..1はdeferなしで受理。
minor 3はdeferなしsimulation Reportでcohort.valueだけnot_configuredへ変えたwireを
実evaluateへ渡す。eval-sim-ratioの反復は1回、GoodのWrongはhits0/trials1、Badはhits1/trials1、
Registeredは両案hits1/trials1。この保持件数からhard区間を再構成せずinconclusive/nullとする。
