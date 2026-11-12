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
