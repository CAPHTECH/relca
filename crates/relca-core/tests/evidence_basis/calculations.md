# evidence_basis の段階 1: 独立した手計算と受入の範囲

## 1. 計算の前提

ADR 0070 の段階 1 と、その追記の診断・証跡の裁定を期待値の根拠とする。
入力の正本はこのディレクトリの `m0.relca`、`m5.relca` と `inputs.py` の単一編集である。
現在の実行結果を意味の期待値へ写さない。

source Actor の集合を集合として計算する。I = Invoice、P = PurchaseOrder と略す。
E(s) は直接の reads と引数の祖先、および支配する観測変数の書き手の和集合。
E_d(s) は直接の source Actor の reads だけ。
契約は正常終了時だけ発火する。
制御依存、潜在の kernel 入力、契約自身の要求をこの和集合へ足さない。

この模型の結果は非決定である。pass と fail の 2 経路へ、それぞれ 1/2 の確率を付けない。
以下の 1/2、2/2 は候補の件数の比だけであり、確率やモデルの網羅率ではない。

## 2. M0 の分数による再計算

各作業の長さは 1/1 分。開始時刻を 0/1 分とする。

| 事象 | 時刻（分） | 根拠源 |
|---|---|---|
| e = enter の終了 | 0/1 + 1/1 = 1/1 | E(e) = {I} |
| p = match_po(e) の終了 | 1/1 + 1/1 = 2/1 | E(p) = {P} ∪ E(e) = {I,P} |
| p = fail の returned | 2/1 | この経路に a の終了は無い |
| p = pass の a = approve(e) の終了 | 2/1 + 1/1 = 3/1 | E(a) = E(e) = {I} |
| post の終了・posted | 3/1 + 1/1 = 4/1 | E(post) = E(p) ∪ E(a) = {I,P} |

逐次で待機が無いので、returned の work = 1/1 + 1/1 = 2/1 分。
posted の work = 1/1 + 1/1 + 1/1 + 1/1 = 4/1 分。
reachable posted は存在量化なので、returned 経路もあってよい。

### 受入 1: 直接の reads

要求 {P} ⊆ E(p) = {I,P} は両経路とも時刻 2/1 に成立する。
match_po の reads P を削ると E(p) = {I}、{P} − E(p) = {P} になる。
正常終了は残るので vacuous でなく violated。
反例の終了事象は enter(1/1)、match_po(2/1) の順。

### 受入 2 と F1: 引数と direct の違い

{I} ⊆ E(p) = {I,P} は成立する。
{I} ⊆ E_d(p) = {P} は成立せず、missing = {I}。
direct と系譜を同じにする F1 は、この正負を区別できない。

### 受入 3 と F3: activation の無い対象

recheck の宣言を足しても呼出しの回数は 0/1 回。
E(recheck) = {I} という宣言だけでは契約を満たさない。
正常終了が全経路に無いので vacuous、exit 5。
mutate は元の検査で止まり、実行した変異体は 0 件。
match_po を対象にすると 2 本とも終了するので satisfied。

### 受入 4 と F2: ガードは引数ではない

p = fail は returned へ進み、approve を発火させない。
p = pass は approve を時刻 3/1 に終了させるが E(a) = {I}。
ガードが p を参照しても P は E(a) に入らず violated。
approve(e,p) に直すと E(a) = E(e) ∪ E(p) = {I,P} となり satisfied。

requires po == pass と wait until po == pass も制御依存であり同じ区別を保つ。
これらの模型では p == pass の既存ガードの後に待つので、実際の待ち時間は 0/1 分。
既存の wait の出力は単一案件でも case_outcomes と cases を使う。
反例は enter=pass(1/1)、match_po=pass(2/1)、approve=pass(3/1)。

## 3. M5 と U4 の再計算

M5 の enter は pass / fail の 2 結果を取る。
pass 側では match_po が po を書き、fail 側では書かない。

| 経路 | 時刻・作業量 | 実際に走る書き手 | approve の根拠源 |
|---|---|---|---|
| enter = pass | 1/1 + 1/1 + 1/1 + 1/1 = 4/1 分 | match_po | I と、条件付きの P |
| enter = fail | 1/1 + 1/1 + 1/1 = 3/1 分 | 無し | I のみ |

may 集合は {I,P}、must 集合は {I}。
P は支配しない辺からしか届かない。
他に確定した違反が無ければ unsupported / U-SEM-0 evidence_basis_conditional_snapshot。
これを satisfied にする F4 を拒む。

if を取り去った dominating は、両結果とも match_po が必ず先に終了する。
2/1 分で po を書き、3/1 分で approve がそれを読み、4/1 分で posted。
must 集合も {I,P} になり satisfied。

条件付き po に加えて直接 reads PurchaseOrder を残す対では、P に到達する must 辺があるので satisfied。
独立した source Missing を要求へ追加した対では Missing は may 集合にも無い。
既知の欠落を conditional_snapshot で覆わず violated とする。

## 4. F5〜F7 と呼出し単位

- F5: enter の reads Invoice を取り去り、approve の kernel が Invoice.difficulty を given と kernel_reads latent で読む。E(e) = {}、E(a) = {}。Invoice を要求すると violated。Invoice の直接 reads を approve に足した対だけ satisfied。潜在の値 false の質量は 1/1、kernel の pass の条件付き質量も 1/1 だが、これを観測の辺に変えない。
- F6: conjunction の元と enter の reads を削った対を、要求の有無で比較する。agency 全体（origin・lineage・lineage_groups・refusal・viewpoints を含む）と responsibilities、W-LIN-2 は byte 単位で同じ。欠けた I を契約から補うとこの不変条件か変異検出のどちらかが崩れる。
- F7: Ledger の効果の前に observed ready == true のガードを置く。既存の導出は lineage を空にしうるが、approve の終了は時刻 3/1 にある。E(a) = {I}、要求 P は violated。引数 p を渡す対は satisfied。導出の行の有無を契約の発火条件にしない。
- 同じ approve を a=approve(e)、b=approve(p) と 2 回呼ぶ対では E(a)={I}、E(b)={I,P}。対象 Capability が同じでも a の欠落を b の集合で補わない。posted は 5/1 分、returned は 2/1 分。

## 5. 並行、取消、応答と再送

fork 模型は enter 終了 1/1 分の後、match_po(3/1 分) と approve(1/1 分) を並行に呼ぶ。
join all の終了時刻は 1/1 + max(3/1,1/1) = 4/1 分、work は 1/1+3/1+1/1 = 5/1 分。
match_po へ引数 I が届くので、その要求は satisfied。

join any は approve が 2/1 分で勝ち、match_po を即時取消する。
work は enter 1/1 + approve 1/1 + 取消までの match_po 1/1 = 3/1 分。
match_po の正常終了は無く、P の要求は vacuous。

応答の模型は同じ Finish を使う authority_basis の既存 DSL 対照も残す。

| 通信 | 時間の手計算 | 結果 |
|---|---|---|
| response_loss none | M0 と同じ 2/1・4/1 分 | evidence_basis は satisfied |
| unknown 1 回 | enter 1/1 + match_po 1/1 = 2/1 分を観測 | Finish の契約は satisfied、案件は receipt_unknown、全体は inconclusive |
| unknown → delivered、backoff 1 分 | enter 1/1 + 1 回目 1/1 + backoff 1/1 + 2 回目 1/1 = 4/1 分を観測、work は 3/1 分 | 非冪等の呼出しでは後の delivered だけで過去の unresolved を消さない。契約は satisfied、全体は inconclusive |
| result fail で最大 2 回 | 初回 pass は posted 4/1。再試行 pass は posted 6/1、work 5/1。再試行 fail は returned 4/1、work 3/1 | 各 Finish の契約は satisfied |

観測上限までの時刻を completed_at、elapsed、work に移さない。
receipt_unknown は observed_elapsed 等へ記録し、完了の欄は null のまま。
recovery の拒否対照は、既存の冪等な Ledger の実装を明示した入力を使う。
新契約を除いた対照の停止時刻は 1/1 分、案件は未完了。

clock を 1 分へ縮めた対では match_po はまだ終了していない。
契約は satisfied / vacuous でなく inconclusive。
二つの案件を同時刻 0 分に入れる対は、各案件の違反を cases に残す。

## 6. mutate の件数の再計算

候補は enter.reads.Invoice と match_po.reads.PurchaseOrder の 2 件。
要求 {I,P} の元は satisfied。

| 削除 | E(p) | missing | evidence_missing |
|---|---|---|---|
| enter の I | {P} | {I} | satisfied → violated、detected |
| match_po の P | {I} | {P} | satisfied → violated、detected |

検出件数比は 2/2 = 1/1。契約の無い旧入力は独立源の喪失だけを検出し 1/2。
差は 2/2 − 1/2 = 1/2 であり、利用者の実モデルへは一般化しない。

original_access_removed に direct の要求 {P} を割り当てると、enter の I の削除は範囲外で no_worsening、match_po の P の削除だけ detected。
系譜型の要求だけのときは同種へ割り当てず、両方 no_assigned_check のまま。
未検出が残る mutate の総合 verdict は既存規約の violated / exit 1。
2 件とも detected の evidence_missing は satisfied / exit 0。

## 7. 既存 DSL による照合と保証の限界

`inputs.old_witness` は、手で計算した membership を observed basis_ok の初期値へ置き、既存のイベント invariant の前件を対象の Finish にする。
これは発火時刻、取消、到達、vacuous、反例の終了順、case ごとの集約の等価な有限対照である。
新しい source 集合の計算を実装しているのではない。
条件付き snapshot の unsupported 自体には既存の同値な構文が無いため、M5 と dominating の実行経路・時間を別々に照合する。
静的拒否は追記で裁定された code と reason に照合し、未知の対象は既存 capability_path の undefined Actor.Capability に揃える。

単一の通常案件の性質行は既存コードでは id と verdict だけで、cases は省略される。
arrivals、wait、response の入口では cases を持つ。ADR の id/verdict/cases の説明を必須キーの追加と解釈しない。
simulate は観測した違反なら violated、反例が無い契約の全経路の安全は inconclusive。
案件の終端件数は case_outcomes を読む。
explain の evidence は反例 object の欄で、binding、actor、capability は steps と同じく分離する。
conform は正常な header のみのログを使い、reachable を入力から外す。
evidence_basis は既存の未対応契約の経路で U-LOG-1 / unsupported、not_checked の unsupported_request と unchecked の evidence_basis の両方へ残す。
比較の simulation 対照は別々の process を選び、sensitivity manifest の confidence は既存 wire の正規形である文字列 1/2 とする。

## 8. 検査の対応

| 新規試験 | 主な実装単位 | 対応する故障 |
|---|---|---|
| Behavior.test_a1 / a2 / conjunction | 1,2,3,4 | F1、直接・上流の削除 |
| Behavior.test_a3 と vacuous_mutate | 4,7 | F3 |
| Behavior.test_a4 / f2 / f5 | 2,3,4 | F2、F5 |
| conditional_snapshot / dominating / definite_violation / direct_alternative | 2,3 | F4 |
| f6_contract_never_feeds_derivation | 3 | F6 |
| f7_no_lineage_row と fixed_arg | 3,4 | F7 |
| check_no_evidence / guard_counterexample | 6 | Finish と証跡の誤り |
| mutate_conjunction / u7_direct_only | 7 | F1、F6、誤った種類9割当て |
| fork / response / clock / two_cases | 2,4 | activation、取消、切断、案件混同 |
| Static の各拒否 | 1,2 | 名前・型・組合せの黙殺 |
| Gates の compare / diff / sensitivity / conform | 6 | 未対応入口の黙殺 |
| Rust の stage_one_each_hand_case | 4,5 | engine と reference をそれぞれ手値へ照合 |
| Controls と Rust の old_dsl_witnesses | 保存条件、8 | 既存 DSL と時間の保存 |

F1〜F7 の実装故障注入は、新契約を実装してからこれらの受入へ実際に注入する必要がある。
構文未実装の RED を、その故障の検出実績とは数えない。
GREEN の時間対照は opt-in の入力故障で 1 分を 2 分へ変え、同じ期待が落ちることを確認できる。
Rust の旧 DSL 対照には、構文を壊す opt-in の試験専用故障もある。
