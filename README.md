# Relca

業務の手順と制約を有限モデルにして、変更案ごとの結果と検査できない範囲を比べるツールです。
人、AI、サービス、文書、規則、台帳、外部当事者を Actor として記述し、
As-Is と To-Be の時間、費用、品質、人的負荷、構成差分を調べます。

分析の対象は入力したモデルです。実業務や外部システムの正しさを認定しません。
観測値と潜在値、業務の打切りと計算の切断、網羅探索と標本推定を別々に扱います。
対応外の機能を省略して成功を返すことはありません。

## 使い始める

Rust 1.88 以降と Cargo、Python 3、bash が必要です。edition 2024 を使用します。
オフラインで実行する場合は Cargo の依存ライブラリを事前にキャッシュしてください。

```sh
cargo build
cargo test --workspace
./target/debug/relca check fixtures/coop-cancel/model.relca
./scripts/check.sh
```

この例の check は `satisfied`、exit 0 です。一時ファイルは既定でソース木の `.tmp` に置きます。
TMPDIR が木の外を指す場合も、その場所は使わず `.tmp` に置きます。
`check.sh` は fmt、clippy（警告をエラー扱い）、workspace の試験、参照と受入表の検査を実行します。
同じソースでの合格は記録され、`--force` で再実行できます。

## 文書と例

- [DSL の書き方](docs/guide/dsl.md): 最小モデルと主な構文。
- [結果の読み方](docs/guide/results.md): verdict、exit、JSON の主要な欄。
- [保証の範囲](docs/guide/guarantees.md): 分析モード、上限、未検査事項。
- [請求書の例](examples/invoice/README.md): 入力、手計算、実行コマンドと境界。
- [fixture の索引](fixtures/README.md): 正例と故障対照、手計算の読み方。
- [受入対応表](docs/guide/acceptance.md): 39 の項目と根拠の所在。
- [モデル記述の補助資料](skills/README.md): 構文の早見表と確認用ページ。

## コマンド

| コマンド | 対象と限界 |
|---|---|
| `check <path>` | 有限モデルの網羅探索。切断に関わる性質は判定を保留する |
| `explain <path>` | check と同じ解析に、対応する契約・安全性・deadlock の反例と位置を加える。再生できない反例は採用しない |
| `simulate <path>` | 宣言した反復数と seed による標本推定。未解決の非決定に遭遇したら拒否する |
| `compare <path> --plans A,B --mode exhaustive\|simulation` | 同条件の構成を並べる。宣言された hard と目的は性質の検査と分けて評価する |
| `diff <path> --from A --to B` | 構成、契約、Agency の差分。計算していない種類は unchecked に残す |
| `mutate <path> [--set bpr-default]` | 既定の変更種を生成し、割り当てた検査で差を調べる。未対応種を含む生成 run は exit 4 |
| `sensitivity <manifest.json> --mode exhaustive\|simulation` | 指定した有限の点だけを分析する。連続領域や未評価点は保証しない |
| `conform <spec> --log <events.jsonl> --scope event-contracts\|approval-burden` | 観測ログの照合。欠測を成功に置き換えない。承認負担は明示した profile と sidecar を要する |
| `verify <path>` | 外部バックエンドは未接続。`unsupported`、exit 4 |

exit は `satisfied=0`、`violated=1`、`inconclusive=2`、`invalid=3`、`unsupported=4`、`vacuous=5`。
exit だけでなく、性質ごとの verdict、assumptions、unchecked も読んでください。

## ソースの構成と検査

| path | 中身 |
|---|---|
| `crates/relca-core` | Parser、型検査、状態展開、参照実行、確率、品質、構成比較 |
| `crates/relca-cli` | CLI と JSON 出力の照合試験 |
| `fixtures/` | モデル、手計算、期待 JSON、呼出し条件 |
| `examples/invoice/` | 請求書の模型と検算 |
| `docs/guide/` | 利用ガイドと受入対応表 |
| `scripts/` | 検査と snapshot 更新 |

stdout、stderr、exit の現行 golden は完全一致で回帰検査します。
意図した出力変更では `CARGO_NET_OFFLINE=true TMPDIR=.tmp ./scripts/update-snapshots.sh` で更新します。
snapshot の一致は手計算や独立参照の代わりにはなりません。
過去の build と byte 単位で互換にすることは要求していません。

## ライセンスと貢献

Apache-2.0（[LICENSE](LICENSE)）。Copyright 2026 CAPH TECH Inc.。
公開先は <https://github.com/CAPHTECH/relca>。crates.io には公開しません。
貢献の手順と DCO 署名は [CONTRIBUTING.md](CONTRIBUTING.md) を参照してください。
バイナリの再配布には LICENSE と NOTICE を添付し、同梱する依存ライブラリの
ライセンス条件と必要な表示も確認してください。
