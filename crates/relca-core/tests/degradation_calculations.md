# Q6b 明示縮退の独立手計算（ADR 0064）

基点 78b71606。正本は ADR 0064 の 5・7・8・9 節と Q6 checkpoint 裁定。
この記録の算術と経路集合は production の実装を読まず、実行結果を採取せずに固定する。
時間は分、work は Task・分、回数は Start 回。全模型は一案件、流入時刻 0 分。
確率を指定しない nondet の枝に 1/2 等を与えない。単価は未設定であり費用は 0 円ではない。

## A7: 既知の結果だけで明示縮退する

Probe.check は 0..1 分、結果 domain は {ok,fail}。共通 prefix は Probe.check の 1 回のみ。
status==fail が真の側を degraded、偽を normal とする。どちらの側も end.done。

| 側 | 順序 | 終了時刻（分） | work（Task・分） | その側の may = must |
|---|---|---:|---:|---|
| normal | Probe 0..1、Fast.run 1..2 | 2 | 1+1=2 | Fast.run |
| degraded | Probe 0..1、Clerk.enter 1..4 | 4 | 1+3=4 | Clerk.enter |

end.done の到達は全枝で真。check は satisfied、reachable 問の exit は 0。
終了/work の範囲は [2,4]。平均・障害確率は未入力なので出さない。
Clerk.enter の側別回数は normal 0..0、degraded 1..1、旧 retention.normal は
process 全体の 0..1 のまま（whole_process_paths）。Probe は側の回数に再計上しない。
縮退側の直接 O は Clerk→Doc、通常側は Fast→Summary。保持根拠は unknown。

一変更対 other: 縮退内の c = Clerk.enter() だけ c = Other.enter() に置換する。
Other は所要 2 分、reads 無し。終了/work は normal 2、degraded 1+2=3 分。
Clerk は両側 0..0、degraded may/must は Other.enter、O は空。
字句の keep は残さない基本対とし、別の keep 付き対で同じ依存値を要求する。
意味の変化を Agency 表で検出する対なので専用診断 code は無い。

## A8: 迂回、ガードの充足性、End 別の依存

Probe の後、縮退側で Review.choice を 1 分実行し、pass なら Clerk.enter（3 分）、
fail なら Other.enter（2 分）。Review の domain は {pass,fail}。

| 構造経路 | 条件 | 終了/work（分 / Task・分） | 側内 Start 集合 |
|---|---|---:|---|
| normal | status=ok | 2 / 2 | Fast.run |
| degraded pass | status=fail, choice=pass | 1+1+3=5 / 5 | Review.choice, Clerk.enter |
| degraded fail | status=fail, choice=fail | 1+1+2=4 / 4 | Review.choice, Other.enter |

degraded done の may は {Review.choice,Clerk.enter,Other.enter}、must は {Review.choice}。
回数は Review 1..1、Clerk 0..1、Other 0..1。共通 prefix は Probe 1..1。
whole-process の Clerk は 0..1、Review 0..1。normal done は Fast 1..1。
一変更対 must: fail 側の Other.enter を Clerk.enter に置換する。
degraded done の may=must={Review.choice,Clerk.enter}、Clerk 1..1、Other 0..0、時間 5。
M8（may を must に昇格 / 迂回を無視）をこの差で検出する。

ガード対 guarded は pass 側で choice==fail のときだけ Other.enter へ分ける。
choice=pass と choice=fail の連言は偽なのでその経路は 0 本。
この impossible End は reachable=false、may/must は空、counts の全 Capability は 0..0。
通常の fail 側は end.alternate、pass 側は end.done。
したがって degraded done の must に Clerk、degraded alternate の must に Other が入り、
別 End の経路を混ぜて Clerk の must を消してはいけない。
一変更対 guarded-open は内側の choice==fail を choice==pass に置換する。
degraded impossible が reachable=true、Review と Other が各 1..1 になる。
degraded done は 0 本に変わり、全体終了/work は 2..4（最大 1+1+2=4）。
これは構造ガードの検査であり、kernel の数値や runtime bounds は使わない。

共通 suffix 対 suffix は両側の End を block の後の Finish.close（2 分）と共通 End に移す。
Probe は common_prefix、Finish は normal/degraded の両側に各 1..1。
normal 時間 1+1+2=4、degraded 時間 1+3+2=6。suffix を prefix と誤分類しない。

## A9: keep は依存ではない（既存構文の保存）

From は Clerk.enter、To は Fast.run だけ。To の intent に
retire Clerk.enter from normal_path keep fallback を置く。
To の終了/work は 1 分 / 1 Task・分、Clerk の retention.normal は 0..0、
fallback は kept_by_intent。retention の unchecked は fallback_dependency、retention_basis、actual_skill。
degrade が無いので agency.degradation 欄そのものを追加しない。
一変更対 no-keep は intent だけ retire Clerk.enter reason に置換する。
Clerk の count は同じ 0..0、fallback だけ unchecked に戻る。保持根拠はどちらも unknown。
wire に dependency=must を捏造する故障は decode/oracle で拒否する。
M8 の keep だけで must を作る故障を検出する。警告は verdict を変えない。

## A11: 潜在、既知 none、ReceiptUnknown

known-none は Probe 呼出しだけ response_loss none を明示する。結果が既知なので A7 と同じ
二つの実行経路・時間・work を保つ。prefix までの state/効果/義務を巻き戻さない。
latent-ok は Probe の kernel だけが潜在 secret を読む模型。secret は初期値 false、
kernel の false 行は ok=1、true 行は fail=1 を完全に宣言する。実行は normal のみで 2 分。
静的な宣言 domain は {ok,fail} なので構造 dependency は A7 と同じ（実行確率とは別）。
一変更対 latent は degrade 条件だけ status==fail から secret==true に置換。
check/simulate/explain は E-POL-LATENT、invalid/exit3、実行なし。
一変更対 unknown は known-none の response_loss none を response_loss nondet に置換。
新 Q1 交差は U-SEM-0、q6_feature_combination、unsupported/exit4。既知 fail への変換は禁止。
未知 binding は E-NAME-0、他の観測 state 条件は U-SYN-0（既知 binding 等値以外）。

## 導けない Start と交差

requires 対は Clerk.enter に requires false else ok を加える。
DSL では観測 ready=false を宣言し requires ready==true else ok と書く（裸の false 条件は旧文法外）。
実行は Probe の 1 分だけで縮退側の Start=0、work=1。normal は 2 分。
構造導出では requires のある対象行は not_derived、理由 requires、counts/may/must は null。
偽の Start=1 も、未知を既知 0 とする表も受理しない。
共有 state ガードも同様に shared_state_guard とする（正確な Start を導かない）。
calendar と priority の実可用性は静的表では not_derived、実行時間は手で別計算する。
calendar 対は Clerk の勤務 [0,3) 休止、[3,11) 稼働。縮退は Probe 1 分後から 2 分待ち、
Clerk 3..6 分、終了 6 分、work=4。通常は終了/work=2。
その他全ての実行 Actor と全 PersonId に [0,11) 稼働を明示し、未入力の勤務を補わない。
priority 対は一案件に固定した Q5 開始順位表を使い動的値は A7 と同じ、静的表は未導出。
wait/fork/入れ子 degrade/複数 degrade/再試行/contest/未接続 mode は
U-SEM-0 または U-SYN-0、q6_feature_combination（上限超過は q6_capacity_limit）で拒否。
費用未入力は常に理由つき null。Cut defer は既存の not_configured を保存する。

## wire の固定（受入補足裁定による推奨）

schema の具体鍵は受入側で決めることを orch が許可した。値は上記の集合から固定する。
agency.degradation の rules は agency-degradation-v1、scope は structural_guard_paths。
閉じた table/route/side/end/count/observation 型、unknown な鍵は拒否する。
構造 executors / qualified / permitted は別集合。保持根拠 basis=unknown、unchecked に
actual_skill と retention_basis。not_derived の量は null、reason は非空。
旧 retention の fallback_dependency は全体から一括削除しない。
ledger は実在 /agency/degradation/rules を参照し、uncertainty.model にもこの仮説を運ぶ。
これは wire の自己整合だけであり、運用での障害検知や実際の技能を認定しない。

具体的な閉じた鍵は次の表。配列は counts が Capability 宣言順、may/must が識別子昇順、
End はその側に字句上ある終端の識別子昇順。満たせないガードの End も reachable=false で残す。
counts は未使用を含む全 6 Capability を持ち、common_prefix の Probe は側の counts では 0。

| 型 | 鍵 |
|---|---|
| table | rules, status, scope, reason, basis, unchecked, notice, routes |
| route | name, condition, producer, common_prefix, normal, degraded |
| condition | binding, value（等値は構文で固定、value は DSL の表示文字列） |
| side | status, reason, ends, observation |
| End | end, reachable, counts, may_require, must_require |
| count | capability, min, max（整数、Start 回） |
| O | actor, capability, source, executors, qualified, permitted |

table の derived は reason=null、routes は配列。導出予算超過は status=not_derived、
reason=derivation_limit、routes=null。空配列や導出済みの部分集合にはしない。
side の not_derived は ends=null、observation=null。requires / shared_state_guard / calendar /
priority を理由語として受入で固定する。calendar は縮退側だけ、priority は両側を未導出とする。
O の executors は構造上の集合、qualified は未宣言なら null、permitted は明示又は既定集合。
qualified 対は executors=[c,z]、qualified=[z]、permitted=[c] で三者を同一視しない。

effect 対は Probe 終了時に marked=true、Clerk 終了時に manual=true を書き、
invariant manual==true -> marked==true を満たす。時刻 4 分に manual=true になる。
explain 対は結論だけ marked==false に替える一変更で、縮退時刻 4 分に違反し、
通常側では前件が偽。参照 replay と元の degrade の source span を要求する。
End を前件にする invariant は既存文法外なので使わない。

## 検査範囲

新 core/CLI target は全 DSL を公開 API/JSON で使う。実装未着手で RED になっても ignore しない。
参照 interpreter と engine をそれぞれ手値に照合する。相互一致だけでは合格にしない。
公開 decode と独立 wire checker を分け、単一故障 wire は各 checker の校正に使う。
derivation_limit の内部 seam は受入 support に固定し、cfg(test) の接続と実行の根拠を別に必要とする。
同じ support の route_ir_contract は、正常な a7 と、producer の範囲外・条件生産者の不一致・
縮退 call 対応の不一致を一つずつ入れた IR の拒否期待を固定する。
新 IR 欄がまだ無いので故障の適用と両入口への接続は未実行。engine は U-SEM-0 /
invalid_q6_ir、reference は SemanticAbort を要求し、正常対は Complete を要求する。
