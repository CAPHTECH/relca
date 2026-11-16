# defer の解決と撤回の表示: 独立した手計算

ADR 0077 の採択 A1/L1/B2/V1/C1/E1 の受入。新しい欄の期待値は binary から採らない。
入力と期待は `support/defer_effects_inputs.rs`、照合は `defer_effects.rs` に固定する。
既存 fixture・snapshot・計算書は変更しない。

## 先に読んだ出力の形

`report.rs` の DeclaredCut/CutEntry、`diag.rs` の Diagnostic/Span、`ir.rs` の
deferred_metrics、`tests/support/deferred_wire.rs`、既存 deferred_metrics 試験を読んだ。
Cut は deny_unknown_fields。診断の span は UTF-8 の半開 byte 区間で、stderr は
code と message、span は JSON に載る。数値は文字列、撤回 scalar は min/max null と
reason not_configured（simulation は mean null、samples 0 も持つ）。
quality は exhaustive の lower/upper/basis、simulation の point を撤回する。
compare は plans[i].report、diff は from.report/to.report、mutate は original を運ぶ。
mutant.report と sensitivity.points[i].report も Report 型を持つので C1 の対象。
実行前にここまでの形と以下の手値を決めた。

## K0〜K7

A.x は p が 3 分、R.y は q が Desk を 2 分使う。R.z は 4 分だが呼ばない。
R は z, y の順に宣言し、派生欄の capabilities は名前順 y, z を要求する。
直列では時刻 0→3→5、work=3+2=5、費用=3×10+2×20+1+2=73。
R.y を呼ばないと時刻 0→3、work=3、費用=3×10+1=31。
終端 done は確実に1回、arrivals=1、cohort=1/1=1、P95=5 または3。

| ID | 解決 D | 行の撤回 | 根拠 | process | 警告 |
|---|---|---|---|---|---|
| K0 | defer 無し | 欄無し | - | - | 無し |
| K1 | Ghost: opaque | process | opaque | true | Ghost 1行 |
| K2 | A.nosuch: opaque | process | opaque | true | Actor A の補足付き1行 |
| K3 | R.y: {R.y} | process | call R.y | true | 無し |
| K4 | R.y: {R.y} | scoped | 本文に無い | false | 無し |
| K5 | R: {R.y,R.z} | process | call R.y | true | 無し |
| K6 | Ghost、metrics/quality 無し | 欄無し | 対象外 | - | 無し |
| K7 | Ghost と R.y | process と scoped | opaque と本文に無い | true | Ghost だけ1行 |

K4 と K7 の R.y の scope は actors=[R], capabilities=[R.y], people=[q],
resources=[Desk]。metrics に R/R.y の行が無くてもこの scope を示し、行は新設しない。
K7 の全体 true を R.y に配らない。K1/K2/K3/K5/K7 はすべて値を撤回する。
reason は not_configured のまま、quality の確率と P95 も null。
K4 は A と全体と品質の数値を保存し、q/Desk だけを撤回する。
空 Actor は D={}、withdraws=none、because=[]、scope=null。重複 defer は同名でも
別 entry と別 warning/span を保つ。死枝の R.y も call 交差に含める。

## 請求書 P0/Pa/Pb/Pc/Pd を再計算

`examples/invoice/phase1.relca` の kernel を分数で掛け直した。
人の誤登録 routine は (4/5)(1/100)(1/5)(3/20)(1/10)=3/125000、
difficult は (1/5)(3/50)(1/2)(2/5)(3/10)=90/125000。
合計 93/125000。AI は同様に誤抽出だけ 1/50 と 3/20 に替え、
6/125000+225/125000=231/125000。
人の正抽出は 4/5×99/100+1/5×47/50=49/50、AI は477/500。
したがって posted は人 122593/125000、AI 119481/125000。
どちらも 19/20 より大きく、posted 時刻は各構成で固定なので、全流入 P95 は
AsIs=6+3+5+2+1=17、ToBeP=6+max(3,5)+2+1=14、
ToBeA=1+max(3,5)+2+1=9。条件付き reached P95 も同じ。
人の作業は二照合完了まで 14/14/8、承認があれば +2。
returned 後だけ contest の recheck +1 がありうる（起票無しも可能）。
承認 fail 後の異議で最大17/17/11、二照合 fail の起票無しで最小14/14/8。
Ledger と Extractor は匿名 capacity なので named_people_work に足さない。
単価は全て未設定。defer を外しても費用を0にしない。

| 変種 | 解決 | process | 数値 |
|---|---|---|---|
| P0 | Resubmission と Vendor.contest_rate は opaque | 全構成 true | 全撤回 |
| Pa | defer 無し | 欄無し | 上記の分数・P95・人作業範囲 |
| Pb | Resubmission だけ opaque | 全構成 true | 全撤回 |
| Pc | Clerk.recheck に完全一致 | 全構成 true | contest_route 交差、全撤回 |
| Pd | Extractor.extract に完全一致 | AsIs/P false、A true | AsIs/P は Pa、A は全撤回 |

Pc は本文に call が無くても returned が存在するため異議 route が active。
Pd の非交差 scope は actors=[Extractor], capabilities=[Extractor.extract],
people=[], resources=[]（executors と uses が無い）。P0 は warning 2行、Pb は1行、
Pa/Pc/Pd は0行。compare は各構成の prefix で6行、diff は from/to 各2行。
P0 の compare と diff の exit は1。ToBeA の誤登録 231/125000 は上限
125/125000 を超え、compare の inconclusive な品質 hard より既存の violated が優先する
（compare.rs の fold は各案の verdict と hard を合わせる）。警告で exit を変えない。
草案の手値との食い違いは無かった。

## 比較・wire・校正の境界

Included は R.y を呼び、Unrelated は呼ばない。capability defer なら process は true/false、
opaque defer なら true/true。前者の品質 hard は inconclusive/satisfied、後者は両方
inconclusive。数値を失った hard/objective に0や retained trials の推定を戻さない。
全体 process と evaluation の deferred の不一致は数値を戻した比較セルで検出する。
W-CUT-2 は warning の語彙、対象の byte span、選択 Cut 限定、metrics-only と
quality-only の双方を検査する。conform は両 scope で対象外。

故障対は (1) K3 に誤警告、(2) K7 の R.y を process、(3) compare の数値復活、
(4) reason を deferred、(5) 選外 Cut に誤警告。手作りの正常値から1項目だけ変える。
新しい wire 欄は DeclaredCut の往復と共有 validate_deferred_metrics_wire で検査する。
後者に意味照合を求める負対は live baseline の値を使うが、新しい欄の期待そのものは
上の固定表だけから置く。型の厳密さと意味の厳密さを混同しない。

旧入力の byte 保存だけは、既存の `fixture-met-serial`（defer 無し）と
`fixture-cut-declared`（opaque defer があるが metrics/quality 無し）の保存済み
stdout/stderr/exit をそのまま読む。これは新しい欄の意味期待でも、今回の binary からの
採取でもない。上記の手計算照合と別に、既存 golden 2件を変更しないことを検査する。
