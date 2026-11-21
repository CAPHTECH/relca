# DSL の書き方の参照

はじめて書く場合は[最小モデル](dsl.md#最小モデル)から始めてください。
このページの表にある `…` は、例の一部を省いた印です。入力には書きません。

## 宣言と状態

### Actor の範囲と状態の更新

| 書き方 | 役割 |
|---|---|
| `actor A { boundary "one registered actor"; … }` | Actor の境界を説明する。説明を付けても操作や検査の意味は変わらない |
| `actor Team { contains [Member]; capacity 1 }` | 構成員を記録する。構成員と包含側に実作業を重ねて容量を二重計上する形は `E-CAP-0` |
| `actor Team { overlaps [Member]; … }` | 他の Actor との重なりを記録する。`contains` の代わりではなく、実行者を共有させる指定でもない |
| `state holder: H = free last_write_wins` | 共有の Actor 状態に対して最後の書込みを残すことを明示する。案件ごとの状態・潜在状態には使えず、重複指定も `U-SEM-0` |
| `version_check [holder] else conflict` | 開始時に読んだ状態の版を更新時に照合する。古い版なら更新せず、戻り型の enum 値 `conflict` を返す |
| `version_check [holder] else conflict resolve priority descending` | 同時刻の競合では優先度が高い案件を先に照合する。古い版を有効にしたり、操作の開始順を変えたりはしない |

`last_write_wins` で最終値が一つに決まるとは限りません。競合する書込みを許すだけで、
書込み順が非決定ならどちらが最後になるかも非決定です。
`contains` と `overlaps` には、宣言済みの別の Actor を指定します。未知名は `E-NAME-0`、自分自身は `E-CON-0` です。
`overlaps` は関係の記録です。相手側の宣言を補ったり、容量を合算したりはしません。
`boundary` や `contains` を二度書くと `U-SEM-0` です。`contains` の一覧で同じ Actor を繰り返す場合も同じです。

`version_check` の対象は、その操作自身が書く共有の Actor 状態です。`else` に通常の結果と同じ値は使えません。
初期値やほかの書込みを調整して、古い版を強制的に通す指定ではありません。
`resolve priority descending` を付けられるのは、方策の区分に属する Actor が自分の共有状態を照合する場合だけです。
範囲が違うと `U-SEM-0 priority_resolution_scope_unsupported`、同じ状態を照合する操作間で解決方法が異なると
`U-SEM-0 priority_resolution_mismatch` になります。再送と版照合の併用は `U-SEM-0 retry_feature_combination` です。
例: [包含](../../fixtures/act-composite/model.relca)、[重複](../../fixtures/act-overlap/model.relca)、
[共有状態](../../fixtures/cf-lww/model.relca)、[優先度での版照合](../../crates/relca-cli/tests/snapshots/inputs/q5-conflict.relca)。

## 手順と応答

### 待つ場合と取り消す場合を分ける

固定の `wait 2min` は時刻を進めます。観測条件が成立するまで待つ場合は
`wait until ready == true` の形で書きます。潜在値は条件に使えません。
例: [成立済みの条件を待つ](../../crates/relca-core/tests/support/q4/condition-constant.relca)。
反復内で待てる位置は[繰り返しの診断](dsl.md#利用できる範囲と診断)を参照してください。
並行処理の `fork` と `join`、有限再送の指定は[早見表](../../skills/relca-model/references/dsl.md)に例があります。

| 書き方 | 役割 |
|---|---|
| `join any f where result == pass tie nondet remaining drain if none -> done` | 同時に採用できる結果があれば非決定で選び、残りを取り消さず終了を待つ |
| Capability 内の `cancel immediate` | `cancel_then_drain` の取り消し要求を受けると、追加の待ち時間なしで取り消す。既に起きた開始効果は巻き戻さない |

`remaining cancel_then_drain` を使う場合は、すべての枝の Capability に `cancel` を指定します。省略は `E-CANCEL-0` です。
`cancel forbidden_after_start` の枝は開始済みなら取り消さず、終了を待ちます。
例: [取り消さずに待つ](../../fixtures/agency-any/model.relca)、[同時の採用を非決定にする](../../fixtures/sov-any/model.relca)、
[即時取消](../../fixtures/req-cancel-immediate/model.relca)。

### 結果不明を照会して復旧する

操作の効果が出た後に応答だけが失われることがあります。応答が届かなかったという事実だけでは、
操作が成功したか、失敗したかは分かりません。最後の応答が不明のときに照会するなら、呼出しに
`on receipt_unknown` を付けます。受信できた場合はこの枝へ入らず、元の呼出しの後続へ進みます。
再送する場合は、最後の不明応答の後に一度だけこの枝へ入ります。

| 書き方 | 役割 |
|---|---|
| `receipt_query charge_keyed key key availability available` | 照会する Capability の結果源。`charge_keyed` は同じ Actor の元の操作。リテラルの `key` の後に照会のキー引数名を書く |
| `availability unavailable` | 照会は実行するが、結果は `still_unknown` |
| `availability nondet` | 照会が利用できる場合とできない場合を両方調べる。確率は補わない |
| `pay = Psp.charge_keyed(case.id) response_loss sequence [unknown] on receipt_unknown { … }` | 応答が最後まで不明の場合だけ復旧へ進む。`retry` があれば、その `exhausted end` の後に置く |
| `status = Psp.query_status(case.id) for command pay` | 元の呼出し `pay` と照会を結ぶ。元と同じ案件のキーを使う |
| `resolve effect pay using status` | 同じ呼出しの確定した照会結果で不明を解消する。元の試行の応答や効果を書き換えない |
| `stop receipt_unknown` | 不明のまま案件を止める。成功・失敗のいずれの `end` にも数えない |

照会先の操作は、`idempotency key` と `atomic` で同じキーの重複を拒む形にします。
`effect_scope modeled` だけの非冪等な操作は復旧対象にできません。照会の戻り型は
`committed_once`、`not_committed`、`still_unknown` の3値を持つ enum にします。
利用可能な照会では、元の操作が持つキー集合にキーがあれば `committed_once`、なければ `not_committed` です。
照会の引数は `CaseId` 型の一つだけです。その名前を `key` の後に書きます。
照会の Capability には `receipt_query`、`duration`、`cancel`、`uses` だけを書けます。`reads` や書込みは宣言しません。
他の Capability が元の操作の書く状態を `writes` に持つ形も未対応です。

以下を `recovery.relca` に保存してください。課金は1分、照会は2分、判断は3分、出荷指示は1分です。

```relca
model Recovery version "0.2" {
  type Pay = enum { charged, rejected_duplicate }
  type PaymentStatus = enum { committed_once, not_committed, still_unknown }
  type Decision = enum { proceed, withdraw }
  type Ack = enum { recorded }
  actor Psp { capacity 1; state keys: set<CaseId>
    capability charge_keyed(key: CaseId) -> Pay {
      idempotency key key; commits; writes [keys]; duration fixed 1min
      atomic { if key in keys { result rejected_duplicate } else { keys += key; result charged } }
      cancel forbidden_after_start
    }
    capability query_status(key: CaseId) -> PaymentStatus {
      duration fixed 2min; receipt_query charge_keyed key key availability available
      cancel forbidden_after_start
    }
  }
  actor Desk { capacity 1
    capability decide(s: PaymentStatus) -> Decision { duration fixed 3min; result nondet { proceed } }
  }
  actor Shop { capacity 1
    capability instruct_ship(d: Decision) -> Ack { duration fixed 1min; result nondet { recorded } }
    capability ship_known(p: Pay) -> Ack { duration fixed 1min; result nondet { recorded } }
  }
  process Flow {
    pay = Psp.charge_keyed(case.id) response_loss sequence [unknown]
    on receipt_unknown {
      status = Psp.query_status(case.id) for command pay
      if status == committed_once {
        decision = Desk.decide(status)
        if decision == proceed {
          resolve effect pay using status
          sh = Shop.instruct_ship(decision)
          end shipped
        } else { stop receipt_unknown }
      } else { stop receipt_unknown }
    }
    normal = Shop.ship_known(pay)
    end shipped
  }
  cut Scope { include [Psp, Desk, Shop] }
  bounds { cases 1; live_tasks 2; clock 20min }
  contract Controls { at_most_once Psp.charge_keyed.commit per case.id }
}
```

`relca check recovery.relca` では、照会結果が3分、判断と解決が6分、`end shipped` が7分です。
判定は `satisfied`、exit 0 です。`availability unavailable` に替えると3分で未解決のまま止まり、
`inconclusive`、exit 2 です。完了時刻は付きません。課金済みだったという元の効果は残ります。
この例は `sequence [unknown]` で応答喪失を固定しています。通常経路の `Shop.ship_known` が開始しない
という `W-UNREACH-0` と、責務の未宣言を示す `W-RESP-0` が出ます。

- `on receipt_unknown` と `for command` は直前の呼出しの一部です。改行はできますが、直前にセミコロンは置けません。
- 一つの process で使える復旧ブロックは一つです。照会の Capability を宣言したモデルでは、選択する process ごとに復旧ブロックが必要です。
  元の呼出しに `response_loss` または `retry` を付けます。元の Capability を同じ process の別の箇所から呼べません。
  ブロック内では通常の呼出し、照会、`if`、固定 `wait`、`resolve effect` を使えます。
  すべての経路を、解消済みなら `end`、未解消なら `stop receipt_unknown` で閉じます。
  元の後続へ戻りません。照会箇所は到達しない枝を含めて最大32です。
- `resolve effect` には、先行する照会の結果名をそのまま渡します。`if` で確定した値に絞った枝に置いてください。
  `using` に通常の呼出しの結果は渡せません。その結果を `if` で判断することはできます。
  元の応答の値は復旧内の条件や引数に使えません。潜在値や保護された元の操作の状態を判断に使うこともできません。
- `fork` / `join`、`wait until`、`repeat`、値を返す `if`、縮退、入れ子の復旧はブロック内では使えません。
  復旧内の呼出しにさらに `response_loss` や `retry` を付ける形、復旧と `contest`・`quantile` の併用も未対応です。
  `evidence_basis` との併用は `U-SEM-0 evidence_basis_feature_combination` です。

| 診断 | 見直す点 |
|---|---|
| `E-SYN-0 invalid_recovery_syntax` | 必須語や引数がない。復旧ブロックに閉じていない枝がある |
| `E-NAME-0 unknown_recovery_reference` | 呼出しや照会結果の参照名が存在しない |
| `E-TYPE-0 recovery_evidence_mismatch` | 照会のキー・結果型・参照先が合わない。解決する枝で結果が確定していない |
| `E-TYPE-0 recovery_provider_state_leak` / `E-POL-LATENT` | 保護された操作の状態、または潜在値を読んでいる |
| `E-CON-0 invalid_recovery_contract` | 復旧・解決の重複、元の操作を別の箇所から呼ぶ、元の呼出しに `response_loss` も `retry` もない、解決状態と終わり方が一致しない |
| `U-SEM-0 recovery_query_unmodeled` | 選んだ process に復旧がない、照会の宣言が元の重複拒否の仕組みと合わない、元の操作が書く状態に他の Capability も書き込む |
| `U-SEM-0 recovery_feature_combination` / `U-SYN-0` | 組合せが未対応、またはその位置では使えない形。反復・合流との組合せには個別の理由語も出る |
| `U-SEM-0 recovery_expansion_limit` | 照会箇所などの上限を超えた |

`resolve effect` は作業時間も外部効果も増やしません。たとえば返金する場合は別の操作として書きます。
出力では `response_receipts` に元の `unknown` が残ります。案件が閉じたかは `case_outcomes` と
`receipt_resolution` で別に確かめてください。他の契約が `vacuous` なら、復旧できても全体は
`satisfied` にはなりません。

## 作業を開始する条件

### カレンダーで時刻ごとの容量を指定する

| 書き方 | 役割 |
|---|---|
| `calendar Always through 21min on_close continue { [0min, 21min) capacity 2 }` | 時刻0から21分までの容量を定める。区間の右端は含めない。`on_close continue` では枠が減っても進行中の作業は続く |
| `actor Desk { executors [p, q]; calendar Always; … }` | Actor の作業枠にカレンダーを適用する |
| `resource Terminal capacity 1 calendar Day` | 資源の枠にカレンダーを適用する |
| `person_calendar p Early` | 人 `p` が開始できる時間を指定する |
| `rate Desk overtime_per_minute 3` | `metrics` 内に書く。時間外と判定した占有に適用する単価。通常単価への加算ではない |

閉じる時間も容量0の区間として明記します。隙間・重複などでカレンダーを分割できなければ
`E-CAP-0 invalid_calendar_partition` です。`on_close continue` の省略は `calendar_close_behavior_required`、
中断や再開を指定する形は `calendar_preemption_unsupported` で拒否します。どちらも `U-SEM-0` です。
一つでもカレンダーを使うなら、正の所要時間を持つすべての操作について、Actor、宣言した実行者全員、使う資源に
カレンダーが要ります。欠けると `U-SEM-0 calendar_target_unconfigured` です。
`through` は `bounds` の `clock` より後まで必要です。同じ時刻では `U-SEM-0 calendar_coverage_missing` です。
区間の容量は Actor・資源の容量以下、人は1以下にします。超えると `E-CAP-0 calendar_capacity_out_of_range` です。
カレンダー名が未定義なら `E-NAME-0` です。
カレンダーの名前や割当の重複は `U-SEM-0 duplicate_calendar` です。
時間外には、Actor の枠を超えた作業だけでなく、実行者が勤務時間を過ぎても続けた作業も含みます。
その時間の単価が未設定なら、費用を0円で補いません。カレンダーを使わず時間外単価だけを指定すると
`U-SEM-0 overtime_rate_requires_calendar` です。
[人の勤務時間](../../crates/relca-core/tests/support/q4/shift.relca)と
[資源の開始可能時刻](../../crates/relca-core/tests/support/q4/resource.relca)に全文例があります。

### 流入の上限と優先順位を分ける

| 書き方 | 役割 |
|---|---|
| `arrivals fixed [0min, 0min, 1min]` | 到着時刻を案件順に並べる。同じ時刻も指定できる |
| `case_priority fixed [0, 10, 20]` | 到着順の各案件に、整数の順位を一つずつ付ける |
| `policy observable priority descending` | 開始できる仕事を順位の大きい順に選ぶ。同順位の非決定を確率で埋めない |
| `concurrent cases 1 on_overflow wait_external fifo` | 同時に受け入れる案件数を制限し、枠が空くまで先着順で外に待たせる |
| `concurrent cases 2 on_overflow wait_external priority descending` | 外で待っている案件から順位の大きいものを選ぶ。初回の受入枠を後の到着のために予約しない |
| `concurrent cases 2 on_overflow reject` | 枠がない流入を拒否する |
| `concurrent cases 2 on_overflow end spill` | 枠がない流入を指定の終端 `spill` に記録する |

`concurrent` は受入の業務条件です。探索の切断を定める `bounds` とは別に書きます。
順位の数を `bounds` の `cases` と揃えます。上の表なら3件です。合わないと `E-ARR-0 priority_case_count_mismatch`、
順位を使う指定があるのに表がないと `U-SEM-0 priority_not_configured` です。
開始の優先順位は `policy` の区分にだけ効きます。`environment` や `party` の選択は非決定のままです。
`wait_external` には正の上限が必要です。`queue` と `concurrent` を併用するときは、あふれた場合の扱いを揃えます。
開始方策、外で待つ順序、[版の競合解決](#actor-の範囲と状態の更新)は別の指定です。
同時受入や順序の指定が合わない場合は、診断の対象が `case_priority`、`policy`、`concurrent` のどれかを確認してください。
例: [外で待つ](../../fixtures/wait-external/model.relca)、[順位による受入](../../fixtures/ld-priority-admission/model.relca)、
[拒否](../../fixtures/lim-count/model.relca)、[終端の指定](../../fixtures/lim-both-end-id/model.relca)。

### 資格・負荷と縮退経路を記述する

| 書き方 | 役割 |
|---|---|
| `qualified_executors [q]` | Capability を開始できる人を Actor の実行者の一部に絞る |
| `performance P basis "fixture assumption; not measured" { … }` | 実行者や負荷に応じて使う結果の分布を定め、仮定の出所を示す |
| `load queued_same_actor_at_start` | 開始時に同じ Actor で待つ仕事の数で分布を選ぶ |
| `executor anonymous { [0,1): kernel Low; [1,infinity): kernel Half }` | 匿名枠の負荷帯と結果 kernel。帯は0から隙間や重複なく並べ、最後を `infinity` にする |
| `load none; executor p { kernel K }` | 負荷によらず、人 `p` に kernel `K` を使う |
| `outcomes performance P` | Capability の結果源に表 `P` を使う。他の結果源とは重ねない |
| `degrade Manual when status == fail { … } else { … }` | 先行操作の既知の失敗に対する代替経路を明示する。`else` は必須 |

縮退の条件は既知の呼出し結果との等値です。応答不明を失敗値にして分岐させません。
`degrade` は process の直下に一つまで置けます。`else` の省略は `E-SYN-0 expected else`、2箇所以上は
`U-SEM-0 q6_capacity_limit` です。縮退を使う process 全体では、ほかに呼出し・`if`・`end` だけを使えます。
`wait` や入れ子の縮退、再送や応答が失われる呼出しは `U-SEM-0 q6_feature_combination` です。
喪失しない `response_loss none` だけなら使えます。未知の結果名は `E-NAME-0`、未対応の条件は `U-SYN-0` です。

性能表の `executor` は `qualified_executors` と一致させます。その指定がなければ Actor の `executors`、
どちらもなければ `anonymous` だけです。行の過不足や帯の欠けは `E-TYPE-0 performance_coverage`、空の根拠は
`E-CON-0 performance_basis_missing` です。各帯の kernel で条件と書込み先の契約を揃えます。違えば
`E-TYPE-0 performance_contract_mismatch` です。未対応の負荷は `U-SEM-0 load_metric_unsupported` です。
`outcomes performance` は、別の結果源、`atomic`、`version_check`、所要時間の分布と併用できません。
空の `qualified_executors []` とも併用できません。
到着時刻列の分布、異議、応答が失われる呼出しや再送との組合せも `U-SEM-0 q6_feature_combination` です。
再送を付けない `response_loss none` は使えます。
例: [有資格者の割当](../../crates/relca-cli/tests/snapshots/inputs/q6a-qualified.relca)、
[開始時の負荷で選ぶ分布](../../crates/relca-cli/tests/snapshots/inputs/q6a-burst.relca)、
[代替経路](../../crates/relca-cli/tests/snapshots/inputs/q6b-a7.relca)。

## 契約と権限

### 授権の条件と委任

次の欄は `authority NAME { … }` の内側に置きます。操作を始められる `permit` と、
契約 `authority_basis` が参照する授権の根拠は別の宣言です。

| 書き方 | 役割 |
|---|---|
| `scope approval == pass` | 操作の終了時点の観測値で、授権が有効かどうかを判断する |
| `not_by [z]` | 効力の対象から `z` を除く |
| `expires 5min` | 操作の終了をモデルの時刻5分と比べる。5分ちょうどは期限内 |
| `revocable_by p` | 取消権者を記録する。この欄だけで実行中に授権が取り消されることはない |
| `delegated_from Parent` | 宣言済みの `Parent` を委任元とする。操作や人の範囲を広げたり、期限を延ばしたりできない |

`scope` で潜在値を読むと `E-POL-LATENT`、process の結果名を読むと
`U-SEM-0 authority scope cannot read process bindings` です。
`not_by` に `grants` の相手や `issued_by` の人を含めると `E-CON-0` です。
委任を発行する子の `issued_by` には、子が授ける操作ごとに親からの許可が必要です。
親の範囲を広げる委任、期限の延長、循環委任は `E-CON-0`、未知の委任元は `E-NAME-0` です。
例: [観測条件](../../fixtures/auth-scope/model.relca)、[委任と期限](../../fixtures/auth-delegation-expiry/model.relca)、
[取消権者の記録](../../fixtures/auth-revocation/model.relca)。

### 事象・期限・免除

| 書き方 | 役割 |
|---|---|
| `requires_event Worker.run.start after Gate.check.finish(pass)` | `Worker.run` の開始ごとに、同じ案件で先に `Gate.check` が `pass` で終了したことを求める。自動で次の開始を待たせる文ではない |
| `excludes A.stop.finish, B.run.start` | 同じ案件で前の事象の後に、新しく後の操作を開始しないことを求める。既に発生した応答義務は消さない |
| `excludes Slow.work.cancel_ack, After.run.start` | 開始済みの操作を取り消した応答を前件にする。開始していない操作の取り消しは応答に数えない |
| `permitted Runner.run` | 操作の実行者に許可があるか検査する。実行者が匿名なら `U-SEM-0 executor identity is required` |
| `response delivery: case.received -> …` | 応答義務に `delivery` の名前を付ける。免除で参照できる |
| `response case.admitted -> one_of(end.done) by 2min after case.admitted otherwise unresolved` | 到着ではなく受入から期限を数える。前件と起算事象を揃える |
| `otherwise unresolved non_waivable` | この応答義務を免除できないようにする |
| `otherwise unresolved debtor q` | 義務を負う主体を記録する。実際に履行した人を判定する条件にはならない |
| `waive delivery on Officer.release.finish reason "registered exception" authorized_by G` | 宣言済みの権限に基づく事象で、その時点の未決の義務を免除する |
| `waive Controls.delivery on Officer.release.finish …` | 別の契約にある義務を `契約名.義務名` で指定する |

`waive` で義務名が未定義なら `E-NAME-0 undefined response reference`、免除できない義務なら
`E-CON-0 response is non_waivable` です。
後から発生した義務には免除が及びません。期限を過ぎた免除は、既に起きた期限違反を取り消しません。
`case.admitted` を書けるのは、`concurrent` か `queue` が `on_overflow wait_external` または
`on_overflow end NAME` の場合です。`on_overflow reject` だけでは `U-SYN-0 case.admitted requires external wait or overflow` になります。
例: [前件と起算](../../fixtures/lim-admitted-after/model.relca)、[名前付き義務の免除](../../fixtures/ctx-waive/model.relca)、
[免除しない義務](../../fixtures/ctx-nonwaivable/model.relca)、[債務者](../../fixtures/act-debtor/model.relca)、
[先行事象](../../fixtures/contract-requires/model.relca)、[開始の排除](../../fixtures/ctx-excludes/model.relca)。

`invariant ok == true` のように、事象を付けずに到達状態の条件を求めることもできます。
`always` や `forall` を式に足す形は `U-SYN-0 unsupported_invariant_expression` です。
例: [初期状態を含む検査](../../fixtures/sinv-initial/model.relca)。

### 異議の発生率と観測の範囲

`contest` の `respond_by` の後に `rates { … }` を付けられます。次の3欄をすべて書きます。

| 書き方 | 役割 |
|---|---|
| `occurrence probability 1 basis "assumed occurrence"` | 対象になった案件で異議が起票される確率と、その根拠 |
| `service_time basis "assumed service"` | 異議経路の所要時間の根拠。時間自体は経路に指定した操作から取る |
| `independence per_case_per_clause` | 異議の起票を案件と条項の組ごとに独立と仮定する |
| `horizon contest within 5d structural_only` | `cut` の欄。異議を見る範囲に、構造だけを扱うという注記を付ける。実行条件は変わらない |

`rates` に欄が欠けると `E-SYN-0`、重複すると `U-SEM-0` です。
`independence` に書ける値は `per_case_per_clause` だけです。ほかの値は `U-SEM-0`、空の `basis` は `E-CON-0` です。
起票確率0は `E-PROB-0 contest_probability_zero_use_cut_or_excludes` です。扱わない異議は範囲や排除の宣言で表します。
率を付けた条項に同じシナリオの `contest_not_filed` は使えません。
例: [異議条項の率](../../crates/relca-cli/tests/snapshots/inputs/p2c-q9-range-stop.relca)、
[Cut の異議範囲](../../fixtures/act-horizon/model.relca)。

### 構成ごとに規則を改める

| 書き方 | 役割 |
|---|---|
| `actor ApprovalRule { control policy; version "r1"; decides approval_required: bool = true }` | 規則の元の版と値を宣言する |
| `authority RuleChange { basis "amendment procedure"; grants cfo to change ApprovalRule; not_by [a1] }` | `cfo` に規則の変更を許す |
| `requires ApprovalRule.approval_required == false \|\| approval == pass else held` | 操作側で、選んだ構成の規則値を条件に使う |
| `amend ApprovalRule from "r1" to "r2" by cfo under RuleChange { approval_required = false }` | `configuration` 内の `process` の後に置き、その構成だけで版と値を変える |

`amend` は案件の途中に実行する操作ではありません。規則や権限の名前が未定義なら `E-NAME-0` です。
元の版が合わない、同じ版への変更、授権のない変更は通りません。変更の授権には `scope` や `expires` を付けられません。
制限付きの授権は `U-SEM-0 restricted rule-change Authority is unsupported` です。
同じ構成で一つの規則に複数の `amend` を書いたり、一度の変更で同じ欄へ二度代入したりすると `U-SEM-0` です。
[全文例](../../fixtures/rule-basic/model.relca)の `ToBeB` は承認に失敗しても登録できるため、
`check --configuration ToBeB` は `violated`、exit 1 です。

## 比較と指標

### 完了を求める検査を選ぶ

| 書き方 | 役割 |
|---|---|
| `verify recoverable end` | 到達した未完了状態ごとに、そこから完了へ進む道がある |
| `verify terminating end` | すべての経路が完了する |
| `verify almost_sure end` | 完了する確率が1である。非決定の選択に確率は補わない |

この3つはすべての終端をまとめて扱います。`end.NAME` を指定すると `U-SYN-0` です。
特定の終端への到達を求める `verify reachable end.NAME` とは区別してください。
計算を切断した経路を終了に数えません。
[全文例](../../fixtures/done-probability/model.relca)では、二つの確率枝がともに完了するため、三つとも `satisfied` です。
[容量0の対](../../fixtures/done-probability.broken-block-failure/model.relca)では完了の確率が9/10になり、三つとも `violated` です。

### 比較する範囲と目的を指定する

| 書き方 | 役割 |
|---|---|
| `compare [Check, Fast] under [Day, Spare] …` | 比べる構成と選択できるシナリオを列挙する。一度の比較で選ぶシナリオは一つ |
| `compare … cut C hard { cohort.WrongPerArrival <= 0.05 }` | Cut を指定し、必須の条件で候補を絞る |
| `objectives { maximize cohort.CorrectPerArrival }` | 流入あたり正しく登録できた期待件数が大きい案を優先する |
| `minimize metrics.overall.work` | 全体の占有作業時間を比べる |
| `maximize metrics.overall.named_people_work` | 名前付き実行者の占有作業時間を比べる |
| `minimize metrics.overall.peak_tasks` | 同時に存在する Task の最大数を比べる |
| `minimize metrics.overall.observed_cost_total` | 観測した総費用を比べる |
| `minimize metrics.overall.observed_cost_per_arrival` | 観測した総費用を流入件数で割った量を比べる |

`metrics.overall` は目的として使います。`hard` の条件に使うのは `cohort` と `mean` です。
分位点は `objectives` にだけ置けます。複数の目的を `weight` や `score` で足して順位を付ける構文は
`U-SYN-0` です。`under` にないシナリオを選ぶと `E-CONF-0`、比較で指定した Cut と構成の Cut が異なると
`E-CMP-0` です。どちらの場合も目的の評価には進みません。
単価が欠けた案を費用0とはしません。例: [シナリオと必須条件](../../fixtures/cmp7-under/model.relca)、
[全体指標](../../fixtures/cmp7-overall/model.relca)。正しい登録の比を最大化するなら、
`property case.Correct = reached(registered == true && correct == true)` と
`metric cohort.CorrectPerArrival = expected(count(case.Correct)) / arrivals` を定義してから目的に指定します。
