# AGENTS.md — Relca 実装規約

最初に `README.md`、`docs/README.md`、`fixtures/README.md` を読んでください。
構文は `docs/guide/dsl.md`、出力と保証の範囲は同じディレクトリのガイド、
動作例は `examples/invoice/` を参照します。

## 崩さない境界

- **Fail closed。**未対応の構文・制約・分布・バックエンド能力は `unsupported` / `invalid`。
  黙って省略せず、`satisfied` 以外で exit 0 にしない。
- **観測と潜在を分ける。**方策・業務判断の条件式から潜在値を読ませない。
- **Agency と O/F/P/J/U/E/L は宣言でなく分析出力。**Capability の操作契約と区別する。
- **Actor の種別名から意味を推測しない。**独立性は名前でなく根拠の系譜で判定する。
- **Silent Cut 禁止。**除外・保留を明示し、未宣言を警告する。
- **業務上の上限と計算上の切断を分ける。**切断は `inconclusive`。
  未完了案件を成功・失敗へ変換しない。
- **非決定性を一様乱数で埋めない。**シミュレーションを全スケジュールの検証と呼ばない。
- 確率未入力を一様分布、単価未設定を 0 円として扱わない。

## 検査

- 変更ごとに `./scripts/check.sh`。fmt、clippy、test、版つき節番号と受入表の照合を
  実行する。同じ内容での合格は記録される。
- 版つき節参照の照合は、既存の来歴番号に限る。番号の実在は `docs/design-sections.txt` で照合する。
  新しい説明は `docs/guide/` や実在する fixture を参照する。
  この索引は本文や機能の実装済み範囲を示すものではない。
- 意味論は fixture の手計算と独立参照で検証する。
  壊した対が落ちることも確認する。
- stdout/stderr/exit の現行 golden は完全一致の回帰検査。
  更新は `CARGO_NET_OFFLINE=true TMPDIR=.tmp ./scripts/update-snapshots.sh`。
  過去 build の byte 互換は要求しない。
  snapshot 一致を意味論の正しさや全範囲の証明にしない。
- 検査結果は実行条件と未確認事項を分けて報告する。

貢献には `CONTRIBUTING.md` の DCO 署名が必要です。
