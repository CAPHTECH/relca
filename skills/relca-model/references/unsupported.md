# よくある unsupported / invalid とその理由

「観測」は補助資料の試行で 1 回実行したもの。「文書」は説明を確認したが回していないもの。
EC 試行は電子商取引の注文・決済を有限再送つきで記述した模型で、`../scripts/testdata/ec-plain/` には plain 案の `model.relca` と表示試験用の固定 JSON の抜粋がある。JSON の抜粋は完全な出力ではなく、比較用の全案のモデルもこのディレクトリには揃っていない。再送の別の実行例は `crates/relca-core/tests/support/response_loss_inputs.rs` と CLI 試験 `response_loss_retry.rs` を参照する。
どれも **合格に読み替えない**。直せるものはモデルを直し、直せないものは報告の「確かめられていないもの」に残す。

## 拒否に当たったとき

モデルを書き換える前に stderr の補足行、または JSON の `diagnostics[].detail` を読む。`kind` で次の手を決め、`blockers` で該当する文を確かめる。

| `kind` | どうするか |
|---|---|
| `rewrite` | `see` の節を読み、示された文を業務の事実に沿って直して、同じ検査を回す。たとえば `same_version` で `case.id` を版として渡していたら、実際に確かめて使う操作の結果に版の要求を掛ける。 |
| `analysis_not_connected` | 示された文を小さく直しても、その機能を分析できるようにはならない。機能や契約を外して通したことにしない。意味を保てるなら、[繰り返しを回ごとに書き下した例](../../relca-design/references/rework-unrolling.md)のように書き下して検査してよい。結果は書き下した版の判定として、上限回数と元の書き方で未対応だったことを併記する。意味を保って書き下せないか、書き下しても同じ拒否が出るなら、どの分析で何を判定できなかったかと原因の文を「確かめられていないもの」に残す。 |
| `property_undecided` | その性質だけが未判定。根拠源の診断なら示された候補の源、対象の呼出し、静的な系譜で数えなかった書き手、出口を報告する。候補を確定した源として扱わない。業務の事実とモデルが違う場合だけ直して、検査を回す。 |

繰り返しの中の値を返す if など、構文の段で最初の原因だけが報告される拒否では、`rewrite` はその原因についての区分であり、直して回し直すと別の拒否が出ることがある。

`detail` がない、または `kind` が未知なら、書き方で直ると決めつけない。元の診断と利用ガイドを照合し、分からない範囲を報告に残す。

## 診断別の例

| 症状 | code / exit | 理由 | どうするか | 根拠 |
|---|---|---|---|---|
| simulate が未解決の非決定に出会う | U-SEM-0、exit 4 | 同時刻に行動が複数残っても、順序を変えた結果と観測が一致すれば先へ進む。一致しない選択に確率を補わない | 方策や確率を置くのは業務上の根拠がある場合だけ。仮定一覧にも書く。根拠が無ければ unsupported のまま報告する | 観測: `simulate examples/invoice/as-is.relca` は時刻 0 の `AsIsFlow.entry` で拒否。`simulate examples/invoice/busyday.relca --scenario BusyDaySmoke --configuration ToBeP` は標本を完了し、`simulation.same_time` を記録。性質10件のうち9件が inconclusive で exit 2。異議の応答だけが除外の印を持ち、残る判定不能には標本の検査・推定の限界がある |
| compare --mode simulation が全案 unsupported | U-SEM-0、exit 4 | 各案の simulate と同じ | 同上。exhaustive の compare を使う | 観測 |
| mutate が元 satisfied でも exit 4 | run `completion: inconclusive`, `reason: mutant_incomplete` | 既定集合の一部の種(規則版の旧版書込み・分母除去など)が作れない | 生成された種の検出・未検出だけを報告し、作れなかった種を列挙する | 観測(as-is) |
| verify | exit 4 | 外部バックエンド未接続。stderr の `outside Phase 1` はこの対応範囲外を指す | check の結果で代用しない | 観測・[分析モード](../../../docs/guide/guarantees.md#分析モード) |
| `//` を書いた | U-SYN-0、exit 4 | コメントは字句で unsupported | 説明は文字列(`reason "…"`、`question "…"`)か別ファイルへ | 観測 |
| 文字列の外に日本語 | U-SYN-0 | 非 ASCII は文字列の中だけ | 名前は ASCII、説明は文字列に | lexer.rs |
| bounds を一部省いた | U-SEM-0「cases, live_tasks and clock must all be explicit」、exit 4 | 上限は全部明示 | `bounds { cases …; live_tasks …; clock … }` | 観測 |
| simulation 節が無いのに simulate | E-SIM-0、exit 3 | 標本数・seed・信頼水準が要る | `simulation replications N seed S confidence 0.95` | 観測 |
| configuration が複数のモデルを選択子なしで check/explain | E-CONF-0、exit 3 | 選択子が要る | check・explain ともに `--configuration NAME` | [結果を読む](../../../docs/guide/results.md#checksimulateexplain) |
| 方策・分岐の条件で潜在を読んだ | E-POL-LATENT、exit 3 | 観測できない真値で業務判断できない | 観測できる値(判定の結果)で分岐する | 観測(to-be-c) |
| 確率の性質が violated でも反例が無い | omitted `probabilistic_property` | 確率的違反は一本の経路で説明しない | 確率値と上限を並べて報告する | 観測(to-be-a) |
| Cut が無い | W-CUT-0、`cut: "undeclared"`、exit 0 のまま | exit は変わらないが範囲は不明 | 必ず Cut を書く。review_page は警告に出す | 観測 |
| 判定したのに「止める判定なし」 | W-RESP-2 | `case.id` 以外の引数も追加の読取りも無い呼出しは判定と数えない | 業務で実際にデータを受け取る操作なら、その引数を書く(`review(claim)`)。警告を消すためだけに引数を足さない。実態として止めていないなら警告のまま報告する | 観測・`fixtures/agency-refusal/` |
| 匿名実行者に職務分離 | unsupported | 実行者が分からないと分離を判定できない | `executors [...]` を書く | `fixtures/exec-anonymous.broken-no-executor/`、assumptions の `executors` 行 |
| 計算上の上限に達した | `truncated: true`、inconclusive(exit 2) | 業務上の上限ではなく計算の切断 | 上限を見直す。未完了案件を成功・失敗へ変換しない | [上限と未完了](../../../docs/guide/guarantees.md#上限と未完了) |
| 再送を含むモデルで compare / diff | U-SEM-0 `retry_feature_combination: compare(diff) with response receipts is unsupported`、exit 4 | Receipt unknown を比較・差分で扱えない組合せを明示的に拒否している。案の一つにでも再送があれば拒否 | 案ごとに `check --configuration` を回して人が並べ、「compare ではない」と明記する(SKILL.md「再送を含むモデル」)。全体の unsupported と各案の verdict を両方報告する | 観測(EC 試行)・[対応範囲](../../../docs/guide/guarantees.md#対応範囲を確認する) |
| 再送を含むモデルで `breakdown waits` | U-SEM-0 `retry_feature_combination: receipt/retry with contest, waiting breakdown or quantiles is unsupported`、exit 4 | 待ちの内訳と Receipt unknown の交差は未対応 | `breakdown waits` を外して回し、時間は終端の elapsed の範囲だけを書く。内訳は「確かめていない」 | 観測(EC 試行) |
| 再送を含むモデルで分位点・contest | 上と同じ文言 | 同上 | 分位点・contest を外すか、再送を外した別モデルにして、外したことを Cut と報告に書く | lower.rs の `retry_feature_combination` 診断。回していない |
| 重複拒否の機構も `effect_scope modeled` も無い Capability を再送 | U-SEM-0 `response_loss_resend_mechanism_unmodeled`、exit 4 | 機構をモデル化していない外部 commit の再送は判定しない | 取引先が重複を拒むなら G の形、拒まないなら `effect_scope modeled` と具体的な counter。業務の事実で選ぶ | 観測・`response_loss_inputs.rs` の `r-resend` |
| `idempotency key` を書いたが atomic の重複拒否の形でない | U-SEM-0 `idempotency_mechanism_unmodeled` | 宣言だけでは一回性を認めない | dsl.md の G の形に揃える | `response_loss_inputs.rs` の `idempotency-without-atomic`。ここでは回していない |
| `retry` の `exhausted end` を省いた | E-SYN-0「expected exhausted」、exit 3 | 最後の失敗先は必須 | `exhausted end <End>` を書く | 観測 |
| `if x == unknown` | E-TYPE-0「unknown is not a member of …」、exit 3 | unknown は戻り値の値ではない。結果不明の後で分岐できない | 分岐せず、結果不明の後の段をその名前で defer し、opaque による全体撤回と W-CUT-2 を報告する。無関係な操作の部分撤回で代用しない(SKILL.md「再送を含むモデル」) | 観測 |
| 最後の応答が unknown になりうる | `receipt_resolution` inconclusive、exit 2(他に違反があれば exit 1) | 未完了を成功・失敗へ変換しない。`truncated` は false のまま | 合格扱いにしない。論点表に「判定不能」で残す | 観測・[上限と未完了](../../../docs/guide/guarantees.md#上限と未完了) |
| 有限の `repeat` | 一律に unsupported ではない | `mean-repeat-policy` の `--configuration Loop` は check と simulate で平均 `15/2` 分を記録。mutate は組合せによって `unsupported / loop_control` または `unsupported / loop_feature_combination` の行を出す | 回数表と計算上の打切りを区別する。未対応の変異を未検出に数えない | 観測: [採取コマンド](../scripts/testdata/numerical/capture.py) |
| 無限のループ・公平性・部分観測方策・実行中の規則改定 | [U] | 組合せごとの拒否理由と出力は今回確認していない | [対応範囲](../../../docs/guide/guarantees.md#対応範囲を確認する) と実際の診断を確認する | 文書 [U] |
