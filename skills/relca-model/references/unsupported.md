# よくある unsupported / invalid とその理由

「観測」は補助資料の試行で 1 回実行したもの。「文書」は説明を確認したが回していないもの。
EC 試行は電子商取引の注文・決済を有限再送つきで記述した模型で、`../scripts/testdata/ec-plain/` には plain 案の `model.relca` と表示試験用の固定 JSON の抜粋がある。JSON の抜粋は完全な出力ではなく、比較用の全案のモデルもこのディレクトリには揃っていない。再送の別の実行例は `crates/relca-core/tests/support/response_loss_inputs.rs` と CLI 試験 `response_loss_retry.rs` を参照する。
どれも **合格に読み替えない**。直せるものはモデルを直し、直せないものは報告の「確かめられていないもの」に残す。

| 症状 | code / exit | 理由 | どうするか | 根拠 |
|---|---|---|---|---|
| simulate がすぐ拒否する | U-SEM-0、exit 4 | `result nondet` や、誰が先に始めるか等の未解決の非決定に出会った。simulate は非決定を一様乱数で埋めない | 確率は業務の根拠があるときだけ `outcomes kernel` に書く。方策(`policy …`)で選択を固定するのは、その固定が業務の実際のやり方だという根拠があり、かつ固定したことを仮定一覧(`--assumptions` と報告)に書く場合に限る。**simulate を通すために方策や確率を置かない**(非決定性を埋めない)。根拠が無ければ unsupported のまま報告する | 観測(as-is・最小例。as-is は時刻 0 の `AsIsFlow.entry`) |
| compare --mode simulation が全案 unsupported | U-SEM-0、exit 4 | 各案の simulate と同じ | 同上。exhaustive の compare を使う | 観測 |
| mutate が元 satisfied でも exit 4 | run `completion: inconclusive`, `reason: mutant_incomplete` | 既定集合の一部の種(規則版の旧版書込み・分母除去など)が作れない | 生成された種の検出・未検出だけを報告し、作れなかった種を列挙する | 観測(as-is) |
| verify | exit 4 | 外部バックエンド未接続。stderr の `outside Phase 1` はこの対応範囲外を指す | check の結果で代用しない | 観測・[分析モード](../../../docs/guide/guarantees.md#分析モード) |
| `//` を書いた | U-SYN-0、exit 4 | コメントは字句で unsupported | 説明は文字列(`reason "…"`、`question "…"`)か別ファイルへ | 観測 |
| 文字列の外に日本語 | U-SYN-0 | 非 ASCII は文字列の中だけ | 名前は ASCII、説明は文字列に | lexer.rs |
| bounds を一部省いた | U-SEM-0「cases, live_tasks and clock must all be explicit」、exit 4 | 上限は全部明示 | `bounds { cases …; live_tasks …; clock … }` | 観測 |
| simulation 節が無いのに simulate | E-SIM-0、exit 3 | 標本数・seed・信頼水準が要る | `simulation replications N seed S confidence 0.95` | 観測 |
| configuration が複数のモデルを check/explain | E-CONF-0、exit 3 | 選択子が要る | check は `--configuration NAME`。explain は旗が無いので単一 process のモデルを別に作る | 観測 |
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
| `if x == unknown` | E-TYPE-0「unknown is not a member of …」、exit 3 | unknown は戻り値の値ではない。結果不明の後で分岐できない | 分岐せず、結果不明の後の段は Cut の defer に置く(SKILL.md「再送を含むモデル」) | 観測 |
| 最後の応答が unknown になりうる | `receipt_resolution` inconclusive、exit 2(他に違反があれば exit 1) | 未完了を成功・失敗へ変換しない。`truncated` は false のまま | 合格扱いにしない。論点表に「判定不能」で残す | 観測・[上限と未完了](../../../docs/guide/guarantees.md#上限と未完了) |
| ループ・公平性・部分観測方策・実行中の規則改定 | unsupported | 範囲外 | 範囲外として報告 | [対応範囲](../../../docs/guide/guarantees.md#対応範囲を確認する) |
