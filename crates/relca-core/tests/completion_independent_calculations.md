# U3: 小母集団の独立探索と手計算

`completion_independent.rs` と CLI の同名 target の期待値。snapshot、製品の
engine/reference/completion/report/agency は期待の算出に使わない。
DSL と手書き OwnModel を `support/completion_explorer_population.rs` に対で置く。
型 V の pass/fail、全 call/binding、Actor 枠、実行者、資源、制御木、終端、
時間と結果、宣言・if・end・people の byte span を lower の結果と別に照合する。
span の期待は DSL 中の一意の文字列から取り、IR の span を期待へ転記しない。

## 手で追える19模型

単位は分。Start は Capability 集合、arms は then/else の集合、person は
どこかの経路で割り当てられる人の集合。空欄の代わりに ∅ と書く。
匿名枠は person に入れない。各終端の括弧は (elapsed, work)。

| 模型 | 到達 end と (時間, 作業量) | Start | arms | person | deadlock / cut |
|---|---|---|---|---|---|
| both | good(1,1), bad(1,1) | A.x | then,else | p | false / false |
| executor-pool | good(1,1), bad(1,1) | A.x | then,else | p,q | false / false |
| anonymous-one | good(1,1), bad(1,1) | A.x | then,else | ∅ | false / false |
| pass-only | good(1,1) | A.x | then | p | false / false |
| capacity-zero | ∅ | ∅ | ∅ | ∅ | true / false |
| resource-one | good(1,1), bad(1,1) | A.x | then,else | p | false / false |
| resource-zero | ∅ | ∅ | ∅ | ∅ | true / false |
| clock-zero | ∅ | A.x | ∅ | p | false / true |
| clock-boundary | good(1,1), bad(1,1) | A.x | then,else | p | false / false |
| serial | done(8,8) | A.x,A.y | ∅ | ∅ | false / false |
| parallel-two | done(5,8) | A.x,A.y | ∅ | ∅ | false / false |
| parallel-one | done(8,8) | A.x,A.y | ∅ | ∅ | false / false |
| parallel-resource | done(8,8) | A.x,A.y | ∅ | ∅ | false / false |
| serial-wait | done(10,8) | A.x,A.y | ∅ | ∅ | false / false |
| shared-person | done(8,8) | A.x,B.y | ∅ | p | false / false |
| distinct-people | done(5,8) | A.x,B.y | ∅ | p,q | false / false |
| arm-call | good(1,1), bad(4,4) | A.x,B.y | then,else | p | false / false |
| arm-call-removed | good(1,1), bad(1,1) | A.x | then,else | p | false / false |
| mixed-deadlock | good(1,1) | A.x | then,else | p | true / false |

導出:

- 1分の x は終了時にだけ pass/fail を公開し、その値で if を選ぶ。
  pass-only なら fail 後継と else/bad は無い。
- 3分と5分の逐次は 3+5=8、容量2の fork all は max(3,5)=5。
  どちらも work は 3+5=8。容量1または R の枠1で競合すれば順序がどちらでも8。
- Actor が別でも同じ p は重ねて割り当てられず8。p/q なら5。
  単なる Actor 名の相違を人の独立性とみなさない。
- wait 2 は elapsed にだけ足すので serial-wait は10/8。
- clock 0 でも時刻0の Start はある。次の finish=1 は切断であり、
  deadlock/成功/失敗に置き換えない。clock 1 は finish を含む。
- capacity-zero は匿名枠0。明示容量と executors の人数が異なる宣言は
  現 lower が E-CAP-0 にするため、合法な容量0と人の競合を別の模型で扱う。
- arm-call の fail は1+3=4。call 削除後は1。mixed-deadlock は B の枠0で
  fail の先が停止し、good の witness は残るが回復・全経路終了・確率1終了は偽。

## 完了と診断

全模型の字句上の end を reachable で問う。回復・全経路終了・確率1終了は、
このループも確率もない有限 DAG では、deadlock があれば violated、
deadlock がなく cut があれば inconclusive、両方なければ satisfied。
reachable は witness があれば satisfied、なければ cut のとき inconclusive、
完全探索なら violated。したがって pass-only は bad の到達だけが violated。
clock-zero は全性質 inconclusive。mixed-deadlock は good の到達だけ satisfied。

警告は字句在庫から上表の観測集合を引く。

- both: W-UNREACH-0 A.unused/not_called と W-UNREACH-3 q/never_assigned。
- executor-pool: q も選べるので A.unused だけ。
- pass-only: both に W-UNREACH-1 else/never_taken と
  W-UNREACH-2 bad/never_reached を足す。
- capacity-zero: A.x/never_started、A.unused/not_called、then/else、good/bad、q の7件。
- resource-zero: 上記に p/never_assigned を足した8件。
- clock-zero: 4警告はいずれも0、unchecked unreachable を1件。
- serial/parallel 系列と anonymous-one: Spare の q は未割当。
  process で使わない people p は pool 外なので警告対象ではない。
- shared-person は q が未割当、distinct-people は4警告なし。
- arm-call は4警告なし。arm-call-removed は B.y/not_called、
  mixed-deadlock は B.y/never_started と bad/never_reached。
  後者2模型でも else は取られる。

診断の code/target/reason/span を完全な message と位置へ照合する。
集合の比較でも重複行は許さない。各終端の時間/作業量はこの19模型では
単点集合なので、公開 Outcome の min=max と正確な集合を比較できる。
範囲の両端から中間の架空の終端を作らない。

## 独立性と非対象

OwnState は独自の制御列、結果、待機、Task、時計、work を持ち、
開始/終了/条件/時間前進を自前で列挙する。work-conserving であり、
意図的なアイドルや一様乱数は足さない。自分の状態重複除去だけを行う。
製品の非公開 State 集合との同型比較ではない。

`completion_explorer_adapter` は IR のデータを読むだけで、対象外を
NotCovered にする。cases1、容量0..2、正の固定時間1..8、2値以下の有限enum、
結果の条件、Call/If/End/Wait、3枝以下の fork all、独立資源1つ(容量0..2、uses1)、
live_tasks1..3、clock0..20、binding8個以下がこのレーンの器である。
全経路に明示 end を要求し、live-task切断を起こす fork は対象外。
現行19模型は最大2 Taskで、条件の And/Or や3枝の全直積を検証したとはしない。
join any/取消/requires/方策/多案件/kernel/atomic/contest/quality 等は対象外で、
製品 engine への fallback はない。対象外対照は独立検証19件に数えない。

製品側の Start/person/arm は公開 Outcome に無いので、別ファイルの
`completion_explorer_product` が既存のイベント監視と吸収endへの置換で観測する。
これは actual 側だけで engine を使う。期待側の OwnModel/探索/警告/完了判定は
この probe を呼ばない。end・時刻/work・deadlock/cut は直接 Outcome と照合し、
実CLIのraw JSON、完了性質在庫、root verdict と exit も別に照合する。

負対照は fail 後継欠落、余分な終端、finish の1tickずれを同じ comparator へ通す。
警告4種のcode/message/span改竄と完了性質の欠落/追加/合否改竄も拒む。
製品への5種類の一時的故障校正は作業結果に別表で記録する。
