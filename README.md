# Relca

Relca は、業務の流れを小さなモデルに書き、起こりうる進み方を上限の範囲ですべて調べるツールです。
モデルには、誰が何をどの順で行うか、どの資源を使うかを書きます。
承認漏れ、職務分離の破れ、期限超過、資源の取り合い、デッドロックを、再生できる反例つきで見つけます。
変更案を同じ条件で並べて、時間、費用、品質、人の負荷を比べることもできます。

人、AI、サービス、文書、規則、台帳、外部の当事者を、どれも Actor として同じ形で書きます。
調べる対象は入力したモデルです。実際の業務や外部システムが正しいと認定するものではありません。

## できること

| やりたいこと | コマンド |
|---|---|
| 1 つの業務モデルで、規則の破れや詰まりを網羅的に探す | `check`、`explain` |
| 指摘がある場合だけ修正し、修正後の値か元の値を後続へ渡す | 値を返す `if`。`check`、`explain`、`simulate` |
| 差し戻しや再確認を、初回を含む上限回数つきの `repeat` で書く | `check`、`explain`、`simulate` |
| 判定に必要な根拠源を `evidence_basis` で求め、読取りの削除を検出する | `check`、`explain`、`mutate` |
| 契約を適用する構成を `applies_to` と理由で明示し、範囲外を未検査として残す | `check`、`explain`、`simulate`、`compare`、`diff`、`mutate`、`sensitivity`、`conform` |
| 時間、費用、品質、人の負荷を標本で見積もる | `simulate` |
| 現行と変更案など、複数の構成を同じ条件で並べる | `compare`、`diff` |
| 到着量や所要時間などの前提を変えて、結論が変わるかを見る | `sensitivity` |
| 検査そのものが誤りを捕まえるかを確かめる | `mutate` |
| 実際の業務ログが、モデルの契約に沿っているかを照合する | `conform` |

## 例: 入力した人が、そのまま承認できてしまう

入力担当 `c1` と承認担当 `a1` がいます。承認の実行者に `c1` も入っているため、同じ人が入力と承認の両方をしうる、というモデルです。

```relca
model ExecSeparation version "0.2" {
  type T = enum { ok }
  people [c1,a1]
  actor Clerk { executors [c1]; capability enter() -> T { duration fixed 1min; result nondet { ok } } }
  actor Approver { executors [a1,c1]; capability approve() -> T { duration fixed 1min; result nondet { ok } } }
  process Flow { x = Clerk.enter(); y = Approver.approve(); end done }
  bounds { cases 1; live_tasks 2; clock 20min }
  verify safety no_resource_overcommit
  contract Controls { separation Clerk.enter.finish, Approver.approve.finish }
}
```

`relca check` は、職務分離の契約を `violated` と判定し、exit 1 で終わります。

```console
$ relca check fixtures/exec-separation.broken-shared-approver/model.relca
```

| 性質 | 判定 |
|---|---|
| `safety no_resource_overcommit` | `satisfied` |
| `contract Controls #1 separation` | `violated` |

`relca explain` は、破れに至る手順を反例として示します。要点は次のとおりです。

| 時刻 | 出来事 |
|---|---|
| 0min | `c1` が入力を始める |
| 1min | `c1` が入力を終え、続けて承認を始める |
| 2min | `c1` が承認を終える。ここで分離の契約が破れる |

承認の実行者を `[a1]` だけにしたモデル(`fixtures/exec-separation`)は、`satisfied`、exit 0 になります。
出力は JSON で、反例の各段には時刻、Actor、操作、実行者、ソース上の位置が入ります。
Cut(モデルから外したもの)を宣言していないと、stderr に警告 `W-CUT-0` が出ます。

## 結果の読み方

exit は次のとおりです。

| exit | 判定 | 意味 |
|---|---|---|
| 0 | `satisfied` | 上限の範囲で、すべての性質が成り立った |
| 1 | `violated` | 成り立たない性質があった |
| 2 | `inconclusive` | 計算の上限に達したなどの理由で、判定できなかった |
| 3 | `invalid` | 入力が誤っている |
| 4 | `unsupported` | 未対応の構文や機能を含む |
| 5 | `vacuous` | 性質の前件が一度も起きず、検査として働いていない |

exit だけでなく、性質ごとの判定と、`assumptions` と `unchecked` の欄も読んでください。
判定は、次の区別を守ります。

- **計算の切断と業務の打切りを分ける。** 計算の上限に達したら `inconclusive` にします。終わっていない案件を、成功にも失敗にも数えません。
- **網羅探索と標本推定を分ける。** `simulate` の結果を、すべての進み方で安全だという保証として扱いません。
- **観測できる値と、本当の値(潜在の値)を分ける。** 業務の判断が潜在の値を読むモデルは、静的なエラーにします。
- **黙って省略しない。** 未対応の機能は `unsupported` にします。確率が書かれていなければ一様分布、単価が書かれていなければ 0 円、とは扱いません。

詳しくは [結果の読み方](docs/guide/results.md) と [保証の範囲](docs/guide/guarantees.md) を参照してください。

## 使い始める

Rust 1.88 以降と Cargo が必要です(edition 2024)。検査のスクリプトには Python 3 と bash も使います。

```sh
cargo build --release
./target/release/relca check fixtures/coop-cancel/model.relca
```

この例は `satisfied`、exit 0 になります。
モデルを調べるときは release build を使ってください。最適化しない debug build(`cargo build`)は、大きなモデルで数倍遅くなります。
モデルの書き方は [DSL の書き方](docs/guide/dsl.md) から読み始めてください。

## コマンド

| コマンド | 対象と限界 |
|---|---|
| `check <path>` | 有限モデルの網羅探索。切断に関わる性質は判定を保留する |
| `explain <path> [--configuration NAME] [--scenario NAME]` | check と同じ解析に、契約・安全性・デッドロックの反例と位置を加える。再生できない反例は採用しない |
| `simulate <path>` | 宣言した反復数と seed による標本推定。解決されていない非決定に出会ったら拒否する |
| `compare <path> --plans A,B[,C...] --mode exhaustive\|simulation` | 同じ条件の構成を並べる。宣言した必須条件と目的は、性質の検査と分けて評価する |
| `diff <path> --from A --to B` | 構成、契約、Agency(判断の主体)の差分。計算していない種類は `unchecked` に残す |
| `mutate <path> [--set bpr-default] [--configuration NAME] [--scenario NAME] [--mutant-limit N] [--kinds k1,k2] [--sites ID1,ID2]` | 変更を生成し、検査が差を捕まえるかを調べる。受理した計算上限は既定128、最大4096。選外は明示。未対応種があれば実行0件でもexit4。未対応が無く実行0件ならvacuous/exit5 |
| `sensitivity <manifest.json> --mode exhaustive\|simulation` | 指定した有限の点だけを分析する。連続した範囲や評価していない点は保証しない |
| `conform <spec> --log <events.jsonl> --scope event-contracts\|approval-burden` | 観測ログの照合。欠けた記録を成功に置き換えない |
| `verify <path>` | 外部の検証エンジンにはまだ接続していない。`unsupported`、exit 4 |

## 文書と例

- [DSL の書き方](docs/guide/dsl.md): 最小モデルと主な構文
- [根拠源を求める例](examples/evidence-basis.relca): 引数を通じた根拠と直接の読取り。[上流の読取りを外した対](examples/evidence-basis-missing.relca)は違反になる
- [構成ごとに契約を選ぶ例](examples/configuration-contracts.relca): Old は1分、New は2分で終了。承認の契約は New だけに適用し、Old の範囲外の契約を満たしたとは数えない
- [結果の読み方](docs/guide/results.md): 判定、exit、JSON の主な欄
- [保証の範囲](docs/guide/guarantees.md): 分析モード、上限、検査していない事項
- [請求書の例](examples/invoice/README.md): 入力、手計算、実行コマンドと境界
- [fixture の索引](fixtures/README.md): 正しい例と、わざと壊した対、手計算の読み方
- [受入対応表](docs/guide/acceptance.md): 受入の 39 項目と根拠の所在
- [モデル記述の補助資料](skills/README.md): 構文の早見表と確認用ページ

## 開発

| path | 中身 |
|---|---|
| `crates/relca-core` | Parser、型検査、状態展開、参照実行、確率、品質、構成比較 |
| `crates/relca-cli` | CLI と、JSON 出力の照合試験 |
| `crates/relca-test-support` | 試験専用の共有比較器。通常の build の依存には含めない |
| `fixtures/` | モデル、手計算、期待する JSON、呼出し条件 |
| `examples/invoice/` | 請求書のモデルと検算 |
| `docs/guide/` | 利用ガイドと受入対応表 |
| `scripts/` | 検査と snapshot の更新 |

変更したら `./scripts/check.sh` を通してください。fmt、clippy(警告をエラーとして扱う)、workspace の試験、節参照、Phase 1 の39行対応表、確認用ページ(`skills/relca-model/scripts`)の試験、公開準備の検査を順に実行します(公開版では公開準備の検査を除きます)。
`cargo-nextest` が PATH にあれば、`cargo nextest run --workspace` で unit/integration test を並列実行し、続けて `cargo test --doc --workspace` で doctest を実行します。
設定は `.config/nextest.toml` にあり、通常の試験は1枠、job を並列実行する大きな試験は4枠を使い、全体を16枠に収めます。試験内の job worker は最大4です。失敗時の再試行や時間切れによる打切りはせず、最適化しない標準の test build を使います。
nextest が無ければ、従来どおり `cargo test --workspace` で doctest も含めて実行します。選んだ実行方法はログに表示し、nextest 自体のエラーを fallback で隠すことはしません。
共通比較器の self-test は、既存の `fixture_match` と `wire` の名前を保ち、`fixtures` と `conflicts` の各試験 binary に1写しずつ置きます。他の試験は同じ比較器を共有するため、その self-test を重複して取り込みません。
同じソースで一度合格すると記録され、`--force` を付けると回し直します。
一時ファイルはソース木の `.tmp` に置きます。TMPDIR が木の外を指していても、その場所は使いません。
オフラインで回すときは、Cargo の依存ライブラリを先にキャッシュしてください。

stdout、stderr、exit は、現行の golden と完全に一致するかで回帰を検査します。
出力を意図して変えたときは、`CARGO_NET_OFFLINE=true TMPDIR=.tmp ./scripts/update-snapshots.sh` で更新します。
snapshot の一致は、手計算や独立した参照実装の代わりにはなりません。

## ライセンスと貢献

Apache-2.0([LICENSE](LICENSE))。Copyright 2026 CAPH TECH Inc.
公開先は <https://github.com/CAPHTECH/relca> です。crates.io には公開しません。
貢献の手順と DCO の署名は [CONTRIBUTING.md](CONTRIBUTING.md) を参照してください。
バイナリを再配布するときは、LICENSE と NOTICE を添え、同梱する依存ライブラリのライセンス条件と、必要な表示も確認してください。
