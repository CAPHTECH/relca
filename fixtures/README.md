# 手計算つき fixture

各ディレクトリには `model.relca`（入力）、`calc.md`（計算と境界）、
`expected.json`（照合する値）があります。`invocation.json` がある場合は、
コマンド、構成、シナリオなどの選択もその条件に含みます。
数値は模型から導く期待値であり、実業務の測定や性能ベンチマークではありません。

## 小さい例から読む

| 例 | 手計算で見るもの | 故障対照 |
|---|---|---|
| [serial-time](serial-time/) | 3+5=8分、作業量8分 | capacity の欠落を拒否 |
| [parallel-time](parallel-time/) | max(3,5)=5分、作業量8分 | join の欠落を拒否 |
| [resource-contention](resource-contention/) | 容量1で8分 | 容量0では到達性が violated |
| [any-drain](any-drain/) | 採用3分、終了5分 | cancel の欠落を拒否 |
| [coop-cancel](coop-cancel/) | 取消要求3分、ack4分、作業量7分 | clock3分では inconclusive |
| [nondet-race](nondet-race/) | fast の経過4〜5分、作業量7〜8分 | fast の結果を制限すると到達性が violated |
| [cut-declared](cut-declared/) | 経過8分、作業量8分、Cut の表示 | include の漏れや除外 Actor の呼出しを拒否 |
| [agency-lineage](agency-lineage/) | 3判定を2つの根拠源に集約 | 根拠の再利用で1源へ減る |
| [contract-response](contract-response/) | 終端5分と期限5分の一致 | 終了6分は violated、clock4分は inconclusive |
| [k8-conditional-error](k8-conditional-error/) | (1/50)(1/5)=1/250 | 見逃し率1/4なら1/200 |
| [k8-common-cause](k8-common-cause/) | 同時失敗1/10、独立積1/100ではない | 上限を1/100にすると violated |

各例の `.broken-*` ディレクトリが対照です。変えた入力と、検出される欄を
`calc.md` で確認してください。Agency などの構造欄の差を検査する対は、
故障側でも全体の verdict が satisfied の場合があります。

## 分野別の入口

- 時間・資源・取消: 上表と `duration-*`、`rsrc-*`。
- 契約・実行者・権限: `contract-*`、`exec-*`、`auth-*`、`resp-*`、`resp2-*`。
- 確率・観測状態: `prob-*`、`state-*`、`aobs-*`、`sinv-*`、`aset-*`、`rec-*`。
- 流入・方策・シナリオ: `arrival-*`、`pol-*`、`lim-*`、`scn-*`。
- 指標・品質・待ち・比較: `met-*`、`qual-*`、`wait-*`、`eval-*`、`qtl-*`。
- 構成・系譜・規則: `config-diff-*`、`diff2-*`、`agency-*`、`agy2-*`、`lin-*`、`rule-*`。
- 反例・変更生成: `explain-*`、`mut-*`、`mut2-*`。

追加の組合せや内部状態の手計算は `crates/relca-core/tests/` と
`crates/relca-cli/tests/` の Markdown にあります。
[受入対応表](../docs/guide/acceptance.md) は39項目と実在する根拠を結びます。

### 開発時の記録について

fixture と試験文書にある Q4、C7、Stage C などの符号やレビューの指摘番号は、
開発時の作業単位の名前です。commit の識別子と ADR の番号は、非公開の開発履歴と設計記録を指し、
公開リポジトリでは辿れません。現在の使い方や対応範囲は利用ガイドを参照してください。

## 手計算と回帰の区別

`calc.md` では経路、イベント時刻、占有区間、分数、系譜集合などから値を導きます。
`expected.json` は部分照合の場合もあり、全出力の仕様を表すとは限りません。
core と CLI の試験には、手値、独立参照、単一変更の負対照があります。
一つの対の成功を全入力や現実の妥当性へ広げないでください。

現行 stdout/stderr/exit は golden snapshot と完全一致で照合します。
更新コマンドは `CARGO_NET_OFFLINE=true TMPDIR=.tmp ./scripts/update-snapshots.sh`。
採取した値を手計算の正解として使わず、snapshot を意味の検証と区別します。
