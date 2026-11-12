# Phase 2 cleanup の独立手値

要求: REC-p2c の2〜4、6、8節と cleanup checkpoint U1〜U12。
実出力、snapshot、旧goldenは意味期待の入力にしていない。
以下は実行前に定めた有限入力の期待であり、実行観測とは区別する。
既存のfixture、16反復のoracle、snapshotは変更しない。

## Q7: A1 / A2

SIMPLE は開始0、所要2、終端done=2。確率問は1、下限0なので satisfied。
共通gate後の私有IR破損を一件だけ入れる。

| backend | failure | 子code | 子verdict | point.reason | driver |
|---|---|---|---|---|---|
| finite | Actor | U-SEM-0 | unsupported | sensitivity_feature_combination | 0 |
| finite | StochasticArrivals | U-SEM-0 | unsupported | sensitivity_feature_combination | 0 |
| finite | Diagnostics E-TYPE-0 | E-TYPE-0 | invalid | sensitivity_model_invalid | 0 |
| finite | Diagnostics U-SEM-0 | U-SEM-0 | unsupported | sensitivity_feature_combination | 0 |
| finite | NoVerify | U-SEM-0 | unsupported | sensitivity_feature_combination | 0 |
| simulation | MissingClause | E-SIM-0 | invalid | sensitivity_model_invalid | 0 |
| simulation | Diagnostics | 元code/span/message | 元分類 | 上と同じ | 0 |
| simulation | StochasticArrivals | U-SEM-0 | unsupported | sensitivity_feature_combination | 0 |
| simulation | WireRange | U-SEM-0 | unsupported | sensitivity_capacity_limit | 0 |
| simulation | Scope | E-CMP-0 | invalid | sensitivity_contract_mismatch | 0 |
| 両方 | Ok | なし | satisfied | null | 1 |

WireRange は cases=2、replications=2^64-1 とし、その積がu64に入らない。
巨大DSLや割当は不要。Scope は子confidence=1/2、suite=3/4の不一致。
DiagnosticsだけはIRの既存codeによる分類を引き継ぐ。他のvariantのreasonはcode文字列から推測しない。
任意messageの変更はreasonを変えない。閉じたenumの全腕は私有受入に列挙する。
未知variantを足した際にproductionの網羅matchがコンパイルを止めることは、実装後のコンパイル故障検査の義務として残す。
試験側の網羅matchだけが落ちてもproduction網羅性の証拠にはしない。

postbarrier の3点は最初まで全点受理。実driver呼出し後、2番目の子だけをnot_runの正規Reportへ差し替える。
呼出しは2、先行点はevaluated/ranのまま、当該点はrefused/not_runと既存feature拒否語、詳細にcontext_mismatch:とpreexecution barrier。
3番目は呼ばずnot_run/no_analysis/report=null。summary=null/incomplete_points、root非0。
本物のSemanticAbortはdriverがranなのでevaluated/point_resultと区別する。
この矛盾が正規DSLで自然発生するとは主張しない。

## Q9: B1 / B2

### 既知拒否と後続範囲停止

Judgeは0開始、1終了、target done=1。二つのclauseは起票率1。
Reply.reject は ready=false により時刻1で拒否、未開始、作業0。
Reply.break_range は時刻1開始、所要2、時刻3のfinishでn=1+1=2が0..1を超える。
反復は2、どちらもrange stop、clock cutは0。

| route | target | filed | start | response | closed | failed | 集約failed/unresolved |
|---|---|---|---|---|---|---|---|
| reject | 1 | true | null | null | 1 | true | 2 / 0 |
| break_range | 1 | true | 1 | null | null | false | 0 / 2 |

率は両行2/2=1。応答標本は無い。rejectの3時間はnull/no_samples、後者はnull/unfinished。
Coreのstate_in_range違反は残り、root violated/exit1。
もし通常DSLでこの管理順が拒否されるなら、それを意味期待の訂正に使わず私有履歴へ移す。

### 新しい2反復・2rowの原因帰属（既存16反復oracleは不変）

row A=Controls#1、row B=Controls#2。case=1。全target=1、起票=true、start=1。
rep1: range停止、AとBとも未応答。rep2: clock切断、Aだけresponse=3、Bは未応答。
Aはtarget=2/filed=2/responded=1/unresolved=1、率1を保ち3時間null/unfinished。
Bはresponded=0/unresolved=2、率1、3時間null/truncated。
Aの未知はrangeの反復だけにある。Bはrangeとclock両方を持つのでtruncated優先。
target版は未応答をtarget未知へ変える。Aはtarget_observed=1/unknown=1/filed=1、率null/unfinished。
Bはtarget_observed=0/unknown=2/filed=0、率null/truncated。
known版はAが両反復でresponse=3。Aはservice=(2+2)/2=2、queue=0、elapsed=2、samples=2。
Bのclock切断でAの既知cohortを撤回しない。反復順を反転し、rep番号とrange帰属を同時に反転しても同じ。
この手組み停止原因の認証はしていない。記録された反復原因との自己整合を試す。

### NotConfiguredの片故障

trials=2。既知行はobserved=2=filed、responded=2。
target_unknown: observed=1、unknown=1、filed=responded=1。
choice_unknown: observed=2、filed=responded=1、choice_unknown=1。
unresolved: observed=filed=2、responded=1、unresolved=1。
各行はtarget分割、選択分割、結末分割を満たす。負対はreasonだけをNCへ変える。
filing_unknownの率NCとresponse_unknownの時間NCは拒否。
起票が既知のunresolved行では、率だけをNCへすることは合法、時間unfinishedは維持。
全既知の率/時間NC、D=0/no_samples、aborted/null件数は正対。
apply_deferredは元reasonがnull（又は既にNC）の量だけを保留するため、D=0の率no_samplesや応答標本0の時間no_samplesをNCへ変えたwireも拒否する。
応答0の対はtarget=filed=route_failed=2の既知失敗行も使い、分母0だけに依存しない。
3時間のreasonは相互一致が必須なので、時間reasonの1論理欄を変える負対では3箇所を同時変更する。

### 管理順の狭い受入

Management の完了正対は2案件の到着[0,4]、Judge1、Reply2、別人。
case1: target/start=1、response/closed=3。case2: target/start=5、response/closed=7。
queue=0、service=elapsed=2。
各反復は起票draw2回だけ。管理再安定化でdrawを増やさない。2反復完了、cut無し。
Actor名の一括改名で時刻/件数は不変。
到着[0,2]の対は時刻3にcase1のroute終端とcase2の業務終端が重なり、二つの別finish actionが残る。
この未解決非決定を勝手に順序づけずunsupported、完了反復0、切断件数nullとする。
最初の入力案ではこの非決定を見落とした。公開シミュレーションのfail-closed規則に照らして入力/手値を訂正したもので、成功実行を要求する変更ではない。
旧rates無しの遭遇が他案件live boundに先行する対も既存入力から明示的に再利用する。
この狭い交差を一般入力同値の証明としない。

## Q6: C1 / C2 / E

DEGRADE のprefixはProbe.checkを必ず1回。prefix_twoは必ず2回。
sideのClerk3/Fast1とその相対的構造は同じ。差分はManual/common_prefixのCount配列1行のみ。
from=[Probe.check:1..1,Clerk.run:0..0,Fast.run:0..0]、to=[Probe.check:2..2,Clerk.run:0..0,Fast.run:0..0]。左右反転で値だけ反転。
Count配列は未使用Capabilityも0で保持する既存表の契約に従う。
旧Lはwhole_process_pathsのまま、normalのProbeは1..1から2..2、Clerk/Fastは両模型とも0..1。side別の依存を旧Lへ混ぜない。
既知同値は0行。table欠落はnormal/degraded/common_prefixの計3行のnot_comparable。
prefixの既知側はderived/null、欠落側はowner.agency.degradation:null。
route名だけ改名すると、それぞれ反対側のrouteが不在なのでprefixは2行not_comparable。
その理由はowner.agency.degradation.route.common_prefix:null。
Countのmin>max、余剰key、Side objectをCount配列の代わりに置いたpayloadは拒否。

cases=2の全表撤回はmultiple_case_interferenceでroutes=null。
片側が既知ならcall=Manual、両側撤回ならcallキー無し。
normal/degraded/prefixの3行ともnot_comparable、理由はfrom/to.agency.degradation:multiple_case_interference。
これを差なしにしない。元のside payloadやwhole_process_pathsの範囲は変更しない。

局所rulesはagency-diff-p2-1、全ledger ownerのbundleはrelca-conclusions-p2-uncertainty-2。
新wireのroundtrip、旧/未知版、rulesだけ旧、componentだけ旧を拒否する。
旧build byte互換を復活させない。全snapshot版差監査は統合Fの担当でありこの受入実行ではない。

missing performance recordは正規のq6_differences Resultを観測し、invalid_q6_record:から始まる非空Err。
両正常表は空差分。片側/両側table欠落を成功や空差分へ丸めない。
diff/compareのU-SEM-0 adapter接続の範囲には別の実行証拠が必要である。

qualificationは厳密昇順の非空名集合。[p,q]、[p]、[]、nullの正対を保持。
[q,p]、[p,p]、[空文字]はどちら側でも拒否。
performanceは未宣言ならキー省略。明示nullの受理・正規化を禁止する。
成績objectと通常kernelは排他、row.kernel必須、未知key拒否。
正対はdecode/encodeでpayload全体が変わらないことを要求する。

## 文書との食い違いと範囲

Q9時間の適用はコード/既存受入のrow単位を採る。ADR0063の全run撤回と読める文言は後続Dで訂正する。
Q9 referenceのhookはEndだけでなく各案件管理末尾。狭い受入は現管理順を保存する。
局所rulesとbundleはコードの旧世代からU8で変更する。ADR0058の旧bundle名へ戻さない。
README/設計写し/その他ADRの網羅整合確認や修正は今回の受入作成の対象外。

## cleanup 補足2: 拒否時の台帳整合

正常なS1完全到着列（確率1で[0]）をcheckへ渡すと、有限backendは未対応でU-SEM-0を1件だけ返す。
根拠の無いU-LEDGER-0を保存しない。この修復はA1に含むとorchが裁定した。
公開core/CLI正対と、私有finite StochasticArrivals腕で同じ診断件数/codeを固定する。
同型のsimulation Diagnostics/StochasticArrivals/WireRangeも、外側simulation_settingsがloweredを記録した後の整合を固定する。
ここでのsimulation試験はrawだけでなく、実際のscope wrapperとsimulation_driverによるsealまで含む。
MissingClauseはspec無しでsettingsを記録しない、Scopeは既にboundsを設定する、Okは実行済みを保持する。
拒否時のqualityは宣言がある限りabortedのnull、空標本no_samplesとは区別する。
有限Okは私有analyze_modelのraw driver境界なので実行者ledgerはunavailable（任意closureをbuiltinと偽らない）。
finite拒否はfinish経由でsealされる。全点runner側は別に完全な子ledger/roundtripを検査する。
ledger.loweredとsimulation_settingsの全呼出しの読取census、経路別の修正要否、旧snapshotの件数はこの手計算だけでは確認できない。
