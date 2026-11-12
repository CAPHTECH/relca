# DSL の書き方

入力は UTF-8 の `.relca` ファイルです。モデルの外枠に宣言と process、
計算上限、検査したい性質を書きます。改行とセミコロンで項目を区切れます。
コメント構文、文字列外の非 ASCII、小数の時間は受け付けません。

## 最小モデル

```relca
model Small version "0.2" {
  type Ack = enum { ok }
  actor Desk {
    capacity 1
    capability work() -> Ack {
      duration fixed 2min
      result nondet { ok }
    }
  }
  process Main {
    answer = Desk.work()
    end done
  }
  bounds { cases 1; live_tasks 1; clock 3min }
  verify reachable end.done
}
```

保存して `relca check <path>` で検査します。結果値が一つでも `result nondet` と書けます。
複数の値を列挙した場合、それぞれが可能な選択であり、等確率という意味ではありません。
計算例は [serial-time](../../fixtures/serial-time/) から読めます。

## 宣言と順序

- `type` は有限の enum、bool、有界整数、限定した record や集合を扱います。
  無制限の整数や任意のデータ構造を渡すことはできません。
- `actor` に `capability` を書き、所要時間、入力、結果、読取り、書込み、commit を定めます。
  正の時間がかかる操作には Actor の容量か実行者の宣言が必要です。
  `people` と `executors` の ID は、職務分離と許可の照合にも使います。
- 呼出し `x = Actor.operation(...)` は終了まで待ち、結果を `x` へ結びます。
  `fork` 内の呼出しは並行に進みます。`join all` は全終了を待ち、
  `join any` は条件、同時刻の選択規律、残りの取消と解放待ちを明記します。
- `if` は観測できる値で分岐します。`observed case` や Actor の `state` と、
  `latent` を区別してください。潜在値は確率計算の入力には使えても、業務の分岐には使えません。
- `resource R capacity N` と操作内の `uses R: N` で資源を宣言します。
  開始に必要な Actor 枠、実行者、資源がそろった時点で獲得します。

`duration fixed 3min` のほか有限支持の時間分布を記述できます。
結果の確率には `outcomes kernel K`、条件依存には `kernel ... given [...]` と
操作側の `kernel_reads latent [...]` を使います。条件の行を省略して既定の分布に頼れません。
入力例は `fixtures/prob-*`、`fixtures/duration-*` にあります。

## 制約と分析範囲

`verify reachable end.X` は終端への到達、`verify safety no_resource_overcommit` は容量、
`verify probability reached ...` は確率についての性質です。
`contract` には `invariant`、`at_most_once`、`response`、`separation` などを書けます。
実行できる Permission、根拠を伴う Authority、責務、Standing は別々の宣言です。
宣言のない授権を、実行可能という理由だけで補いません。

`cut` では問い、`include`、`exclude`、`defer`、除外で扱えなくなる事項などを記述します。
未宣言の Cut は警告とともに出力されます。`configuration` は Cut と process を選び、
`intent` で構成変更の理由を添えます。複数の構成がある check には `--configuration NAME` が必要です。

`bounds` の `cases`、`live_tasks`、`clock` は計算の上限です。
`arrivals`、queue、`scenario` の horizon などの業務条件と混同しないでください。
`simulation replications N seed S confidence P` は simulate の標本設定です。
`policy observable fifo ...` などの方策は、実際に固定したい選択にだけ宣言します。
非決定を消して simulate を通すためだけに追加しないでください。

## 追加の記述

- `metrics` は時間、費用、件数を要求します。`breakdown waits` は待ちの内訳を追加します。
  単価の未設定は費用 0 ではありません。
- `property case.X` と `metric cohort.X` は案件の性質と期待件数の比を定義します。
  `metric quantile.X` は到達時刻の分位点です。compare の hard と目的は宣言した場合だけ評価します。
- 応答喪失と有限再送には、呼出しの `response_loss` と `retry max_attempts ...` を使います。
  最初の呼出しも試行数に含みます。冪等なキーの atomic 処理と非冪等な `effect_scope modeled` は
  別の模型です。応答が不明のままの案件は完了扱いにしません。
- calendar、明示 rank の priority、資格と backlog による performance、rates 付き contest は
  限定した組合せだけを扱います。詳しい構文例は
  [補助資料](../../skills/relca-model/references/dsl.md) と `crates/relca-core/tests/support/` を参照してください。

Agency と O/F/P/J/U/E/L は入力で申告する値ではありません。
JSON の構造欄として計算され、導けない項目は理由つきで残ります。
構文の受理と、その機能のすべての組合せの対応は別です。拒否診断と unchecked を確認してください。
