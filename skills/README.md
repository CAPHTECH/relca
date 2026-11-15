# skills/

Relcaを **使う側** のAIエージェント向けの補助資料とスクリプト。本体の開発規約はルートのAGENTS.mdを参照する。

| スキル | 中身 |
|---|---|
| `relca-model/` | 業務の説明からモデルを起こし、relca で検査し、結果を人間が確かめるページにして渡す手順 |
| `relca-design/` | 現行と変更案を検査し、検査の穴を塞ぎ、違反や空虚になった統制を構造で直して、残った案を判断へ渡す手順 |

## 使い方

- エージェントには `skills/relca-model/SKILL.md` を読ませる(frontmatter の `description` がいつ使うかの一文)。長い参照は `references/` にある。
- 結果を人に渡すときは `skills/relca-model/scripts/review_page.py` で単一の HTML を作る。python3 の標準ライブラリだけで動く。
- 生成した HTML はリポジトリに commit しない。

## 検査

```sh
CARGO_NET_OFFLINE=true CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0 cargo build -p relca-cli
python3 skills/relca-model/scripts/test_review_page.py
python3 -B skills/relca-design/scripts/test_design_examples.py
```

`./scripts/check.sh` の中の `scripts/check-refs.py` は、ソースの `.md`・`.py` 等に残る既存の来歴番号も検査する。その番号に限って版つき節参照を維持し、新しい説明は `docs/guide/` や実在する fixture を参照する。外部の貢献者に設計書の参照は要求しない。
check.sh の他の段(fmt・clippy・cargo test)はこのディレクトリの python を回さないので、上の2つは別に回す。
