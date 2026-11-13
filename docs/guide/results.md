# 結果の読み方

CLI は標準出力に JSON を出し、診断は標準エラーにも書きます。
機械的な成否は最上位 verdict に対応する exit で返します。

| verdict | exit | 意味 |
|---|---:|---|
| `satisfied` | 0 | 対象の性質が、今回のモデルと分析条件で成立 |
| `violated` | 1 | 少なくとも一つの検査で違反 |
| `inconclusive` | 2 | 切断などにより結論を出せない |
| `invalid` | 3 | 入力の不整合や静的エラー |
| `unsupported` | 4 | 必要な機能や組合せが対応範囲外 |
| `vacuous` | 5 | 性質の前件が発生せず、検査として働いていない |

## check、simulate、explain

最初に `mode` と `verdict` を確認し、次に以下を読みます。
explain は check と同じ最上位の欄に `explanation` を加えます。反例はその欄にあります。
explain と mutate も `--configuration NAME` と `--scenario NAME` で check と同じ対象を選べます。
構成の指定が必要なモデルで省略した場合や、未知の構成名は invalid です。
mutate の元の検査と変異体、explain の反例と再生は同じ選択を使います。
引数の誤りなどで模型を読む前に止まった report は、hash が null、Cut は `unresolved` です。
この場合は未宣言とは判断できないため W-CUT-0 を出しません。
読んだ模型の Cut が未宣言なら、従来どおり警告します。
欄は宣言や分析モードによって省略されることがあります。欄の省略だけから、対応済み・検査済みとは判断しないでください。

| 欄 | 確認する内容 |
|---|---|
| `model.sha256` | 分析した入力の識別子 |
| `cut`, `cut_context` | 宣言した範囲と、除外・保留・未宣言事項 |
| `bounds` | 案件数、同時 Task 数、clock、`truncated` |
| `properties[]` | 各性質の `verdict`、確率境界、根拠や注記 |
| `terminals[]` | 到達先別の時間・work の範囲。平均とは限らない |
| `diagnostics[]` | code、severity、message、入力内のバイト位置 span |
| `assumptions[]` | 既定値や分析で置いた前提 |
| `actors[]` | 宣言値と省略時に補った欄 |
| `metrics`, `quality` | 要求した量と、算出できない理由 |
| `ledger`, `uncertainty` | 入力選択、結論の根拠、未検査事項の記録 |
| `unchecked` | 今回は検査していない項目 |

最上位が violated でも、別の性質は vacuous や inconclusive のままの場合があります。
null は 0 ではありません。`not_configured`、`undetermined`、`truncated` などの理由を
値と一緒に扱ってください。`per_case_relaxation` の区間を一つの方策で実現可能な区間と読まないでください。
simulate の `basis` は性質や中断の有無で異なり、`properties[].cases[]` の `basis: statistical` は網羅探索の確率ではなく標本推定を表します。

## 構造の出力

JSON の `agency` はモデルの呼出し、読取り、条件、commit から作ります。
O/F/P/J/U/E/L は次のような集合・件数・状態を読む記号として使います。
鍵名は製品 API であり、入力に同名の自己評価欄を追加するものではありません。

| 記号 | 主な欄 | 操作的に読む内容 |
|---|---|---|
| O | `origin`, `viewpoints.observation` | source までの直接・間接経路、Actor と実行者の集合 |
| F | `viewpoints.framing` | 終端への到達条件を変える call の列と対象集合 |
| P | `viewpoints.proposal` | 現在は `not_derived` とその理由 |
| J | `lineage` | 入力や reads を持つ判定 call と、その祖先となる根拠源 |
| U | `refusal` | commit を止める条件と迂回文脈、`all_paths` / `conditional` / `none` |
| E | `viewpoints.effects` | commit の位置、実行可能な人、Permission や Authority の記録 |
| L | `viewpoints.retention` | 通常経路での呼出し回数の min/max、contest route や intent の有無 |

`not_derived` は「存在しない」ではありません。`rules`、`scope`、`unchecked` を併読します。
回数や経路の有無から、実際の人物の能力や業務上の正当性を判定できません。

## 複数案とその他の出力

- compare は `plans[].report`、diff は `from.report` と `to.report` を持ちます。
  最上位だけでなく各案の判定も報告してください。未対応の組合せでは、
  案の report が violated でも比較自体は unsupported になります。
- compare の表、hard の採否、目的の比較を区別します。目的の良さで安全性違反を相殺しません。
  diff の `differences` は構造差分で、未計算の種類の不在を「変化なし」とは読めません。
- mutate は `original` と生成した変更の結果を分けます。`run` が未完了のものや未対応種は
  検出・未検出の数に混ぜません。件数から検出率や網羅率を主張できません。
- 応答喪失の `response_receipts` は到達可能な観測の集合です。実行順や発生率の一覧ではありません。
  `case_outcomes` の `receipt_unknown` と `receipt_resolution` の保留を、成功や失敗に置き換えません。
- conform は観測ログの範囲の照合、sensitivity は manifest の有限点の分析です。
  子の結論、理由、対象を最上位の表示と一緒に読みます。

[保証の範囲](guarantees.md) と [手計算](../../fixtures/README.md) も参照してください。

## mutate の範囲と実行上限

```sh
relca mutate model.relca --mutant-limit 512 --kinds evidence_missing,original_access_removed
```

`--mutant-limit N` は変異体全体の実行数の上限です。既定は128、指定できるのは
1〜4096の整数です。0、負数、非整数、空値、上限超過は `invalid` (exit 3)。
受理した計算上限は `run.mutant_limit` と `ledger.execution.limits.mutant_limit` に入ります。
上限後の候補は `not_run` / `mutant_limit` として残り、run は
`completion: inconclusive` / `reason: mutant_limit` になります。
未対応種が選択されていれば、最上位の verdict は優先順により `unsupported` です。

`--kinds` は `mutations[].kind` の名前をカンマで区切って指定します。
名前は `evidence_missing`、`authority_expired`、`hold_delayed`、`command_resent`、
`summary_reused`、`stale_definition_write`、`receipt_unknown`、
`old_promise_denominator_removed`、`refusal_path_cut`、`original_access_removed` です。
未知名、空リスト、空要素、重複、余白は `invalid`。省略時は全種です。
選外の行も残り、`status: not_selected`、`reason: not_selected`、`mutants: []` と表示します。
`unchecked` の `mutation.kind.<kind>` も併読してください。
選外は成功にも検出漏れにも数えず、verdict と completion は選択範囲だけで決めます。
明示した範囲の除外であって計算の切断ではないため、選外だけでは `inconclusive` にしません。
実行した変異体が0件でfoldがsatisfiedになる場合だけ `vacuous` (exit5)、reasonは `no_selected_mutant` です。
未対応種が選択されてfoldがunsupportedなら、0件でもunsupported/exit4を維持します。
該当箇所なしやkinds/sitesの積集合が空でも、成功とはしません。元の検査の拒否や計算の切断は別扱いです。

| 欄 | 意味 |
|---|---|
| `mutations[].candidates` | 既存の列挙規則で見つかった候補数。選外や `not_run` の分も含む。該当なしは0。未対応・元の検査で停止など、列挙できない場合は null |
| `run.candidates_total` | 列挙できた種類の候補数の合計。選外も含む。列挙に進めなければ null |

全種を回すには合計、種類を絞るなら選択した行の合計を実行上限の目安にします。
合計が4096超なら種類を分けられますが、1種類だけで4096超なら全候補を実行できません。
上限を増やしても、個々の検査の切断や未対応は解消するとは限りません。
候補数は実行数でも網羅率でもなく、null を0と読んではいけません。

場所を絞るには `--sites ID1,ID2` を指定します。ID は出力の `mutations[].mutants[].id`
をそのまま使います。kind と既存の site の区別を含むため、同じ reads でも種類が違えば
別IDです。ソースを編集するとIDが変わることがあります。未知ID、空指定・空要素、重複は invalid。
`--kinds` と両方指定すると積集合を実行します。
選外の場所も mutant 行に残り、`result: not_selected`、`reason: not_selected`、report は null。
`unchecked` の `mutation.site.<id>` と併読します。選外の場所は予算・検出・未検出・成功に
数えず、それだけでは inconclusive にしません。候補数は選外の場所も含みます。

授権失効の変異は、全操作が0分で、切断の無い完了時刻もすべて絶対時刻0の場合、
`not_applicable` / `no_time_elapse`、候補数0です。期限0は時刻0を含むためです。
0分の操作でも、到着や待ちで時刻が進む場合はこの除外をしません。
全応答のbackoffが0であることも直接検査します。正のbackoffが到達しない箇所にあっても除外しません。
静的な正のWaitまたはWaitUntilがあれば、End後や到達しない枝も含めて同じく除外しません。
カレンダーも別の時間源なので、Actor・資源・人のカレンダーを含むresource_timeがある場合は、この除外をしません。
この保守的な判定では実際の時刻が進まなくても変異体がundetectedとなり得ますが、その場合はviolated/exit1でありexit0にはなりません。

構成を明示選択したときは、構成の数を問わず実効process内のcall・reads・guard・終端に候補を絞ります。
共有Capabilityのaffected_bindingsにも、そのprocessの呼出しだけを記録します。
種類の候補箇所を1つでもsitesで外すと、その種類の段はcheckedに入れずuncheckedへ残します。
全体の生成/比較の段も同様です。kindsで1種類でも外した場合も全体の段はuncheckedです。
比較を0件しか行っていない段もcheckedには載せません。
構成の数を問わず、明示した構成の実効processで列挙します。無指定の宣言ベースの列挙は保持します。
不正な上限やsite IDでは上限は未使用なので、run/ledgerのmutant_limitは
not_applicable / value:null / reason:no_analysis と表示し、128とは表示しません。
