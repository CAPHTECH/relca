# 記録の由来: 手計算と受入の境界

根拠は ADR 0075 の採択本文と採択後の裁定。出力から期待値を採取しない。
`r0.relca` は本文の R0 を手で転記した入力、`inputs.py` は一箇所ずつ変えた対である。
Python の `Fraction` と core の二つの実行器で、整数を分母 1 の分数として再計算する。
非決定性には確率を置かない。以下の 1/4 は経路の本数の比であって成功確率ではない。

## 1. 全 5 例の再計算

1 操作の時間は 1/1 分。待ち・並行は無い。
I = Invoice、K = Know、N = Notes と略す。E は正常終了した呼出しの must の源の集合。

| 入力 | review 終了までの和 | posted / returned | 手計算の集合と判定 |
|---|---|---|---|
| a1_r0 / a3_single_result | 1/1 + 1/1 + 1/1 + 1/1 = 4/1 | 5/1 / 4/1 | E(e)={I}, E(r)={K}, E(b)={I,K}。Basis satisfied |
| a1_registrar_blind | 同じ 4/1 | 5/1 / 4/1 | E(r)={} なので E(b)={I}、missing={K}。violated |
| a1_direct | 同じ 4/1 | 5/1 / 4/1 | b 自身の source reads は空。N は source でない。direct={}、violated |
| a2_dominating | 同じ 4/1 | 5/1 / 4/1 | r は e の値にかかわらず必ず先に終了。E(b)={I,K}、satisfied |
| a2_conditional | then は 4/1、else は 3/1 | then は 5/1・4/1、else は 4/1・3/1 | r の無い経路がある。must={I}, may={I,K}。K は may だけなので U-SEM-0 evidence_basis_conditional_snapshot |
| a2_never | 1/1 + 1/1 + 1/1 = 3/1 | 4/1 / 3/1 | 宣言はあるが呼出しが無い。E(b)={I}、violated |
| a3_result_dependent | 性質評価の前に拒否 | extrema を要求しない | 正常結果が ok と rejected の 2 個。名前では成功失敗を推さず U-SEM-0 registration_result_dependent |
| a4_overwrite_before | e,o,r,a,b の 5 × 1/1 = 5/1 | 6/1 / 5/1 | 最後の r が N を {K} にする。E(b)={I,K}、satisfied |
| a4_overwrite_after | e,r,o,a,b の 5 × 1/1 = 5/1 | 6/1 / 5/1 | 最後の o が N を {I} に置換。{K} を残さない。E(b)={I}、violated |
| a4_overwrite_maybe | overwrite 有は 5/1、無は 4/1 | 経路で異なる | {K} と {I} の共通分は空。b の引数から I は届くが K は may だけ。conditional_snapshot |
| a5_copy_of_copy | e,r,c,a,b の 5 × 1/1 = 5/1 | 6/1 / 5/1 | E(c)={K}、N2 も {K}。E(b)={I,K}。N と N2 は源として加算しない。satisfied |

R0 の a,b は各 2 通りなので 2/1 × 2/1 = 4/1 本。posted は pass/pass の 1 本、
returned は残り 3 本。1/4 + 3/4 = 1/1 は本数の検算であり、一様確率ではない。
dominating は e も 2 通りなので 2/1 × 2/1 × 2/1 = 8/1 本。
どの e の経路でも r は終了するので、8 本とも時刻 4/1 の E(b) に K がある。
草案の手値と食い違いは無い。

### 独立性

R0 の a は subject={I}, independent={K}、b は subject={I,K}, independent={}。
group の独立源の和は {K}、個数 1/1。コピー自身は新しい源ではない。
copy-of-copy にしても 1/1 のまま。旧の source record N を使う対では independent={K,N}、
個数 2/1 のままである。旧の意味を登録として自動修正しない。
Agency の subject は may の上近似なので、overwrite-after の旧 snapshot 書き換えで
subject に K が残ることは、must の Basis に K を入れる根拠にしない。

## 2. 追加の由来・拒否・境界

- f5_latent_not_observation: Know.sure は kernel の入力だけ。E(r)={}、E(b)={I}、
  K は観測したことにならず violated。直接 reads [Know] を戻した対だけ satisfied。
  kernel は各行で 1/1:ok、時間は 4/1 と 5/1 のまま。
- f8_requires: eligible は e の結果(pass/fail)。pass なら r は 1 分で正常終了するが、
  fail なら else rejected を返して登録しない。r を must にできず conditional_snapshot。
  requires の変数は由来のデータではない。ok という正常結果は 1 個なので U5 の拒否とは別。
- u4_relayed_argument: k=judge(e) が {I,K} を持ち、register(k) は自分では source を読まない。
  E(r)={I,K}、両方 relayed。e,k,r,a,b は 5/1 分。posted=6/1、returned=5/1。
- u4_relayed_snapshot: judge が writes knowledge、register が reads knowledge。
  支配する must の書き手 k の {I,K} を登録する。引数の対と同じ時間・集合。
- u2_multiple_targets: 一つの r が Notes2 と Notes に同じ {K} を登録する。
  b は両方読むが {K}∪{K}={K}。時間は増えず 4/1 と 5/1。
  via_records は Notes、Notes2 の昇順であり、宣言順には依存しない。
- u4_control_not_data: blind な r を a の pass の分岐に移す。a の結果は pass のみ。
  e,a,r,b は 4/1 分だが、条件 a の {I,K} は E(r) に入らない。
  r は入力も source reads も持たず E(r)={}、E(b)={I} なので violated。
- u7_same_source_conditional_overwrite: r の後に条件付き r2=Clerk.register() を置く。
  最後の登録者は r または r2 だが、両者の {K} の共通分は {K}。
  source 単位の must は K のまま。Basis satisfied、via_records は 1 要素に
  registrars=[r,r2]、sources=[Know:registrar_direct]。b は 4/1〜5/1、
  posted は 5/1〜6/1、returned は 4/1〜5/1。登録者ごとに源を二重に数えない。
- source の document / record / tacit はすべて E-CON-0 registration_target_is_source。
  capacity / executors / Capability / state / latent は各単独で
  E-CON-0 registration_target_has_behavior。Actor の名前では登録先と推定しない。
- Notes を evidence_basis の要求源に書いても source ではないので
  E-CON-0 evidence_basis_not_source。Cut から外した登録先への登録・読取りは E-CUT-1。
- repeat の登録者・読取りの両方は U-SEM-0 loop_feature_combination。
  応答喪失・再送・recovery・縮退・異議経路の双方、および start 効果と registers の併用、
  印の無い案件数 2 は U-SEM-0 registration_feature_combination。
  fork の兄弟の登録者・読み手は U-SEM-0 read variable has a concurrent writer。
- per_case の Notes は案件数 2 を受ける。各案件に satisfied を要求し、全案件の源を混ぜない。
  この対の全体の経過時間は資源競合のため 1 案件の手値を流用しない。
- Know.sure を if の条件で読むことは引き続き E-POL-LATENT。

## 3. 観測と責務の読み分け

via_records は record ごとに 1 要素。その中の registrars に call と people、sources に
source と route を保持する。元の草案の source ごとの要素を期待に使わない。
R0 の q1 と Reviewer と段 b の経路は Notes / r / [p1] / Know:registrar_direct。
copy-of-copy の b は Notes2 / c / [p1] / Know:relayed。
空の登録・無登録・latent-only は空配列。may-only も空配列だが人の indirect には K が残る。
段階 1 で may 専用の配列は追加しない。段の via_outputs は引数・観測変数由来だけを保つ。

責務の対は e の reads を Invoice から Know へ替え、q1 for Reviewer.review を足す。
これで既存 sources() が要求する源も {K} になる(記録だけの辺を要求に混ぜていない)。
q1 は直接読まず、must 記録で K が届くので行は present_via_record、missing_sources={}。
ただし段 b の required={K}, direct={}、missing={K} は残る。
同人が別の段で直接 K を見ていないので W-RESP-1=0、W-RESP-3=0。
Reviewer の実行者を p1 に替え、責務も p1 に替える対では、全体 direct={K} なので
行は present、段 b の missing={K} と via_records は同じ、W-RESP-3 は 1 件。
記録を読んだことを直接の観測に格上げして警告を消す実装を拒否する。
may の対では q1 の行に K は欠けたまま。W-RESP-1=1、W-RESP-3=0。

### 共通誤りと W-LIN-2

Producer は bad を書き、A と B の kernel は given bad を使う。
A の各行は 1/2:pass と 1/2:fail、和は 1/1。入力ではそれぞれ 0.5 と書く
(既存の DSL の確率リテラルは分数表記を受けない)。条件付き登録には実際に登録しない経路もある。
B の各行は 1/1:pass。これらは明示した確率であり、非決定性の一様化ではない。
A の源は {Invoice,Other}、B は引数の {Invoice} と Notes の由来に依存する。
条件付き登録の K は B にとって may だけ。B の must={Invoice} と他判定の may を比べても
B に独自の源は無い。B の W-LIN-2 が 1 件出る。Notes を独立の明示源にすれば消えるので、
この誤った抑止を検出できる。一方 must の登録の B は独自の tacit K を持つので、
W-LIN-2 は 0、wlin2_tacit の開示は必須。source record としての無開示の抑止は許さない。

## 4. コマンドの入口と既存 DSL の書き換え

- check/explain は網羅。blind/direct の反例は e(1/1),r(2/1),a(3/1),b(4/1) まで。
  actor と capability は trace の別々の欄。上書き後の反例は e,r,o,a,b、最後は 5/1。
  missing={K}、direct の sources={}、それ以外の sources={I}。
- 条件付き r と別の確定の不足源 Missing を要求する explain では violated を先に出す。
  missing={K,Missing}、sources={I}、excluded_edges=[r -> b]。
  採択後の U9 裁定により、evidence は既存の 7 欄のままで、記録名の別欄は追加しない。
- simulate は各判定の結果を pass だけにし、反復 4、seed 7、confidence 1/2。
  安全性(Basis)に反例が無い標本は inconclusive、blind/overwrite-after は violated。
  案件ごとの終端は case_outcomes に end posted が count 4。
- mutate evidence_missing の Clerk.register:reads:Know を外す変異は、
  元の satisfied から violated に悪化する Basis により detected。
- compare(両モード)、diff、sensitivity(両モード) は
  U-SEM-0 registration_unsupported_command。構文エラーや無関係の理由では通さない。
- conform は登録の由来をログで観測しない。空の有効なログの inconclusive を保持し、
  unchecked に registration_provenance を残す。証跡を補わない。
- clock 3/1 は時刻 4/1 の review 終了へ届かない。Basis も到達性も inconclusive、
  truncated=true、terminals は空。未完了案件を失敗や成功へ変えない。

`legacy()` は Notes/Notes2 を optional<Ack> の観測 case 変数 note/note2 に手で写し、
registers を writes、記録の reads を観測変数の reads に置き換える。
記録の Actor と Cut 宣言は空のまま残す。値は業務判断に使わない。
全 5 例・壊した対・F5/F8・中継・複数先について、この既存構文を parser/lower に通し、
engine と独立 reference の双方に上の手判定を要求する。
R0・blind・overwrite-after・copy-of-copy では engine の公開 evidence table を空にしても
reference が手判定を再構築し、engine だけが壊れた表を拒否することも要求する。
新しい表示の等価性までは主張しない(既存の観測変数には record の識別が無い)。
U3 の静的入力は登録と読み手の辺だけを既存構文へ写して周囲の構文を検査する。
未対応の機能の足場も同様。fork は既存 snapshot の同じ拒否になる。
recovery は有限のキー集合を持つ atomic/idempotency 操作を使い、登録・読取りを
受領不明の handler に置く。異議経路は引数なしの route と明示の reversal authority を持つ。
既存の別の拒否理由で、新機能の境界の検査を代用しない。
真偽値そのものの条件 if true は使わず、すべて変数・結果を比較する。

## 5. 故障の対応と互換の検査

| 故障 | 落ちる新しい受入 | 初期状態の校正対象 |
|---|---|---|
| F1 may を must にする | a2_conditional、a4_overwrite_maybe | 既存 snapshot の両対も unsupported |
| F2 記録を独立の源にする | R0 と copy-of-copy の lineage/group、multiple | source record の旧対だけ 2 源を保つ |
| F3 登録者を見ず知識を付ける | registrar_blind | 既存 snapshot の blind は violated |
| F4 全書き手の和を must にする | overwrite_after | 同名 snapshot の上書き後は violated |
| F5 kernel の潜在入力を観測扱い | latent_not_observation と direct_control | 潜在だけは violated、reads の対だけ satisfied |
| F6 may を O に数える | Observation の may-only と W-LIN-2 | 記録の新表示は未実装なので初期は RED |
| F7 記録経由を direct にする | a1_direct、段 missing と W-RESP-3 | 既存 snapshot の direct は violated |
| F8 requires の登録を must にする | f8_requires | 既存 snapshot の対も unsupported |
| F9 旧入力が変わる | Preservation と Golden の stdout/stderr/exit | 既存 golden を更新せず全件 byte 照合 |

GREEN の旧構文試験は一時的な入力の時間の 2 倍化、または verdict の wire 故障で
落ちることを確かめ、入力とソースを変えずに元の実行も確かめる。
core の旧構文試験には明示的な環境変数で構文を壊す seam を持つ。
F1〜F8 の production への実故障注入はまだ登録の実装が無いので実施できない。
初期 RED を実装後の故障校正と取り違えない。
全 golden の比較は新しい試験から既存の非更新比較器を使う。snapshot は意味の手値ではない。

## 6. 文書とコードの境界

- 草案の行番号・0074/0076 未読の記録は起草時のもの。採択の追記が現行の境界である。
- 責務の sources() は観測変数の書き手をたどらず、引数と source reads だけを要求にする。
  記録の導入を理由に旧入力の責務の意味を広げない。
- 草案の source 単位 via_records と may の別欄案は採択後の裁定で置き換わった。
  record 単位の形と段の missing の裁定を本受入の正本にする。
- 設計 v0.2 §9.1 の読取りの説明には観測変数の書き手の辺の明記が無いが、
  既存コードの must/may を書き換えの基準として使う。
- ADR 0034 の source 種別を読まないという記述に対して、既存 common_cause は
  Tacit を読み、wlin2_tacit の開示を分ける。新しい検査もこの開示を維持する。
