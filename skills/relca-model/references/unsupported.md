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

## 理由語から対処を選ぶ

[診断の理由語](../../../docs/guide/results.md#診断の理由語)で code と exit を照合する。書き方を変えると業務の意味も変わるなら、自分で直さず人に確認を求める。

| 理由語 | モデルを作るときの対処 |
|---|---|
| concurrent_read_write、concurrent_writers | 読み書きの順序や案件ごとの分離を人に聞く。実際の順序・分離を表す場合だけ直す。 |
| result_not_definitely_available | 利用先を結果が得られた経路に移せる場合だけ直す。 |
| executor_identity_unavailable | 実行者と権限を人に聞く。匿名のまま職務分離を主張しない。 |
| unsupported_scenario_item | 同じ条件を対応する指定に書き換える。条件を外す必要があるなら人に上げる。 |
| unsupported_invariant_expression | 同じ不変条件を対応する式で表せる場合だけ書き直す。 |
| unsupported_standing_target | `.start` のような未対応の指定は、原因箇所が `start` を指す。実際の対象を人に聞き、通るように別の対象に差し替えない。 |
| unsupported_set_element_type | 原因箇所が `optional` なら、問題は要素の型ではなく集合の外側にある。要素の型と、値なしを区別する必要を人に聞く。 |
| loop_feature_combination | 意味を保てる場合だけ回ごとに書き下す。元の組合せの未対応を報告する。 |
| merge_feature_combination | 同じ値を渡せる場合だけ書き換える。別の分析を比較やログ処理の代わりにしない。 |
| registration_result_dependent | 実際に登録する経路を確認し、登録の分岐を明示する。 |
| registration_feature_combination | 記録の範囲と読取りを人に確認する。必要な登録を消さない。 |
| same_version_feature_combination | rewrite なら実際の版の結果を参照する。未対応の組合せは人に上げる。 |
| evidence_basis_feature_combination | 根拠源の要求を残し、確かめられない経路を人に上げる。 |
| evidence_basis_conditional_snapshot | 候補源、書き手、操作、出口を人に示す。業務上の事実との違いだけを直す。 |
| simulation_unresolved_nondeterminism | `priority_tie_remaining` なら、優先順位が同じ候補をどう扱うかを人に聞く。それ以外の `unresolved_choices` では、方策や確率の根拠を人に聞く。根拠が無ければ保留し、必要なら網羅探索を別に行う。 |
| stochastic_arrivals_mode_unsupported | 分布を固定時刻で代用しない。標本推定で足りるか人に聞く。 |
| stochastic_arrivals_feature_combination | 残すべき機能を人に確かめ、同時に分析できなかった範囲を報告する。 |
| sensitivity_schema_unsupported | 指定の形式や対象を直す。別の問いになるなら先に人に聞く。 |
| sensitivity_feature_combination | cause を人に示す。全点で同じ問いを保てなければ推定しない。 |
| condition_wait_diff_unsupported | 案を個別に検査できても差分検査の代わりとしない。 |
| condition_wait_contest_unsupported、contest_rates_effect_target | 待ちや異議の業務要件を人に聞く。異議の対象を勝手に変えない。 |
| priority_resolution_scope_unsupported | 実際の共有状態と所有者を確かめる。対象を変えるなら事実に合わせる。 |
| priority_feature_combination | 案で揃えるべき条件を確認する。verify では外部検証が未実施と伝える。 |
| retry_feature_combination、recovery_feature_combination | cause を読み、未対応の組合せだけを報告する。応答不明を成功や失敗にしない。 |
| policy_feature_combination | 方策を外すと問いが変わる。必要な順序と経路を人に示す。 |
| mean_feature_combination | 期待値は保留する。別の指標を出して同じ量とは呼ばない。 |
| scope、schema、time_unit、approval_spec_schema | 指定・記録形式・単位を採取元と照合する。変換や再採取の根拠が揃うまで待つ。 |
| unsupported_q1_model_feature、unsupported_contract、unsupported_verify | 未検査の機能・契約・検査を列挙し、人に上げる。 |
| unsupported_cancel_semantics、unsupported_capability_lifecycle | 照合できない取消や操作属性を報告する。検査を通すために記録を加工しない。 |
| unsupported_approval_profiles、unsupported_approval_semantics | 照合の定義や操作・戻り値を人に確認する。モデルの合格に置き換えない。 |

異議経路や分位点を外した別モデルを検査するなら、外したものと失う検査を Cut に宣言し、報告にも書く。元のモデルを検査したとは書かない。

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
| 重複拒否の機構も `effect_scope modeled` も無い Capability を再送 | U-SEM-0 `response_loss_resend_mechanism_unmodeled`、exit 4 | 機構をモデル化していない外部 commit の再送は判定しない | 取引先が重複を拒むなら G の形、拒まないなら `effect_scope modeled` と具体的な counter。業務の事実で選ぶ | 観測・`response_loss_inputs.rs` の `r-resend` |
| `idempotency key` を書いたが atomic の重複拒否の形でない | U-SEM-0 `idempotency_mechanism_unmodeled` | 宣言だけでは一回性を認めない | dsl.md の G の形に揃える | `response_loss_inputs.rs` の `idempotency-without-atomic`。ここでは回していない |
| `retry` の `exhausted end` を省いた | E-SYN-0「expected exhausted」、exit 3 | 最後の失敗先は必須 | `exhausted end <End>` を書く | 観測 |
| `if x == unknown` | E-TYPE-0「unknown is not a member of …」、exit 3 | unknown は戻り値の値ではない。結果不明の後で分岐できない | 分岐せず、結果不明の後の段をその名前で defer し、opaque による全体撤回と W-CUT-2 を報告する。無関係な操作の部分撤回で代用しない(SKILL.md「再送を含むモデル」) | 観測 |
| 復旧で解決していない応答不明の経路が残る | receipt_resolution が inconclusive。終了コードは最上位の verdict に従う | 未完了を成功・失敗へ変換しない。計算切断とは別 | 合格扱いにしない。論点表に「判定不能」で残す | [上限と未完了](../../../docs/guide/guarantees.md#上限と未完了) |
| 有限の `repeat` | 一律に unsupported ではない | `mean-repeat-policy` の `--configuration Loop` は check と simulate で平均 `15/2` 分を記録。mutate は組合せによって `unsupported / loop_control` または `unsupported / loop_feature_combination` の行を出す | 回数表と計算上の打切りを区別する。未対応の変異を未検出に数えない | 観測: [採取コマンド](../scripts/testdata/numerical/capture.py) |
| 無限のループ・公平性・部分観測方策・実行中の規則改定 | [U] | 組合せごとの拒否理由と出力は今回確認していない | [対応範囲](../../../docs/guide/guarantees.md#対応範囲を確認する) と実際の診断を確認する | 文書 [U] |
