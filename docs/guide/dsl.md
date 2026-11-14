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

## 分岐した値を先へ渡す

指摘がある場合だけ修正し、修正後の値か元の値を一つの後続処理へ渡すには、値を返す `if` を使います。

```relca
ready = if r == fail {
  f = Office.fix(d)
  yield f
} else {
  yield d
}
s = Office.send(ready)
```

全入力は [分岐と合流の例](../../examples/merge-if/model.relca) にあります。
`relca check examples/merge-if/model.relca` は `violated`、exit 1 です。
修正する経路は1+1+2+1=5分、修正しない経路は1+1+1=3分で、4分の期限を超える経路があります。
修正する確率は1/4、しない確率は3/4です。この例の分布は説明用の仮定で、実業務の測定ではありません。

各枝の末尾は `yield 結果名` または `end 終端名` にします。
`else` は必須で、少なくとも一方の枝から値を渡す必要があります。
`end` の枝は案件を終了し、合流後の処理へは進みません。入れ子の値を返す `if` も使えます。
`yield` は、その枝の末尾で確実に利用できる呼出し結果か、先に作った合流値だけを返します。
すべての `yield` 元は同じ型名でなければなりません。型の形が同じでも、別名の型は合流できません。

合流は値の複写だけで、Task、所要時間、資源の使用、乱数の抽選を増やしません。
枝の操作が書いた観測値や潜在値は引き継ぎます。
合流値を読めるのは、後続呼出しの引数、process の `if` 条件、後の `yield` です。
`wait until`、操作や権限の guard、契約・性質・指標の式、kernel の `given`、通常の代入や数値の宣言からは読めません。
これらの位置では、既存の診断 code に理由 `merge_read_position` を添えて静的に拒否します。

通常の結果名と合流値の名前は、process 内で重複できません。
枝の内部の結果名を後で直接読むには、続行するすべての経路で確実に作られている必要があります。
同じ名前を両枝で宣言しても暗黙の合流にはなりません。上の形で別の合流名を付けてください。
合流名を観測 case の欄と同名にすることもできません。合流はその欄への書込みではありません。
`yield` はこの位置だけで文として認識され、`yield = Office.write()` のような結果名には従来どおり使えます。

現在接続しているのは `check`、`explain`、`simulate` と指標・品質です。
複数案件にも使えます。枝内の `fork` / `join all` は使えますが、取消されうる結果は返せません。
次の組合せは未対応です。

- 合流値を `wait until`、契約、操作の `requires`、権限の scope、`join any` の `where`、property・metric から読む。
- 繰り返し、応答喪失、再送、recovery の公開結果を `yield` する。
- `repeat` の body・`on_limit`、recovery handler、縮退経路の中に値を返す `if` を置く。
- 合流を含むモデルの `compare`、`diff`、`mutate`、`sensitivity`、`conform`、および `contest`・`quantile` との併用。
- 状態の値、潜在値、enum の定数を `yield` する。潜在値は `E-POL-LATENT` で拒否します。

合流前に固定した値を、その後の `repeat` の操作の引数へ渡すことはできます。
回をまたいで値を更新し、次の回へ渡す仕組みとは別です。

| code | reason または message | 意味 |
|---|---|---|
| `E-CON-0` | `merge_fallthrough` | 枝の末尾に `yield` または `end` が無い |
| `E-CON-0` | `merge_unreachable_after_yield` | `yield` の後に同じ block の文がある |
| `E-CON-0` | `merge_no_continuing_branch` | 両枝が `end` で閉じる |
| `E-CON-0` | `merge_yield_outside` | 値を返す `if` の枝末尾以外で `yield` を使った |
| `E-TYPE-0` | `merge_type_mismatch` | `yield` 元の型名が異なる |
| `U-SEM-0` | `merge_source_unsupported` | 結果 binding 以外を返そうとした |
| `U-SEM-0` | `merge_feature_combination` | 未対応の参照位置・組合せ・分析 |
| 既存の `U-SEM-0` / `U-SYN-0` / `E-SYN-0` / `E-TYPE-0` / `E-NAME-0` | `merge_read_position` | 許可されていない位置で合流値を読む |

`repeat` の中は `loop_feature_combination`、`conform` は `U-LOG-1` で拒否します。
未定義の結果名は `E-NAME-0`、未完了・取消されうる結果は従来どおり `U-SEM-0` です。
[合流の出力](results.md#合流の出力)も参照してください。

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

確認用ページの `--assumptions` に出所を申告するときは、繰り返しの上限を
`NAME.at_most`、明記した gap を `NAME.gap` とします（例: `rework.at_most`、`poll.gap`）。
Actor の `via model` は `Actor.via_model` です。省略時既定の gap は宣言の target にしません。
申告の扱いと入力例は[確認用ページと仮定の出所](results.md#確認用ページと仮定の出所)を参照してください。

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

### 契約を適用する構成を選ぶ

契約は、何も指定しなければすべての構成に適用されます。
ある構成だけに必要な統制は、名前付きの契約に `applies_to` と理由を書いて区別します。

```relca
contract Controls applies_to [New] reason "Oldには承認画面がない" {
  authority_basis Screen.approve Mgr
}
```

[実行できる全文例](../../examples/configuration-contracts.relca)では、Old は支払だけ、
New は承認してから支払います。各操作は1分です。

```sh
relca check examples/configuration-contracts.relca --configuration Old
relca check examples/configuration-contracts.relca --configuration New
relca compare examples/configuration-contracts.relca --plans Old,New --mode exhaustive
```

| 構成 | done までの時間 | reachable end.done | Controls の扱い | check の exit |
|---|---:|---|---|---:|
| Old | 1分 | satisfied | 範囲外で未検査 | 0 |
| New | 2分 | satisfied | satisfied | 0 |

Old の契約は satisfied になったのではなく、検査する性質から外れています。
`contract_scope` と `unchecked` に範囲と理由が残ります。
Old では承認操作を呼ばないという既存の警告も残ります。
詳しくは[結果の適用範囲](results.md#契約の適用範囲)を参照してください。

- 適用先は宣言した構成名の空でない並びです。未知名、重複、configuration の無いモデルではエラーになります。
  全構成を列挙した場合の意味は、指定を省略した場合と同じです。
- 契約名と `reason "..."` は必須です。名前なしの契約への指定は未対応、理由の欠落は構文エラーです。
  Cut の除外理由と同じく空の文字列も構文上は受理しますが、使うときは除外の理由を記入してください。
- 適用範囲内で前件が一度も起きなければ、従来どおり `vacuous` です。
  到達しない枝に対象があるだけでも、自動で範囲外にはしません。
- 範囲外の構成の手順や、別の契約の異議経路から対象を呼ぶ場合は W-CONF-2 で警告します。
  到達しない枝、復旧、再送、縮退の代替も呼出しの候補に含め、実際に開始するかどうかとは分けます。
  Capability を参照しない state invariant も、検査を外していることを警告します。
- `waive` の範囲は参照する response の範囲の部分集合にします。
  `contest` を含む契約への指定は、実行経路も変えてしまうため未対応です。
- `verify` の要求は構成ごとに外せません。適用する契約と verify がどちらも0件なら、
  範囲外の宣言だけで成功とはせず `unsupported` になります。
- `intent { preserve Controls }` と書いた行き先で Controls が範囲外なら、diff は E-DIFF-0 で拒否します。

`bounds` の `cases`、`live_tasks`、`clock` は計算の上限です。
`arrivals`、queue、`scenario` の horizon などの業務条件と混同しないでください。
`simulation replications N seed S confidence P` は simulate の標本設定です。
`policy observable fifo ...` などの方策は、実際に固定したい選択にだけ宣言します。
非決定を消して simulate を通すためだけに追加しないでください。

## 根拠源を求める契約

`evidence_basis [direct] Actor.capability [Source, ...]` は、その操作が正常に終了するたびに、
指定したすべての根拠源が含まれることを求めます。角括弧の `direct` は省略可能部分で、
入力するのは `evidence_basis Clerk.match_po [Invoice,PurchaseOrder]` のような形です。
根拠源に指定できるのは `source` を宣言した Actor だけです。
空の並び、重複、Actor の state、観測変数、潜在変数は要求に書けません。

[実行できる全文](../../examples/evidence-basis.relca)の要点は次のとおりです。

```relca
actor Invoice { source document }
actor PurchaseOrder { source record }
actor Clerk { capacity 1
  capability enter() -> V {
    reads [Invoice]; duration fixed 1min; result nondet { pass }
  }
  capability match_po(e:V) -> V {
    reads [PurchaseOrder]; duration fixed 1min; result nondet { pass }
  }
}
contract Basis { evidence_basis Clerk.match_po [Invoice,PurchaseOrder] }
```

`enter` の出力 `e` を `match_po(e)` に渡すので、`p` の根拠は
自分の `reads` の PurchaseOrder と、引数を作った操作の Invoice の和です。
2操作は順に各1分で終了し、時刻2分に両方の源を含むため、次の検査は `satisfied`、exit 0 になります。

```sh
relca check examples/evidence-basis.relca
relca mutate examples/evidence-basis.relca --kinds evidence_missing
```

この mutate は2つの読取り削除を生成し、どちらも `detected` となって exit 0 で終わります。
一方、[上流の `reads [Invoice]` を外した対](../../examples/evidence-basis-missing.relca)は、
戻り値も終了時刻も同じですが、`p` の根拠から Invoice が欠けるため `violated`、exit 1 です。

```sh
relca explain examples/evidence-basis-missing.relca
```

全文例の契約を `evidence_basis direct Clerk.match_po [Invoice]` に置き換えても違反になります。
`direct` はその操作自身の `reads` の source Actor だけを見るため、引数から届く Invoice は数えません。
引数の系譜は呼出しの引数から計算し、`reads [e]` のように引数名を重ねて書いても変わりません。
その値を作った操作の読取りを引き継ぐのは、操作が正常に終了して返した値だけです。
`requires` の拒否で返る `else` の値や、その拒否結果を応答として採用した値には、
拒否された操作の読取りを加えません。他に根拠が無ければ空の集合です。

観測変数の `reads [po]` では、読み手の前で必ず正常終了する書き手だけを数えます。
読む時点の値を最後に書いた操作が対象です。別の操作で必ず上書きされる古い値の根拠は数えません。
上書きの有無が分岐で変わる場合は、ありうる最後の値の根拠を分けて扱います。
同じ block か外側の block の先行操作が対象で、分岐や fork の枝の書き手、
`requires` で拒否されうる書き手、省略されうる再送試行は、必ず終わるとはみなしません。
要求した源がこうした辺からしか届かなければ `unsupported` で閉じます。
ただし、別の要求源が確定で欠ける場合の違反は残します。
`if`、`requires`、`wait until` の条件や `kernel_reads latent`、kernel の `given` は読取りに数えません。

正常終了かどうかは、戻り値の `pass` / `fail` という名前ではなく、操作が正常に終わった事象で区別します。
受理されたモデルで、取消や拒否で一度も対象が正常終了しなければ、その契約は `vacuous` です。
計算の切断による未完了は `inconclusive` で、成功にも空虚な成功にも置き換えません。
Agency の判定行に出ていない操作にも使えますが、契約を足して Agency や責務を補完するものではありません。
実際の資料アクセスを観測・認定する契約ではなく、モデルの `reads` とデータの系譜を照合します。

| 入力・分析 | code と message 内の識別語 |
|---|---|
| 未知の源 | `E-NAME-0`、`undefined source` |
| 空の並び / 重複 | `E-CON-0`、`evidence_basis_empty` / `evidence_basis_duplicate_source` |
| source Actor 以外を要求 | `E-CON-0`、`evidence_basis_not_source` |
| 必ず終了する辺だけでは要求を確かめられない | `U-SEM-0`、`evidence_basis_conditional_snapshot` |
| 異議経路の操作を対象とする契約 | その性質だけ `U-SEM-0`、`evidence_basis_contest_route`。他の性質の判定は残す |
| Cut で除外した Actor を要求 | `E-CUT-1`。既存の契約・条件式と同じく、範囲外の Actor 参照を拒否 |
| repeat との併用 | `U-SEM-0`、`loop_feature_combination` |
| 縮退経路・recovery との併用 | `U-SEM-0`、`evidence_basis_feature_combination` |
| 選んだ構成にこの契約が適用される compare / diff / sensitivity | `U-SEM-0`、`evidence_basis_unsupported_command` |

`check`、`explain`、`mutate` で扱い、`simulate` では標本中の違反だけを確認します。
応答喪失と有限再送は既存の操作・権限契約が受理する範囲で扱い、応答の受信と操作の正常終了を分けます。
`conform` は読取りをログで観測しないので、適用されるこの契約を検査せず `unsupported` にします。
範囲外の契約はこれらの未対応要求にも数えず、適用範囲の欄に残します。
詳しくは[根拠の結果と証跡](results.md#根拠源の要求と証跡)を参照してください。

要求を書くのは、業務で本当に必要な根拠源だけです。
未検出を0件にするために、すべての `reads` を契約へ転記することは勧めません。

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
