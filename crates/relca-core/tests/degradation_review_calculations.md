# Q6b merge 前レビューの追加手値

基点 00d2a072。ADR 0064 の 5.2 節、8 節と Q6b review 裁定を正本とする。
新しい実出力を期待値へ写さない。既存 q6b_oracle の A7/suffix 手集合を再利用する。
数値は Start 回数、時間は分。実技能・保持根拠・SLA の保証ではない。

## MJ1 と m4 calendar

Always は [0,11) の新規開始を許す。Probe=1、Clerk=3、Fast=1、Finish=2。
suffix は分岐の後で一度呼ぶ。到着0なら Probe 0..1、縮退 Clerk 1..4、
Finish 4..6、通常 Fast 1..2、Finish 2..4。各側の二作業は各1..1、
Probe は common_prefix 1..1、側では0..0。既存 suffix 手表と同じ。
到着9なら Probe 9..10、Clerk 10..13、Finish の最初の開始候補は13。
13 は through11の外なので Finish を must や derived 1..1 にしない。
裁定に従い calendar/not_derived、又は到達しない側に限る。
計算clock10は意味上のカレンダー上限ではなく、この手表の導出に使わない。
別対: 到着0でも Clerk=20なら Finish の候補は21で範囲外。
この対は既存の coverage 補正を守り、offset の穴とは分ける。

## U2 宣言結果 domain

Status={ok,fail}。Probe の単一値ok、result nondet {ok} を宣言したとき、
宣言外 fail の縮退側 done は reachable=false。全6 Capability の側別countは0、
may/must/Oは空。通常側は Fast 1..1。Probeはprefix1..1。
単一値fail / result nondet {fail} の対は通常側だけが到達不能、縮退Clerk1..1。
U2補足裁定: result ok/fail と結果宣言無しは現DSLの文法を開かない。
単一値は公開IRのresultsをその値だけにする。両実行入口で受理を先に確認してから
agency::deriveを上の手集合へ照合する。
宣言無しのIR(results空)は両実行入口で検査の誤りとして拒否し、終端無しを要求。
無関係な実行時non-progressingエラーだけでは合格しない。拒否IRをderiveへ渡さない。
Probeだけでなく、未使用Other.enterのresultsだけ空にした別の対も拒否する。
未使用宣言は実行時に遭遇しないため、遭遇時だけの拒否ではこの対は通らない。
宣言無しでenum全値という規則は、それを受理する入口の無い現DSLの実行保証ではない。
kernel {1:ok;0:fail} と {0:ok;1:fail} も両方の結果を構文で宣言している。
質量0を含む宣言和なので、両方とも同じ A7 手表。実行確率は表へ混ぜない。

## m4 撤回の境界

- 二案件が同時刻0に到着し共有する人・容量を使う場合、案件を独立な構造Startと
  扱えない。理由 multiple_case_interference。m8補足により表全体をnot_derived、
  routes=nullとしprefixも撤回。単一案件ではA7手表そのまま。
- Clerk の atomic が marked を読み、falseならmarked=trueとok、trueならfail。
  結果は履歴依存。shared_state_guardで縮退側のends/Oをnullへ撤回。
  これを宣言nondet {ok,fail}へ替えれば結果は未使用なのでA7の依存表そのまま。
- Seat capacity1、Clerk uses2なら開始不能。resource_impossibleで縮退側撤回。
  uses1なら逐次・単一案件で干渉なし、A7手表。死枝でも字句だけでStartを作らない。
- calendar外の対は上記 Clerk20/3。撤回時は偽の0回も出さない。

## U3 と m7 decode

clock3のA7では通常側のEndがあっても縮退側が未完了。bounds.truncated=true、
root inconclusiveが正。rootだけsatisfied又はvacuousへ替える負の対を拒否する。
新degradeだけの安全側gateであり、同じ実行木の通常Ifのroot契約は変えない。
宣言仮定 agency.degradation.declarations と表の有無は双方向に照合する。
新表があるとき agency.retention.normal.scope=whole_process_paths が必須。
正例は完全なwire。負例では独立のledger/uncertaintyの参照欠損を原因にせず、
仮定や表の一変更に従って派生した参照だけを整える。

## m5 公開mode

Probeの宣言結果はokのみ、両側End done。問い probability reached end.done >=1 は真。
degrade版のmutate/conform/sensitivityはunsupported、exit4、U-SEM-0、
q6_feature_combination。通常If版の同じ模型はこのQ6理由を持たない。
sensitivityの正例は同一模型の2点、同一問い、確率1..1、exit0。
conformの正例はProbe/Fastの開始終了と案件の完全観測範囲宣言、conformant/exit0。
conformは確率verifyを未対応とする既存契約なので、そのpairの両側からだけ
確率verifyを外し、requires_event Fast.run.start after Probe.check.finish(ok)を照合する。
ログは案件の完全観測範囲を宣言し、二Taskの開始終了を記録する。Endログは使わない。
mutateの旧modeは既存未対応種のためexit4のままでも、通常の列挙段階まで進む。
Q6拒否gateと既存の未対応種の拒否を同一視しない。

## m6 探索

他のエラー無しで注記を失う入力が作れた場合にだけ受入を追加する。
作れない経路と根拠、探索実行の結果は別の証拠を要する。期待不明の診断語は作らない。
