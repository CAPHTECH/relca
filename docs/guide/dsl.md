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

## 繰り返し

差し戻し後の再提出や、状態を待つ再確認を、回数に上限のある `repeat` で書けます。
基本形は次のとおりです。角括弧は省略できる部分を示し、入力には書きません。

```text
repeat NAME at_most N [gap fixed DURATION] { BODY } on_limit { LIMIT }
```

`N` は**初回を含む回数**で、1〜32の整数です。`on_limit` は必須です。
body の各経路は `again`、`leave`、`end NAME` のいずれかで閉じます。

| 文 | 進み方 |
|---|---|
| `again` | 次の回の body へ進む。最後の回なら `on_limit` へ進む |
| `leave` | 繰り返しを抜け、`repeat` の後の文から続ける |
| `end NAME` | 案件をその終端で終了する。繰り返しの後の文も実行しない |

`on_limit` の各経路は `leave` か `end NAME` で閉じ、`again` は書けません。
`again` / `leave` の後ろに、同じ block の文を置くこともできません。
`gap fixed 5min` は次の回があるときだけ、その回の前に待ちます。
最初の回の前と、最後の `again` から `on_limit` へ進む間には待ちません。
省略時の gap は0分です。`again` に至る各経路には、正の最短所要時間を持つ操作、
正の固定 `wait`、正の gap のいずれかが必要です。操作が `requires` で即時拒否されうる場合、
その操作の時間だけではこの条件を満たしません。

`NAME.round` は1から始まる現在の回です。読めるのは**その繰り返しの body 内だけ**で、
`if`、呼び出す操作の `requires`、結果 kernel の `given` に使えます。
`requires` や kernel の宣言は body の外に置けますが、値を使う呼出しは body 内に限ります。
後続の文、別の繰り返し、`on_limit` からは読めません。
観測値と潜在値は回をまたいで引き継ぎ、毎回初期化するわけではありません。

### 差し戻しと再提出

確認に通れば抜け、不合格なら修正して再確認します。下の0.5は説明用の仮定です。
実際に使うときは、その業務の結果分布に合わせてください。

```relca
model Rework version "0.2" {
  type Verdict = enum { pass, fail }
  kernel Inspection { * -> { 0.5: pass; 0.5: fail } }
  actor Office { capacity 1
    capability inspect() -> Verdict { duration fixed 1min; outcomes kernel Inspection }
    capability correct() -> Verdict { duration fixed 2min; result nondet { pass } }
    capability escalate() -> Verdict { duration fixed 1min; result nondet { pass } }
  }
  process Main {
    repeat rework at_most 3 {
      r = Office.inspect()
      if r == pass { leave } else { c = Office.correct(); again }
    } on_limit { e = Office.escalate(); end escalated }
    end done
  }
  cut Scope { include [Office] }
  bounds { cases 1; live_tasks 1; clock 20min }
  verify reachable end.done
  contract Deadline { response case.received -> one_of(end.done,end.escalated) by 9min after case.received otherwise unresolved }
}
```

`rework.relca` に保存して、`relca check rework.relca` と `relca explain rework.relca` を実行します。
どちらも `violated`、exit 1です。3回とも不合格の経路は、確認と修正に9分、
上限後の操作に1分の合計10分となり、9分の期限を超えます。
explain はこの経路の反例を示します。回数は1〜3回、`on_limit` に進む確率は1/8です。
出力の読み方は[繰り返しの出力](results.md#繰り返しの出力)を参照してください。

### 条件を待つ再確認

確認結果が `ready` になるまで、5分の間隔を空けて確認します。
この模型では、初回は `pending`、2回目以降は `ready` と kernel に明記しています。
回に依存する `given` の行は、到達しない回の分も含めて列挙します。

```relca
model Poll version "0.2" {
  type Status = enum { pending, ready }
  kernel Readiness given [poll.round] {
    1 -> { 1: pending }
    2 -> { 1: ready }
    3 -> { 1: ready }
  }
  actor Registry { capacity 1
    capability read() -> Status { duration fixed 1min; outcomes kernel Readiness }
  }
  process Main {
    repeat poll at_most 3 gap fixed 5min {
      s = Registry.read()
      if s == ready { leave } else { again }
    } on_limit { end timed_out }
    end done
  }
  cut Scope { include [Registry] }
  bounds { cases 1; live_tasks 1; clock 20min }
  verify reachable end.done
}
```

`poll.relca` に保存して、`relca check poll.relca` と `relca explain poll.relca` を実行します。
どちらも `satisfied`、exit 0です。2回の確認が各1分、間の gap が5分なので、
7分で `done` に到達します。回数は2回、`on_limit` は0件です。
この例に違反の反例はなく、explain の `counterexamples` は空です。

### 利用できる範囲と診断

body と `on_limit` で使えるのは、操作の呼出し、`if`、固定 `wait`、終了や繰り返しの制御です。
次は未対応で、`unsupported` になります。

- `over` を使う明細の走査、繰り返しの入れ子、再到着、無限の繰り返し。
- body 内の `fork` / `join`、条件待ち `wait until`、縮退経路、および呼出しの
  `response_loss` / `retry` / recovery との組合せ。recovery や縮退経路の中への `repeat` も置けません。
- 繰り返しを含むモデルの `compare`、`diff`、`sensitivity`、`mutate`、`conform`。
- `contest` による異議経路、`quantile` による分位点との併用。
  これらはモデルの宣言・指標であり、単独の CLI コマンドではありません。

主な診断を次に示します。reason は `diagnostics[].message` 内の識別語です。
必須部分の欠落などは、reason ではなく `expected ...` の構文診断になります。

| code | reason または message | 意味 |
|---|---|---|
| `E-SYN-0` | `expected at_most` / `expected on_limit` | 必須部分の欠落 |
| `E-CON-0` | `loop limit must be positive` | 回数が0 |
| `E-CON-0` | `loop_fallthrough` | body または `on_limit` に閉じていない経路がある |
| `E-CON-0` | `zero_time_iteration` | 0分で `again` に戻れる経路がある |
| `E-CON-0` | `loop_again_in_limit` | `on_limit` 内に `again` がある |
| `E-CON-0` | `loop_unreachable_after_control` | `again` / `leave` の後に同じ block の文がある |
| `E-NAME-0` | `loop name collision` | 繰り返しの名前が他の名前と衝突する |
| `U-SEM-0` | `loop_expansion_limit` | 回数が32を超える |
| `U-SEM-0` | `loop_round_out_of_scope` | body の外などから `NAME.round` を読む |
| `U-SEM-0` | `loop_stage_unsupported` | `over` を使う未対応の形 |
| `U-SEM-0` | `loop_feature_combination` | 未対応の入れ子・組合せ・分析 |

`conform` の繰り返しモデルの照合は `U-LOG-1` で拒否します。
この場合も message に `loop_feature_combination` が含まれます。

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
