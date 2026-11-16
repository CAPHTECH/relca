# Relca DSL 早見表

根拠の印:

- **[実]** `examples/` か `fixtures/*/model.relca` に実在し、relca がその例で期待どおりの exit を返す形。
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

## Agency の導出の計算量 [実]

`relca check examples/evidence-basis.relca --agency-limit-scale 20` は satisfied / exit 0。
同じ指定を `simulate`、`explain` でも使える。DSL の宣言や `bounds` の項目ではない。
ASCII数字の1〜1000で省略は1、先頭の0は可。0、負数、符号、小数、空値、範囲外、overflow、重複は invalid / exit 3。
work/cellsだけを増やし、depth/paths/contexts、縮退の予算、性質の状態探索予算は変えない。
他のコマンドでは拒否する。実際の倍率と未導出の読み方は[利用ガイド](../../../docs/guide/results.md#agency-の導出の上限)を参照。

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
| AI 等の経路 | `via model "doc-extract@v3"`。確認用ページの `--assumptions` target は `<Actor>.via_model` | [実](../../../examples/invoice/phase1.relca) | 無し |
| 局所状態 | `state keys: set<CaseId>` | [実](../../../examples/invoice/phase1.relca) | 無し |

Capability の中 [実]:
`reads [...]`、`duration fixed 3min` か `duration {3min:0.5,5min:0.5}`、`result nondet { a, b }` か `outcomes kernel K` + `kernel_reads latent [...]`、
`writes x` / `writes [x,y]`、`finish { x = true }`、`commits`、`requires <観測条件> else <値>`、
`cancel cooperative ack_within 1min` / `cancel forbidden_after_start`、`atomic { if key in keys { result r } else { … } }`。
知識の記録への登録は `registers [Notes]`。後の `reads [Notes]` で由来を引き継ぐ。下の節を参照。

- 正の所要時間を持つ Capability の Actor に容量も実行者も無いと invalid(`fixtures/serial-time.broken-missing-capacity/`)。
- **判定として数えられる呼出しは、`case.id` 以外の引数を取るか、他のcallが書いた観測値や登録した記録への依存を持つもの**([構造の出力](../../../docs/guide/results.md#構造の出力)、`fixtures/agency-refusal/`)。sourceの`reads`だけでは判定に数えない。通常モデルではcommitの効果に関わるものが対象。
  `review()` のように引数なしだと、後ろの `if` で止めていても「止める判定なし」(W-RESP-2)になった [試]。申請を受け取る `review(claim)` にすると「全経路で止める」になった [試]。
  **引数は、その操作が実際にデータを受け取るときだけ書く。**警告を消すために引数を足さない。足すとモデルが業務と違うものになり、警告が示していたこと(判定が効果を止めていない)が隠れる。

## 知識を記録へ登録する [実]

```relca
actor Notes { }
actor Clerk { executors [p1]
  capability register() -> Ack {
    reads [Know]; registers [Notes]; duration fixed 1min; result nondet { ok }
  }
}
```

[全文例](../../../examples/record-provenance.relca)は check が satisfied / exit0、done は3分。
後の Reviewer.review が `reads [Notes]` で Know の由来を引き継ぐ。
q1 の責務は present_via_record、段 b の direct は空、missing は Know のまま。

- 登録先は registers の宣言で決まる。source・capacity・executors・Capability・state・latent・decides を持てない。
  複数先は registers [Notes,Notes2]。instance per_case なら案件ごと、印無しは cases1 のみ。
- 正常終了で最後の登録を上書きする。拒否・取消・未完了は登録しない。追記や案件間の共有は扱わない。
- 由来は登録者の reads と、引数・確定した観測変数・別の記録から導く。条件や潜在を観測へ格上げしない。
  記録自身は値も独立の根拠源も持たない。source record は独立の原本のままで、自動で読み替えない。
- 全候補に共通する源だけ via_records に出す。条件付きだけの源は indirect の上近似に残す。
  evidence_basis には must、direct 契約には source reads だけを使う。Notes 自体は要求できない。
- 正常終了の結果が複数なら registration_result_dependent。
  repeat 内は loop_feature_combination、応答喪失/再送/recovery/縮退/異議内や start 効果との登録の併用、
  印の無い記録で複数案件は registration_feature_combination。同じ記録の fork 兄弟での登録/読取りも拒否する。
- 除外した記録への登録/読取りは E-CUT-1。
  check/explain/simulate/mutate で扱い、compare/diff/sensitivity は registration_unsupported_command。
  conform は登録の由来を照合せず unchecked に registration_provenance を残す。
- 記録の由来は同じ版の証明ではない。呼出し結果の版の一致は same_version で別に要求する。

[構文と書き直し方](../../../docs/guide/dsl.md#知識を記録へ登録する)、[結果の欄](results.md#記録経由の到達)を参照。

## 根拠源を求める契約 [実]

```relca
contract Basis { evidence_basis Clerk.match_po [Invoice,PurchaseOrder] }
```

[全文例](../../../examples/evidence-basis.relca)は check が satisfied / exit 0。
`enter` の読む Invoice は引数 e を通じて、PurchaseOrder は match_po 自身の reads から届く。
2操作が各1分で終わり、対象が正常に終わるのは時刻2分。
[上流の読取りを外した対](../../../examples/evidence-basis-missing.relca)は violated / exit 1。

- `evidence_basis direct Clerk.match_po [Invoice]` に置き換えると、引数の源を数えないため violated。
- source Actor のみを要求でき、複数は全部を求める。空・重複・state・観測・潜在は不可。
- 引数が受け取るのは正常終了して返った値の根拠。拒否のelse値・拒否応答の採用には、生産者のreadsを加えない。
- 観測変数の reads は、同じ/外側のblockで先行し、必ず正常終了して最後の値を書く操作だけを数える。必ず上書きされる古い源は残さない。
  if/forkの枝、requiresの拒否、任意の再送試行などのmay辺だけから届く要求は
  `U-SEM-0 evidence_basis_conditional_snapshot`。別の要求源の確定欠落は violated のまま。
- 条件式、requires、wait until、kernel_reads latent、given はデータの読取りに数えない。
  引数の源は実際の呼出し引数から計算するので、readsに引数名を重ねても増えない。
- 正常終了の無い対象は vacuous。計算切断は inconclusive。異議経路の対象はその性質だけ evidence_basis_contest_route でunsupportedとし、他の性質は残す。契約の追加を Agency や責務の補完に使わない。
- Cutで除外したsourceの要求は、既存の範囲外Actor参照と同じE-CUT-1で静的に拒否する。
- repeat は `loop_feature_combination`、縮退/recovery は `evidence_basis_feature_combination`、
  compare/diff/sensitivity は `evidence_basis_unsupported_command` で拒否する。
  conform は読取りを観測しないため U-LOG-1 の unsupported。

`mutate --kinds evidence_missing` はこの行も検査に使い、全文例の2削除はどちらもdetected。
original_access_removed が使うのは direct の行だけ。未検出を0件にするため全readsを転記しない。
詳細な[構文と診断](../../../docs/guide/dsl.md#根拠源を求める契約)、
[結果と証跡](results.md#根拠源の要求と証跡)を参照する。

## 確かめた版と使う版 [実]

```relca
contract V { same_version Checker.check(d), Approver.approve(d) }
```

dは各Capabilityの引数名で、processの結果名ではない。
後の操作が正常終了するたび、開始前に同じ案件で最後に正常終了した先の操作の引数と版を比べる。
先の確認が無くても違反。pass/failの結果は読まないので、使う結果の条件はif/requiresで書く。
版は呼出しの実行が作った結果の同一性であり、enumの等しさではない。変換は新しい版、合流はyield元の版。
requiresで拒否した確認は数えず、拒否した後の操作は発火しない。正常終了なしはvacuous、切断はinconclusive。
version_checkとatomicのreject、異常終了・取消・timeout・応答待ちも数えず、最後に正常終了した確認を維持する。通常のenum値の名前だけでは拒否と判定しない。

[正の全文](../../../examples/same-version.relca)はcheckがsatisfied/exit0、done3分・rejected2分。
[取り違えた対](../../../examples/same-version-reused.relca)はviolated/exit1、最初の違反は承認終了4分。
値は両方draftだが、確認したdと承認するd_dupは別の版。正の例のmutate --kinds summary_reusedはこの要求で1件detected。

同じ型名が必要（E-TYPE-0 same_version_type_mismatch）。未知の操作/引数はE-NAME-0、
同じ操作の同じ引数を両側に書けばE-CON-0 same_version_same_target。同じ操作の別引数は受理する。
applies_toは他の契約と同じ。check/explainに対応、simulateは標本中の違反だけ。
合流・join allは利用でき、観測変数で渡す値の版は対象外。
対象引数のcase.idと観測case変数（同名の呼出し結果やその合流を含む）はsame_version_feature_combinationで拒否する。
定数やprocess入力を直接渡す構文も無い。対象でない別の引数のcase.idは、この契約を妨げない。
repeatはloop_feature_combination、join any・応答喪失/再送/recovery/縮退との併用はsame_version_feature_combination。
異議経路の対象はその性質だけsame_version_contest_route。
compare/diff/sensitivityはsame_version_unsupported_command、conformはU-LOG-1。
合流/繰り返しを含むmutateの範囲も広げない。
[構文の詳細](../../../docs/guide/dsl.md#確かめた版と使う版を揃える)、[三つの版と証跡](results.md#値の版と証跡)を参照。

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
| `ready = if r == fail { f = Office.fix(d); yield f } else { yield d }` | [試]。全入力は[合流の例](../../../examples/merge-if/model.relca) |
| `end NAME` | [実] |
| `wait 5min` | [実](../../../fixtures/) |
| `repeat NAME at_most N [gap fixed DURATION] { ... } on_limit { ... }`。角括弧は省略可能部分で、入力には書かない | [試]。実行した全文は[繰り返しの例](../../../docs/guide/dsl.md#繰り返し) |
| 条件に潜在変数(`Invoice.difficulty` 等)を書く | **E-POL-LATENT で invalid** [実](../../../examples/invoice/to-be-c.relca) |

## 分岐した値の合流 [試]

値を返す `if` の各枝の末尾を `yield 結果名` または `end 終端名` にする。
`else` は必須で、両枝を `end` にしてはならない。入れ子も使える。
全 `yield` 元は同じ型名で、枝末尾で確実に作成済みの呼出し結果か、先行する合流値に限る。
後続の call 引数、process の `if`、後の `yield` から読める。
wait until、guard、権限のscope、契約・性質・指標の式、kernelのgiven、通常の代入や数値の宣言は読取り位置ではない。
静的に拒否し、既存codeのmessageに`merge_read_position`を付ける。
合流はTask、時間、資源、乱数drawを増やさず、枝の書込みを保持する。

通常のbindingも合流名もprocess内で重複できない。枝内の結果を後で直接読むには、
続行するすべての経路で利用可能でなければならない。同名を両枝で宣言しても合流にはならない。
合流は観測case欄への書込みではないので、その欄と同名にもできない。
`yield = Office.write()` のように名前として使う形は変わらない。

check・explain・simulate、指標・品質、複数案件に接続する。compare・diff・mutate・sensitivityは
`U-SEM-0 merge_feature_combination`、conformは`U-LOG-1`で閉じる。
契約・requires・joinのwhere・property・metricでの合流値の参照、contest・quantileとの併用、
recovery handlerや縮退経路内の合流も未対応。
repeatのbody・on_limit内は`loop_feature_combination`。
repeatや応答喪失・再送・recoveryの公開結果をyieldする形も未対応である。
合流前に固定した値を、その後のrepeatのcall引数に渡すことはできる。
観測状態や定数のyieldは`merge_source_unsupported`、潜在は`E-POL-LATENT`。

枝の閉鎖の違反は`E-CON-0`の`merge_fallthrough`、`merge_unreachable_after_yield`、
`merge_no_continuing_branch`、`merge_yield_outside`。型名の不一致は`E-TYPE-0 merge_type_mismatch`。
未定義の元は`E-NAME-0`、取消されうる元は従来の可用性の`U-SEM-0`で拒否する。
[利用ガイド](../../../docs/guide/dsl.md#分岐した値を先へ渡す)に実行例と境界をまとめている。

## 上限つきの繰り返し [試]

差し戻しと再提出、状態を待つ再確認を記述できる。`N` は初回を含む1〜32回、
`on_limit` は必須。body の全経路を `again`（次の回、最後なら上限後へ）、
`leave`（repeat の後へ）、`end NAME`（案件を終了）で閉じる。
`on_limit` は `leave` か `end` で閉じ、`again` は書けない。
同じ block の `again` / `leave` の後ろに文を置かない。
固定 gap は次の回の前だけで、初回と最後の `again` の後には待たない。
正の最短操作時間、固定 wait、gap のいずれも無く0分で `again` に戻れる経路は拒否される。

`NAME.round` は1始まりで、その body 内の `if`、呼び出す操作の `requires`、kernel の
`given` にだけ使える。宣言が外でも、値を使う呼出しは body 内に置く。
後続、別の繰り返し、`on_limit` からは読めない。観測と潜在は回をまたいで保持する。

body の fork/join、条件待ち、縮退経路、response_loss/retry/recovery、
入れ子、`over`、再到着、無限反復は未対応。繰り返しモデルの
compare/diff/sensitivity/mutate/conform、およびcontest宣言・quantile指標との併用も閉じる。
短い全入力と、code/reasonの一覧は[利用ガイド](../../../docs/guide/dsl.md#利用できる範囲と診断)を参照。
主なreasonは次のとおり。messageに含まれる識別語で、独立のJSON欄とは限らない。

| code | reason | 主な対象 |
|---|---|---|
| E-CON-0 | loop_fallthrough / zero_time_iteration | 閉じない経路 / 0分でのagain |
| E-CON-0 | loop_again_in_limit / loop_unreachable_after_control | 上限後のagain / 制御文の後続 |
| U-SEM-0 | loop_expansion_limit / loop_round_out_of_scope | 33回以上 / roundの範囲外参照 |
| U-SEM-0 | loop_stage_unsupported / loop_feature_combination | over / 未対応の入れ子・組合せ・分析 |

回数0はE-CON-0、必須部分の欠落はE-SYN-0、名前の衝突はE-NAME-0。
conformの照合はU-LOG-1で拒否する。
本文の2つの全文例はcheckとexplainで実行して確認した形であり、あらゆる組合せの対応を意味しない。

確認用ページは、名前・`at_most`・gap、body と `on_limit`、`again` の戻り矢印、
`leave` の出口、`end` を表と流れ図に出す。入れ子や `over` などの拒否された形は
「Relca が受け付けない形」と表示し、正常な loop として描かない。
`--assumptions` の target は `<NAME>.at_most`（繰り返しの上限回数）、
`<NAME>.gap`（明記した繰り返しの間隔）。例は `rework.at_most`、`poll.gap`。
省略した gap の0分は既定として表示し、宣言の target にはしない。
経由モデルを含む target の一覧は[結果の読み方](results.md#確認用ページの-assumptions)を参照。

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
- **結果不明の後の段は書けない。**後ろに呼出しを書くことはできるが、最後の応答が unknown の経路では実行されない。`[unknown, unknown]` にすると後ろの呼出しは `W-UNREACH-0 … never_started`、案件は `receipt_unknown` で止まり、`receipt_resolution` が inconclusive・exit 2 [試]。人が照会して進める・返金する段は今は書けないので、その段の名前で例えば `defer ReceiptFollowup` とし、reason と loses を付ける。Actor や操作に解決しない opaque な defer による全体撤回と W-CUT-2 は意図した結果として報告する。無関係な操作の部分撤回で代用しない(SKILL.md「再送を含むモデル」)。
- 再送を含むモデルは compare・diff・待ちの分解・分位点・contest と組めない(U-SEM-0 `retry_feature_combination`)。`references/unsupported.md`。
- fork の枝そのものへの retry、`version_check` 付き Capability への retry は U-SEM-0 `retry_feature_combination`(`response_loss_inputs.rs` の `fork-retry`・`version-retry`)[U]。

## 構成変更の理由

構成内の `intent { ... }` は行き先の既定。相手別の `intent from A to B { ... }` は最上位に書く。
diff A→B は相手別があればそれだけ、無ければ B の既定を使う。空の相手別も既定を除外し、逆向きには使わない。

```relca
intent from A to B {
  replace Clerk.check with Bot.check reason "照合を自動化" loses "人の目視"
}
intent from C to B {
  replace Robot.check with Bot.check reason "別製品へ移行" loses "Robot の保守契約"
}
```

`replace` は能力の完全な削除・追加だけに当たる。Actor の一括差し替えや、元を呼び続ける部分差し替えには使わない。
能力の削除・追加に加え、それだけで説明できる Actor・権限・容量/実行者・原本の読み手・生産者の差分を説明する。
追加の読み手などで説明しきれない行、契約範囲・意味・時間・異議・規則・資格/性能/縮退は自動で覆わない。
個別の理由や注記が同じ行に当たれば個別を優先する。複数能力から1能力への差し替えは同時に扱う。

個別の形は `add Actor[.capability] reason "..."`、`retire Actor[.capability] reason "..." loses "..."`、
`change KIND TARGET reason "..." [loses "..."]`。
保持は `preserve ContractName` または `preserve "性質id"`、特定行なら `on KIND TARGET` を付ける。
上書きは `override [KIND] TARGET by SUBJECT [reason "..."]`、保守は `maintain [KIND] TARGET by PERSON [reason "..."]`。
`by` の後ろは単一の主体だけ。通常経路から退ける `retire A.c from normal_path keep fallback` は構成内だけで、相手別では U-SYN-0。
ここで角括弧は省略可能部分の説明であり、そのまま入力する記号ではない。

すべての宣言を静的検査するが、差分との照合は diff だけ。compare は比較する構成の空の宣言でも `unchecked` に `intent` を残す。
[全文例](../../../examples/capability-replacement.relca)の A/B/C は3/2/4分。A→B と C→B の diff は各10行を説明し exit 0。
構文・診断の詳しい対応は[利用ガイド](../../../docs/guide/dsl.md#構成変更の理由を書く)を参照。

## Cut

```relca
cut InvoiceCut {
  question "One receipt of one invoice"
  include [Invoice,PurchaseOrder,ReceiptRecord,Clerk,Extractor,Approver,Ledger,Vendor]
  exclude Payment reason "separate payment domain" loses "payment errors"
  exclude VendorMaster reason "master data out of scope" loses "master data correlation"
  defer Clerk.recheck reason "recheck route is deferred; process indicators are withdrawn" loses "process time, cost and quality estimates"
  externality [Vendor.dispute_effort]
  reentry [contest Vendor -> Clerk.recheck, review cut quarterly by cfo]
  horizon receipt..resolution within 60min
}
```

[実]。Cut を書かなくても check は exit 0 を返し、`cut: "undeclared"` と W-CUT-0 だけが残る [試]。**exit 0 でも範囲は不明**なので、必ず書く。
include にも exclude にも無い Actor はエラー(`fixtures/cut-declared.broken-missing-member/`)。
この Cut の例は、請求書の as-is モデルの Cut を上の block 全体で置き換えて実行したもの。
そのモデルでは `contest Vendor on end.returned route Clerk.recheck` があるため、
`cut.defer_effects.process` は true、Clerk.recheck の withdraws は process、
because は `[{"kind":"contest_route","capability":"Clerk.recheck"}]`。check は exit 0、W-CUT-2 は0行 [試]。
reentry は再入経路の宣言であり、それだけで route を実行する指定ではない。全体撤回の根拠は上の contest 契約。
宣言済み Actor 名か Actor.capability を対象にするのは、実際にモデルに持つ層の指標を保留するときだけ。
モデルにできない段はその段の名前で defer する。Actor や操作に解決しない名前は opaque となり、
対象 report の時間・費用・品質などを全体で撤回して W-CUT-2 が出る。これは欠けた段を含まない値を使わないための意図した扱い。
無関係な宣言済み操作の defer に変えて値を出させない。
正しく解決しても、本文の字句上の呼出し（死枝を含む）か起動しうる異議の route なら全体へ広がる。
`cut.defer_effects` の process・各行の because・scope を読む。指標が無い report と conform にはこの欄・警告は出ない。
保留は実行を削除する指定ではない。実際にモデル外へ置く範囲は、理由と失うものも合わせて記録する。

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

JSON の性質 id は `contract <名前> #<何番目の節> <種類>`(例 `contract InvoiceControls #4 separation`)。
`verify probability …` は `probability reached #<何番目>` になる [実]。

### 呼出しごとの根拠源と責務 [実]

`reads [Source]` はその操作自身の直接の読取り。引数や観測変数を受け取ることとは分ける。
`responsible a1 for Approver.approve` の出力は、人の案件全体と、本人の対象呼出しの段を併記する。
段の表を自己申告する構文は無い。O の `agency.viewpoints.observation.stages` と
責務の `observation.stages` を読む。

[全文例](../../../examples/stage-observation.relca)は e で Invoice を読み、v では e だけを受け取る。
check は satisfied / exit0、posted 3分・returned 2分。行全体が present でも段 v は missing Invoice、
W-RESP-3 が1件出る。`confirm` に `reads [Invoice];` を足すと段の欠落は無くなるが、独立根拠は増えない。
必須条件にするなら `evidence_basis direct Clerk.confirm [Invoice]`。生成物だけの例では violated / exit1。
構造の警告から実際の閲覧や正当な授権を推測しない。
[欄と未導出の理由](../../../docs/guide/results.md#呼出し位置ごとの観測)を参照。

### 契約を構成に限定する [実]

省略時はすべての構成に適用する。名前付き契約にだけ、適用する構成名の並びと理由を書ける。

```relca
contract Controls applies_to [New] reason "Oldには承認画面がない" {
  authority_basis Screen.approve Mgr
}
```

[全文例](../../../examples/configuration-contracts.relca)は Old の check が1分、New が2分で、どちらも satisfied / exit 0。
Old の契約は満たしたのでなく未検査で、`contract_scope` と `unchecked` に残る。
範囲内で活性化しなければ vacuous / exit 5 のまま。到達しない枝を自動で範囲外にしない。
適用先の未知名・空・重複、configuration の無い模型、理由の欠落は拒否する。
匿名契約への指定と、contest を含む契約への指定も拒否する。
waive の範囲は参照する response の範囲の部分集合にする。
verify は全域のままで、適用する要求が0件なら unsupported / exit 4。
対象を手順・異議経路・復旧などから呼ぶ構成で契約を外すと W-CONF-2（state invariant も警告）。
空の reason は Cut の除外理由と同様に構文上は受理するが、理由を記入して使う。
全構成の列挙は受理し、警告しない。
詳しい構文・診断・保持宣言との関係は[利用ガイド](../../../docs/guide/dsl.md#契約を適用する構成を選ぶ)を参照。

## 宣言させないもの

Agency と O/F/P/J/U/E/L は書かない。relca が構成から導いて JSON の `agency` に出す。Capability の操作契約とは区別する。
