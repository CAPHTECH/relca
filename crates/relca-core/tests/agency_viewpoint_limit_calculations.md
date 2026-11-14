# Agency viewpoints の上限指定と同値性の受入

導出と上限指定の決定は ADR 0072。読取基点は `6c6617cb`。
2026-11-14 に全 U の推奨を採択した。この受入の期待値は以下の経路計算から書いた。
旧 binary の採取値を意味期待値へ転記していない。snapshot は別目的の旧出力保存にだけ使う。

## 固定するものとしないもの

保存するものは、F の selector / read_only、L の行使回数と行ごとの未導出理由、
viewpoints 全体の型付き serialize の byte、verdict / exit、他 block の導出と旧入口の拒否。
変えるものは D の記録と W-AGY-5、invalid_ir の分離、S1 / S2 で導ける範囲、CLI の明示倍率と ledger。
work / cells 以外の上限と degradation の予算を増やしてはならない。

2026-11-14 の受入条件の補足で、次の区別を固定した。

* k12 の旧列挙器が既定 work に当たることは固定する。最適化後の k12 の未導出を要求しない。
* 最適化後の gate_end k20 は既定で導出する目標として固定する。
* k24、stage_and 6×4、all_and k11 の最適化後の work 境界は固定しない。
  実装後の検証記録に各 block の状態、記録、使用倍率、予算消費を測定結果として残す。
  導けた場合の意味値は下記で決まる。手数や時間の見込みを意味期待と混ぜない。
* 新しい DSL 構文は無い。新旧 DSL の書換えは恒等変換である。
  `support/agency_viewpoint_limit_inputs.rs` の全入力を既存 parser と lower に通す。
* 倍率 1000 の運用上限は ADR U7 の実装後 RSS 観測で再判断する。時間は合否に使わない。

## 分数による再計算

結果非決定性を確率とは読まない。以下の分母 1 は整数の厳密値を明示するためであり、
pass / fail をそれぞれ 1/2 の確率とする仮定ではない。二値の直積の大きさだけは 2^k と数えられる。

### gate_end k

done の経路は全 j が pass の一本。各 j を fail に変えると到達しないので全 j が all_contexts。
rejected の経路 r_m は j1〜j(m−1) が pass、jm が fail で、後続の新しい束縛は未定義である。
jm (m<k) を pass に変えても、未定義の後続 j を fail にすれば rejected に届く。
jm を fail にしても自分で届く。従って m<k は read_only。
jk は、先行すべて pass の文脈では値により止められるが、先行 fail の文脈では迂回するので some_contexts。
ただし k=1 に限り先行 fail がなく all_contexts(生成比較の k1 は手値の一般化の例外)。

行使回数は r_m で Intake=1/1、Judge=m/1、Ledger=0/1、done でそれぞれ 1/1、k/1、1/1。
全到達経路で min / max をとると Intake=[1/1,1/1]、Judge=[1/1,k/1]、Ledger=[0/1,1/1]。
従って k4、k10、k11、k12、k20、k24 の Judge の上端はそれぞれ 4/1、10/1、11/1、12/1、20/1、24/1。
1分の呼出しを直列実行する done の時刻は 1/1+k/1+1/1=(k+2)/1 分。
k10 は 12/1 分、k20 は 22/1 分。全体 200/1 分の clock 内である。

### stage_and S×W

第 t 段の rejected までに、各 Judge_i を t/1 回ずつ呼ぶ。done では各 S/1 回。
各 Judge_i の範囲は [1/1,S/1]。全 Judge の呼出しの和は rejected が tW/1、done が SW/1。
Intake=[1/1,1/1]、Ledger=[0/1,1/1]。
done の全 SW 個は all_contexts。最終段の判定は他の最終段 fail または先行段 fail で迂回できるので some_contexts。
それより前は自身の fail と後段の fail が相互に代替するので read_only。

* S5 W2: 判定数 10/1、各 Judge=[1/1,5/1]。a5_1,a5_2 が some_contexts、残り 8 個が read_only。
  done 時刻=1/1+10/1+1/1=12/1 分。
* S2 W4: 判定数 8/1、各 Judge=[1/1,2/1]。a2_1〜a2_4 が some_contexts、前段 4 個が read_only。
  done 時刻=1/1+8/1+1/1=10/1 分。
* S6 W4: 判定数 24/1、各 Judge=[1/1,6/1]。最終 4 個が some_contexts、残り 20 個が read_only。
  done 時刻=26/1 分。ただし既定の導出可否は未固定。

### all_and と Or、違反対

all_and k の全 k 判定はどちらの終端でも実行済みなので Judge=[k/1,k/1]。
done は全 pass が必要なので各 all_contexts。rejected は他が全 pass の文脈で値が効き、
他に fail がある文脈では効かないので各 some_contexts(k>=2)。
Or では反転し、done が some_contexts、rejected が all_contexts。
k11、倍率100の受入はこの意味を確認し、倍率1で未導出と決め打ちしない。

違反対は Ledger.post を連続して2回呼び、at_most_once を宣言する。
全 pass の経路で commit 数は 1/1+1/1=2/1 > 1/1。倍率1と100の両方で violated / exit1。
explain の commit event は actor=Ledger、capability=post の別欄で2件を数え、replay=verified を要求する。
simulate の safety は反例がない標本だけなら inconclusive / exit2、basis=sample である。
明示 0.5 / 0.5 の outcomes を持つ別入力で4回の反復完了を検査する。非決定性を一様乱数で補わない。

### G1〜G4 の独立の小さい対

* G1: 観測状態 flag の真偽で Ledger の0回と1回を分ける。
  状態ガードを緩めた同じ結果束縛の下で両 sink が同時に成立するため、Ledger は state_guards。
  Intake は両 sink で1/1のため taint されず [1/1,1/1]。
* G2: j1=pass かつ j1=fail は同時に成立しない。中の Ledger は [0/1,0/1]。
  And 一語を Or に変えると必ず Ledger を呼び [1/1,1/1]。この一変更対を両方検査する。
* G3: results が pass のみで version_check の reject=fail を持つ判定。
  正当な DSL のままなら version_check は状態ガードになり、F の終端は state_guards に閉じる。
  この DSL は参照比較集合に残す。これだけで値域の故障を校正したとはしない。
  2026-11-14 の受入条件の補足に従い、非公開の直接値域 seam を別に置く。
  状態ガードの無い記録済み sink j1==fail に、F/L の値域 {pass} を与えると
  rejected は reachable=false、selectors/read_only は空。主の値域 {pass,fail} なら reachable=true。
  この状態ガード無しの version_check sink は今の DSL からは作れない。故障注入専用である。
* G4: 上述の And / Or の各3判定。Or と否定 And を独立因子へ分解すると分類が変わる。

## 比較する母集団と参照の置き場所

参照は `crates/relca-core/src/agency/viewpoints.rs` に cfg(test) の
`with_limits_and_warnings_reference` を残す。
関数の引数と返り値は既存 `with_limits_and_warnings` と同じものを期待する。
現在の assignments / framing_shared / exercised の総当たりを、最適化と共有しない参照として保持する。
main の C1 前の参照は F/L の参照の代わりにはならない。

固定済み比較集合:
* gate、all_and、Or の k1〜8: 24入力。
* (S,W)=(1,2),(2,1),(2,2),(2,3),(3,2),(4,2) と serial/and/fork: 18入力。
* G1、G2、G3、requires、join any、Actor容量0、usesが容量を超える例: 7入力。
  合計49入力。全て parser/lower を通す。既定と倍率100で比較する。
* 全 fixture の model.relca。invocation.json の configuration / from / to / plans / scenario を保存。
  lowerで拒否されたものは両方式の同じ拒否を確認して記録する。到達しなかった F/L を比較済みとはしない。
* 旧参照が導けた block は既定で退行させない。旧参照が計算上限に当たった block だけ改善を許す。
  それ以外は Viewpoints 型の serde_json::to_vec の結果を無加工で byte 比較する。
* k20 の新方式(既定)と旧方式(試験専用倍率1,000,000)は全 viewpoints の byte を比較する。
  この試験専用倍率を CLI で受理してはならない。旧方式がなお上限なら比較成功とはしない。
* 保存済み全 snapshot の stdout/stderr/exit。新 ledger 欄を文字列から除く以外は byte 無加工。
  response-control の L 未導出の2件も例外にしない。新規 script に golden 更新機能は無い。

## 非公開 adapter の接続契約

production の公開 API を試験のために増やさない。agency.rs に cfg(test) で
`#[path = "../tests/support/agency_viewpoint_limit_private.rs"] mod agency_viewpoint_limit_acceptance;`
を接続し、親 module に次を供給する。

`fn adr0072_run(request: &agency_viewpoint_limit_acceptance::Request) -> serde_json::Value`

これは変換の薄い adapter であり、受入の期待値や参照を持ってはならない。

1. source を既存 parser/lower に通す。configuration/scenario を保存する。
   lowerが拒否なら status=rejected と diagnostics を返す。
2. reference=true は上記の凍結列挙器、false は本番と同じ新方式を使う。
   block=None は各既定上限。指定があればその block の counter を limit に置換してから scale を適用する。
   work/cells のみ scale 倍。counter の depth/paths/contexts は不変。
3. 返り値は status=ok、viewpoints(値)、viewpoints_bytes(型付き serialize の UTF-8 文字列)、diagnostics。
   外側の report 全体ではなく、viewpoints の生 serialize を比較する。
4. corruption=call_capability は、正常に lower した後、j1 の Call.capability を CapabilityId(usize::MAX) にする。
   corruption=origin_actor は、正常な Agency.origin の末尾へ direct=[Doc] の Origin を1件追加し、
   Actor の実数を超えた origin の参照を作る。対応する block の invalid_ir を観測する。
   主の再導出の invalid_ir で前処理を打ち切ってはならない。
5. revalidate=true は IR に scale を保持して主を再導出し、responsibility::validate を実行する。
   status=ok、responsibility_valid、model_scale、agency_derived を返す。
   責務と control party を持つ Or11 が scale1で主未導出、scale100で導出することも確かめる。
   mainやcommon_cause等の予算へも本番と同じ倍率を使い、degradationは変えない。
6. domain_probe=true は j1==fail の記録済み終端への F の存在判定へ直接値域を注入する。
   主の possible_results={pass,fail} で main_reachable を得て、F 本番と同じ値域引数化の経路へ
   {pass} を渡し、viewpoint_end を得る。adapter 自身で期待値を作らない。
   終端は rejected、状態ガード無し、call j1 と results={pass}、version reject=fail。
   この seam だけは DSL が生成しない内部入力である。
7. budget_profile=true は、本番が実際に各導出入口へ渡す予算を budgets に投影する。
   main/common_cause/observation/framing/retention/effects の各 work/cells/depth/paths/contexts と、
   degradation_work を返す。adapter が別の既定値や倍率計算を再実装してはならない。

`python3 -B scripts/test-agency-viewpoint-limit-private.py` は10件の試験名を列挙してから実行する。
接続なしの cargo test が0件で成功しても、この runner は RED にする。
現時点では adapter と参照関数が存在しない。これらの RED を実装済みの同値検証とは報告しない。

## 故障校正と実装単位

* D: 各 block の work=0 注入、旧列挙器 k11/k12 の境界、G8 invalid_ir と G9警告欠落。
  記録 counter/limit/requested/phase、警告 severity/code/件数、他 block 不変、部分結果の不在を照合。
* S1: k11 のL、G1 taint、G2不成立sink、49模型とfixtureの参照比較。
* S2: k20 の既定導出と大予算参照の byte、G3値域、G4 joint context、同じ比較集合。
* CLI: ASCII整数1〜1000と先頭ゼロ、符号/空値/Unicode/小数/overflow/重複/欠値の拒否。
  check/simulate/explainの受理、他6コマンドのinvalid拒否、knownとno_analysis、明示仮定、G5/G6/G7。
  ledger既存のタグ形に従い known は reason:null、not_applicable は value:null を含める。
  ADR の短い JSON 例はこれらの null 欄を省略している。

新規に unsupported へ閉じる DSL の組合せは無い。未受理コマンドの新フラグは unsupported でなく invalid。
verify の通常入口は外部backend未接続でunsupportedのままだが、新フラグを黙って無視してはならない。
旧入力の潜在値条件は E-POL-LATENT / policy cannot read latent state を保存する。
新しい DSL キーワードはない。agency_limit_scale、framing、retention、invalid_ir の旧識別子使用も parser/lower で保存する。

GREENの校正は一時production故障で実際に落としてから戻す。期待の緩和で成功にしない。
故障G1〜G9のうち、実装前に入口のないものの実注入は未実施と明示し、実装の GREEN 前の義務として残す。
新規受入の失敗理由と校正の観測は検証記録とログに残す。
