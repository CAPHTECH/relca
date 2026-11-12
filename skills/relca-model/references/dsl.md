# Relca DSL 早見表

根拠の印:

- **[実]** `examples/invoice/*.relca` か `fixtures/*/model.relca` に実在し、relca がその例で期待どおりの exit を返す形。
- **[試]** `references/minimal.relca` か一時的なモデルで 1 回だけ回して確かめた形。1 回の観測であり、一般化しない。一時的なモデルの全文はこの資料には含めず、再実行できる入力を各節で案内する。
- **[コ]** `crates/relca-core/src/syntax.rs` / `syntax/lexer.rs` を読んで確かめた形。回してはいない。
- **[U]** 未確認。機能名だけから構文を推測せず、書く前に実装と fixture を照合して実行する。

構文の正本は syntax.rs。[利用ガイド](../../../docs/guide/dsl.md) と実在する入力例を併読する。
範囲外の構文は `unsupported`(U-SYN-0)、範囲内で意味が通らないものは `invalid` になる。

## 字句 [コ]

| 規則 | 内容 |
|---|---|
| 区切り | `;` と改行は同じ区切り。ブロックは `{ }` |
| コメント | **書けない。**`//` と `/* */` は unsupported(U-SYN-0)。`//` は [試] でも exit 4 |
| 非 ASCII | 文字列 `"…"` の外では unsupported。文字列の中の日本語は [試] で通った |
| 文字列 | 改行を含む文字列は invalid、`\` のエスケープは unsupported |
| 時間 | 整数 + 単位。`min` は [実]。`h`・`d` も fixtures に実在 [実](`window 2h`、`respond_by 5d`)。小数の時間は unsupported [コ] |
| 確率 | `0.99` のような小数 [実] |

## 最上位の項目

`model NAME version "0.2" { … }` の中に置く。syntax.rs の分岐にある語 [コ]:
`type kernel performance actor process bounds verify cut configuration contract people permit authority responsible observed latent metrics metric property simulation scenario arrivals policy queue concurrent resource calendar person_calendar case_priority`。

| 項目 | 例 | 印 |
|---|---|---|
| 型 | `type Verdict = enum { pass, fail }` | [実] |
| record 型 | `type Entry = record { confidence: enum { high, low } }` | [実](../../../examples/invoice/phase1.relca) |
| 人 | `people [c1,c2,a1,cfo]` | [実] |
| 案件の観測 | `observed case { po, receipt: optional<Verdict>; registered: bool = false }` | [実] |
| 案件の潜在(真値) | `latent case { extraction_correct: bool = false }` | [実] |
| 条件付き確率 | `kernel po_match given [extraction_correct, Invoice.difficulty] { true, routine -> { 1: pass } … }` | [実] |
| 上限 | `bounds { cases 1; live_tasks 2; clock 60min }`。**3 つとも必須**(欠けると U-SEM-0、exit 4 [試]) | [実] |
| 検査 | `verify safety no_resource_overcommit` / `verify reachable end.paid` / `verify probability reached <条件> <= 0.001` | [実] |
| 指標 | `metrics { window 60min; currency "JPY" }`。単価は `rate Office per_minute 100; rate Office.short per_start 2` | [実] |
| 待ちの分解 | `metrics { …; breakdown waits }` | [実] |
| 品質 | `property case.Posted = reached(end.posted)` / `metric cohort.X = expected(count(case.Y)) / arrivals` / `metric quantile.P95 = quantile(0.95, case.Posted)` | [実](../../../examples/invoice/phase1.relca) |
| 標本推定 | `simulation replications 1000 seed 0 confidence 0.95`。無いと simulate は E-SIM-0(exit 3 [試]) | [実] |
| 流入とシナリオ | `scenario BusyDay { arrivals fixed_every 4min during [0min,480min); queue capacity 200 on_overflow reject; bounds {…}; horizon 960min; simulation … }` | [実](../../../examples/invoice/busyday.relca) |
| 固定方策 | `policy observable fifo tie_by case_id assign first_idle`。業務の実際のやり方だという根拠があり、仮定一覧に書くときだけ使う。simulate を通すために置かない | [実](../../../fixtures/) |
| 資源 | `resource R capacity 1` と Capability の `uses R: 1` | [実](../../../fixtures/) |
| 構成 | `configuration ToBeA { cut InvoiceCut; process ToBeAFlow; intent { add Extractor reason "…" … } }` | [実] |
| 応答喪失・再送・idempotency・Receipt unknown | 下の「応答喪失・再送」の節 | [試] / [U] |
| カレンダー・優先度・技能と縮退・異議の発生率 | 限定した形を扱う。この早見表では構文を未確認 | [U] |

## Actor

```relca
actor Clerk { executors [c1,c2]
  capability match_po(e:Entry) -> Verdict { reads [PurchaseOrder]; duration fixed 3min; outcomes kernel po_match; kernel_reads latent [extraction_correct,Invoice.difficulty]; writes po }
}
actor Invoice { source document; latent difficulty: Difficulty ~ { routine: 0.8, difficult: 0.2 } }
actor Vendor { control party; standing over [end.returned] basis "invoice claim" }
actor Ledger { capacity 1
  capability post(p:Verdict) -> Ack { commits; duration fixed 1min; result nondet { committed }; writes registered; finish { registered = true } }
}
```

| 欄 | 例 | 印 | 書かないとき |
|---|---|---|---|
| 実行者 | `executors [c1,c2]` | [実] | 匿名。職務分離・権限の検査は unsupported(JSON の assumptions に出る) |
| 容量 | `capacity 2` | [実] | 実行者を宣言していればその数(JSON の assumptions に `capacity = count(executors)`)。容量も実行者も無く、正の所要時間の Capability を持てば invalid(`fixtures/serial-time.broken-missing-capacity/`) |
| 原本 | `source document` / `source record` / `source tacit` | [実] | 無し |
| 制御 | `control party` / `control policy` | [実] | 呼ばれ方から既定(policy / environment)。JSON の actors で `status: default` |
| 粒度 | `grain team` 等 | [実] | **意味を持たない。**入れ替えても結論は変わらない(`fixtures/act-grain/` と `.broken-swapped`)。種別名に意味を込めない |
| AI 等の経路 | `via model "doc-extract@v3"` | [実](../../../examples/invoice/phase1.relca) | 無し |
| 局所状態 | `state keys: set<CaseId>` | [実](../../../examples/invoice/phase1.relca) | 無し |

Capability の中 [実]:
`reads [...]`、`duration fixed 3min` か `duration {3min:0.5,5min:0.5}`、`result nondet { a, b }` か `outcomes kernel K` + `kernel_reads latent [...]`、
`writes x` / `writes [x,y]`、`finish { x = true }`、`commits`、`requires <観測条件> else <値>`、
`cancel cooperative ack_within 1min` / `cancel forbidden_after_start`、`atomic { if key in keys { result r } else { … } }`。

- 正の所要時間を持つ Capability の Actor に容量も実行者も無いと invalid(`fixtures/serial-time.broken-missing-capacity/`)。
- **判定として数えられる呼出しは、`case.id` 以外の引数を取るか、追加の読取りを持つものだけ**([構造の出力](../../../docs/guide/results.md#構造の出力)、`fixtures/agency-refusal/`)。`case.id` だけで追加の読取りも無い呼出しは判定に数えない。
  `review()` のように引数なしだと、後ろの `if` で止めていても「止める判定なし」(W-RESP-2)になった [試]。申請を受け取る `review(claim)` にすると「全経路で止める」になった [試]。
  **引数は、その操作が実際にデータを受け取るときだけ書く。**警告を消すために引数を足さない。足すとモデルが業務と違うものになり、警告が示していたこと(判定が効果を止めていない)が隠れる。

## process

```relca
process Flow {
  claim = Applicant.submit()
  fork checks { poV = Clerk.match_po(entry); receiptV = Clerk.match_receipt(entry) }
  join all checks
  if poV == pass && receiptV == pass {} else { end returned }
  posted = Ledger.post(case.id, poV)
  if posted == committed { end posted } else { end exception }
}
```

| 文 | 印 |
|---|---|
| `x = Actor.cap(args)`(呼出し。終了まで待つ) | [実] |
| `fork NAME { 呼出し; … }` + `join all NAME` | [実] |
| `join any NAME where result == V tie by [a, b] remaining cancel_then_drain if none -> END`(複数行に分けてよい) | [実](../../../fixtures/coop-cancel/) |
| `if <条件> { … } else { … }`。`&&` と `||` の混在は括弧が要る | [実] / [コ] |
| `end NAME` | [実] |
| `wait 5min` | [実](../../../fixtures/) |
| 条件に潜在変数(`Invoice.difficulty` 等)を書く | **E-POL-LATENT で invalid** [実](../../../examples/invoice/to-be-c.relca) |

## 応答喪失・再送・idempotency・Receipt unknown

構文と診断は `crates/relca-core/src/syntax.rs` と `lower/response_loss.rs` で定義する。例は `crates/relca-core/tests/support/response_loss_inputs.rs` の `service()`・`correlated()` にもある。
下の [試] は relca 0.0.0(`target/debug/relca`、2026-11-11 の build)で各 1 回 `check` を回した観測。

### Capability 側: 再送される外部効果の三つの形

再送(`retry … when receipt unknown`)する呼出し先の Capability は、次のどれかで書く。**どれを選ぶかは業務の事実(取引先が重複を拒むか)で決める。**検査を通すために選ばない。

| 形 | 書き方 | 再送したときの結果 | 印 |
|---|---|---|---|
| G: 同じキーの重複拒否 | `idempotency key k` + 所有 Actor の `state keys: set<CaseId>` + `atomic { if k in keys { result rejected_duplicate } else { keys += k; …; result charged } }` | at_most_once を判定する。2 回目は `rejected_duplicate` を返し、commit しない | [試] |
| modeled: 重複拒否なし(非冪等) | `effect_scope modeled` + `result nondet { … }` + `finish { …; count += 1 }` | 二重 commit が反例として出る(at_most_once violated、exit 1) | [試] |
| R: 機構を書かない | 上のどちらも無い `result nondet { … }; finish { … }` | **U-SEM-0 `response_loss_resend_mechanism_unmodeled`、exit 4**。violated にはならない。非冪等を問いたいなら modeled を書く | [試] |

G の例(注文・決済の試行モデルの案 A。[試] で violated・exit 1。違反は at_most_once 以外の契約で、二重課金の 2 性質は satisfied):

```relca
actor Psp { capacity 1; state keys: set<CaseId>
  capability charge_keyed(key: CaseId) -> Pay {
    idempotency key key
    commits; writes [keys, charged_flag, charges]; duration fixed 1min
    atomic { if key in keys { result rejected_duplicate } else { keys += key; charged_flag = true; charges += 1; result charged } }
    cancel forbidden_after_start
  }
}
```

modeled の例(同じ試行の案 B。at_most_once と `verify probability reached charges == 2 <= 0` が violated [試]):

```relca
  capability charge_plain(key: CaseId) -> Pay {
    effect_scope modeled
    commits; writes [charged_flag, charges]; duration fixed 1min
    result nondet { charged }; finish { charged_flag = true; charges += 1 }
    cancel forbidden_after_start
  }
```

- `idempotency key` は宣言だけでは保証を作らない。G の atomic の形が揃わないと U-SEM-0 `idempotency_mechanism_unmodeled`(`crates/relca-core/tests/support/response_loss_inputs.rs` の `idempotency-without-atomic`)[U]。
- G の認めるのは「このキーに対するモデル内 commit の一回性」だけ。取引先の実際の仕様(キーの保持期間、timeout 後の非同期処理)は response_analysis の unchecked に残る。
- G で 2 回目が `rejected_duplicate` のとき、その値がそのまま process に返る。元の結果(課金済みか)を受け取ったことにはならない([複数案とその他の出力](../../../docs/guide/results.md#複数案とその他の出力))。後段の分岐は自分で書く。

### 呼出し側: suffix

```relca
pay = Psp.charge_keyed(case.id) response_loss nondet retry max_attempts 2 when receipt unknown backoff fixed 0min exhausted end payment_failed
```

`response_loss <配信>` と `retry …` の順で、各高々 1 つ。1 行に書いた形は [試]。複数行に分けた形はテスト入力(`correlated()`)にあるが回していない [U]。

| 部分 | 形 | 印 |
|---|---|---|
| 配信 `nondet` | 喪失するかを非決定(min/max)で全部展開 | [試] |
| 配信 `sequence [unknown, delivered]` | 試行ごとの受信を固定する(故障注入・手計算用)。長さは max_attempts と同じ。違えば E-CON-0(`response_loss_inputs.rs` の `sequence-short`) | [試](長さ違いは [U]) |
| 配信 `probability 0.1 basis "…"` | 喪失確率と根拠。根拠の無い確率を置かない(一線)。basis 欠落は E-CON-0(同じ入力の `missing-basis`) | [試](欠落は [U]) |
| 配信 `none` | 喪失させない。新しい欄(response_receipts 等)は出る | [試] |
| 配信の省略(`retry` だけ) | `nondet` と同じ扱い。assumptions の `response.<binding>` に `loss=Nondet` と出た | [試] |
| `retry max_attempts N` | **初回を含む総試行数**。1〜32。33 は U-SEM-0 `retry_expansion_limit`(同じ入力の `max-thirty-three`) | [試](N=2 のみ) |
| `when receipt unknown` | 応答が失われたら再送 | [試] |
| `when result [fail]` | 既知の失敗で再試行。喪失を意図しないなら `response_loss none` を明記(同じ入力の `correlated()` と `NONE`) | [試](`response_loss none` と組んで satisfied・exit 0) |
| `when receipt unknown or result [...]` | 両方 | [U] |
| `backoff fixed 0min` | 必須。欠けると E-SYN-0(同じ入力の `missing-backoff`) | [試](0min と 3min) |
| `exhausted end E` | **必須**。欠けると E-SYN-0「expected exhausted」 | [試] |

- `exhausted end` へ進むのは、最後の試行が**既知の失敗**のときだけ。最後まで unknown なら exhausted End にも後続の End にも進まず、Receipt unknown で止まる([複数案とその他の出力](../../../docs/guide/results.md#複数案とその他の出力))。`when receipt unknown` だけなら exhausted End に到達せず、W-UNREACH-2 が出ることがある [試](試行の途中版)。
- **process で `unknown` を分岐に書けない。**`if ack == unknown { … }` は E-TYPE-0「unknown is not a member of Ack」、exit 3 [試]。unknown は戻り値の enum 値ではない。
- **結果不明の後の段は書けない。**後ろに呼出しを書くことはできるが、最後の応答が unknown の経路では実行されない。`[unknown, unknown]` にすると後ろの呼出しは `W-UNREACH-0 … never_started`、案件は `receipt_unknown` で止まり、`receipt_resolution` が inconclusive・exit 2 [試]。人が照会して進める・返金する段は今は書けないので、Cut の `defer` に `loses` を付けて外す(SKILL.md「再送を含むモデル」)。
- 再送を含むモデルは compare・diff・待ちの分解・分位点・contest と組めない(U-SEM-0 `retry_feature_combination`)。`references/unsupported.md`。
- fork の枝そのものへの retry、`version_check` 付き Capability への retry は U-SEM-0 `retry_feature_combination`(`response_loss_inputs.rs` の `fork-retry`・`version-retry`)[U]。

## Cut

```relca
cut InvoiceCut {
  question "One receipt of one invoice"
  include [Invoice,PurchaseOrder,Clerk,Approver,Ledger,Vendor]
  exclude Payment reason "separate payment domain" loses "payment errors"
  defer Vendor.contest_rate reason "structure only" loses "frequency and effort"
  externality [Vendor.dispute_effort]
  reentry [contest Vendor -> Clerk.recheck, review cut quarterly by cfo]
  horizon receipt..resolution within 60min
}
```

[実]。Cut を書かなくても check は exit 0 を返し、`cut: "undeclared"` と W-CUT-0 だけが残る [試]。**exit 0 でも範囲は不明**なので、必ず書く。
include にも exclude にも無い Actor はエラー(`fixtures/cut-declared.broken-missing-member/`)。

## 契約・権限・責務 [実]

```relca
authority ApprovalGrant { basis "approval table"; grants a1 to Approver.approve; issued_by cfo }
permit a1 to Approver.approve
responsible a1 for Approver.approve
contract InvoiceControls {
  invariant Ledger.post.commit -> approval == pass
  at_most_once Ledger.post.commit per case.id
  authority_basis Approver.approve ApprovalGrant
  separation Clerk.enter.finish, Approver.approve.finish
  response case.received -> one_of(end.posted,end.returned) by 60min after case.received otherwise unresolved
  contest Vendor on end.returned route Clerk.recheck reversible_by [a1] respond_by 7200min
}
```

JSON の性質 id は `contract <名前> #<何番目の節> <種類>`(例 `contract InvoiceControls #5 separation`)。
`verify probability …` は `probability reached #<何番目>` になる [実]。

## 宣言させないもの

Agency と O/F/P/J/U/E/L は書かない。relca が構成から導いて JSON の `agency` に出す。Capability の操作契約とは区別する。
