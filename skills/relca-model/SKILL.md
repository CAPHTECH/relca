---
name: relca-model
description: 業務の説明から Relca のモデル(.relca)を起こし、relca の check・explain・compare・simulate・mutate・diff・verify で検査し、結果と確かめていない範囲を人間が確認できる HTML ページにして渡すときに使う。Relca 本体の実装や受入 fixture の作成には使わない。
---

# relca-model

Relca は業務フローを DSL で書き、検査・比較・標本推定する CLI。構文と範囲は `docs/guide/`、境界の規約は `AGENTS.md` を参照する。
このスキルで一番大事なのは、判定の前に **モデルが実際の業務を正しく写しているか** を業務の人が確かめられること。AI が書いたモデルを誰も照合していなければ、satisfied に意味はない。

参照:

- `references/dsl.md` DSL 早見表(根拠の印つき。[U] は未確認)
- `references/results.md` verdict・exit・JSON の欄の読み方
- `references/estimate.md` 実績ログから所要時間・確率の候補を出す手順と確認
- `references/unsupported.md` よくある unsupported / invalid とその理由
- `references/minimal.relca` 最小の例(check で satisfied・exit 0 を 1 回確認。承認の分岐を消すと violated・exit 1)
- `scripts/review_page.py` 確認用ページの生成、`scripts/test_review_page.py` その検査

## 一線

- **分からない値をでっち上げない。**確率・単価・所要時間・発生率が業務の説明に無ければ、まず書かずに業務の人に聞く。確率未入力を一様分布、単価未設定を 0 円にしない。
  モデルを回すために所要時間などをどうしても仮に置くときは、**仮定として明示する**: 置いた値を `--assumptions` の JSON に `status: "placeholder"` で書き、確認用ページに「AI の仮置き(実測ではない)」として出す。報告の仮定一覧にも載せる。
  `--assumptions` は AI(自分)が書く申告である。`confirmed` や `measured` と書いても、ページは確認済み・実測とは見せず、「AI の申告: 確認済み(confirmed_by X)。人が未確認」「AI の申告: 実測(出典 X)。人が未確認」と出し、冒頭に「人が確かめる必要のある AI 申告」として集める。申告を確かめるのは人間で、AI は確認者や出典を書くだけ。報告でも「確認済み」「実測」と断定しない。
- **Agency と O/F/P/J/U/E/L は宣言しない。**relca が構成から導く。書くのは Actor・Capability(操作契約)・process・Cut・契約。
- **Actor の名前や種別(`grain`)に意味を持たせない。**「AI だから独立」「監査だから原本を見る」と名前で決めない。原本に届くかは `reads` と `source` で書く。
- **Cut を必ず書く。**モデルから外したものは `exclude` / `defer` に理由と `loses` を付ける。モデルにできない段はその段の名前で defer する。Actor にも操作にも解決しない opaque な defer は指標を全体で撤回し、W-CUT-2 が出る。欠けた段があるときは意図した保守的な扱いなので、警告と撤回を報告する。宣言済み Actor 名か Actor.capability を対象にするのは、その層が実際にモデルにあり、その指標を保留したいときだけ。無関係な操作を対象にして値を出させない。Cut が無くても exit 0 になるので、exit だけを見て安心しない。
- モデル作成では規範文書や crates を編集しない。対応範囲を広げる必要があれば変更を別に提案する。

## 書く

1. **業務の説明を表にする。**誰が(人・部署・システム・文書・台帳・取引先)、何を、どの順で、何を見て、誰の承認で、どこで確定するか。説明に無いものは空欄のまま「業務の人に聞く」列に入れる。
2. **最小から始める。**`references/minimal.relca` を写し、主経路だけ(受付 → 判定 → 確定)を書いて `relca check` を通す。通ってから分岐・並行・確率・指標・契約を一つずつ足し、足すたびに回す。
   主経路だけにした間も、**外した分岐と例外経路(差戻し・再提出・取消・障害など)は Cut の `exclude` か `defer` に理由と `loses` を付けて書く。**モデルにできない段の defer はその段の名前で書き、opaque による全体撤回と W-CUT-2 を報告する。黙って省いたり、関係の無い操作の defer で代用したりしない。
3. **Actor を宣言する。**人が担う Actor は `executors [..]` を書く(匿名だと職務分離・権限は unsupported)。原本は `source document` / `source record`。取引先などの当事者は `control party`。
4. **Capability を書く。**`reads`(何を見るか)、`duration`、結果(`result nondet {..}` か根拠のある `outcomes kernel`)、`writes`、確定なら `commits`。引数は、その操作が実際にデータを受け取るときだけ書く(`review(claim)` は申請を受け取って判定する)。**警告(W-RESP-2 等)を消すために引数を足さない。**`case.id` 以外の引数も追加の読取りも無い呼出しは判定と数えられない([構造の出力](../../docs/guide/results.md#構造の出力)、`fixtures/agency-refusal/`)。警告が業務の実態(判定が本当に止めていない)を示していれば、そのまま報告する。
5. **process を書く。**`x = Actor.cap(args)`、`fork … join all`、`if … {} else { end … }`。分岐の条件に潜在(真値)を書かない(E-POL-LATENT)。
6. **Cut・bounds・verify・contract を書く。**`bounds` は cases・live_tasks・clock の 3 つとも。承認の統制は `invariant Ledger.post.commit -> approval == pass` のように契約に書く。
7. **案を比べるなら** 共通の宣言と案ごとの process を一つのモデルに置き、check と explain の `--configuration` で選ぶ(examples/invoice/configurations.relca)。
   案の違いが再送にかかわるなら、下の「再送を含むモデル」で比較表の読み方を確認する。

## 回す

ビルド: `CARGO_NET_OFFLINE=true cargo build -p relca-cli`。binary は `./target/debug/relca`。

| コマンド | 使う場面 | 注意 |
|---|---|---|
| `relca check <m> [--configuration N] [--scenario N]` | 上限(bounds)内の全状態を展開して性質を判定 | 最初に必ず回す。複数 configuration なら `--configuration` が必須 |
| `relca explain <m> [--configuration N] [--scenario N]` | check と同じ解析に反例と DSL 位置を足す | 反例は一つの例で原因ではない。確率の性質には反例が出ない |
| `relca compare <m> --plans A,B --mode <mode>` | 案を並べ、宣言があれば必須条件と目的を評価する | モードは `exhaustive` または `simulation`。順位・総合点は出さない。Cut が違えば mismatch |
| `relca diff <m> --from N --to N` | 構成と Agency の差分 | 変更前と変更後の構成を指定する。intent に無い差分は W-DIFF-1 |
| `relca simulate <m> [--configuration N] [--scenario N]` | 標本推定 | 未解決の非決定に出会うと unsupported。同時刻に残った順序を入れ替えて結果が一致する場合は先へ進める。全スケジュールの安全性検証ではない |
| `relca mutate <m> [--set bpr-default]` | 検査がわざと壊したモデルを捕まえるか | 検出数は検出率でもモデルの強さでもない |
| `relca verify` | 外部バックエンド | 未接続で常に unsupported・exit 4。priority を使うモデルでは理由が `priority_feature_combination` になる |

必ず stdout・stderr・exit を別々に保存する。

```sh
relca check m.relca > out/check.json 2> out/check.err; echo $? > out/check.exit
```

exit: 0 satisfied、1 violated、2 inconclusive、3 invalid、4 unsupported、5 vacuous(`crates/relca-core/src/lib.rs`)。
satisfied 以外は exit 0 にならない。exit と JSON の verdict が食い違ったら、その結果を使わない。exit を記録していない結果は「exit 未確認」で、satisfied でも合格として扱わない。

拒否に当たったら、書き換える前に stderr の補足行(または JSON の `diagnostics[].detail`)を読む。
示された文を確認し、`kind` に応じて [拒否に当たったとき](references/unsupported.md#拒否に当たったとき)の手順を選ぶ。

### 再送を含むモデル

再送の有無だけで compare や diff は止まらない。最上位と各案の verdict を別々に報告する。応答不明の案件は完了に数えない。終端集合が違う案の比較では mismatch を確かめる。

待ちの内訳は check または simulate の出力で確かめる。比較・差分の表が待ちの内訳をまとめているとはみなさない。異議経路や分位点と応答喪失・再送を併用する形は未対応。[診断の理由語](../../docs/guide/results.md#診断の理由語)で拒否された組合せを確認する。

復旧の宣言がない場合、最後の応答が不明の経路では後ろの呼出しを実行しない。照会して復旧する場合は[結果不明からの復旧](../../docs/guide/dsl-reference.md#結果不明を照会して復旧する)を参照する。必要な段を表せない場合は、次の defer の扱いに従う。

- その段を黙って省かず、その段の名前で、例えば `defer ReceiptFollowup reason "結果不明の後の照会と判断はモデルにできない" loses "誰が結果不明を判断するか、負担、期限"` と書く。ReceiptFollowup を Actor や操作として持たないこの場合は opaque となり、対象 report の指標を全体で撤回し W-CUT-2 が出る。これは意図どおりで、報告には W-CUT-2 と撤回の範囲を残す。関係の無い Clerk.recheck などを代わりに defer して値を出させない。実際にモデルに持つ層の指標を保留するときだけ、その Actor や操作を対象にし、`cut.defer_effects` で範囲を確認する。
- 論点表には、判定を「判定不能 / 未対応」として 1 行置く。根拠の欄に `receipt_resolution` の inconclusive、`case_outcomes` の `kind: receipt_unknown` の行(`observed_until`)、`response_receipts` の unknown の行、Cut の defer 名を書く。「要件として決めること」に、誰が(担当・権限)、何を見て(管理画面・照会 API)、いつまでに、どちらへ(進める・返金)決めるかを書く。
- `response` 契約(`… otherwise unresolved`)だけで結果不明の案件が閉じるとは限らない。[上限と未完了](../../docs/guide/guarantees.md#上限と未完了) と `receipt_resolution` を併読する。「response 契約が satisfied なので必ず閉じる」と書かない。

## 言ってはいけないこと

- simulate の結果を「全スケジュールで安全」「検証済み」と書かない。標本推定であり、非決定に出会えば unsupported になる。
- unsupported・inconclusive・invalid・vacuous を合格・おおむね合格・問題なしと書かない。「確かめられていない」と書く。
- 既定値で埋めた欄(JSON の `assumptions`、actors の `status: default`)を確定値として書かない。「既定で仮に入った値」と書く。
- 1 回通ったことを一般化しない。「この上限・この Cut・この版で 1 回 satisfied」と書く。
- satisfied を「業務が正しい」「現実で統制が効く」と書かない。モデル上の結論である。
- mutate の検出数を検出率・網羅率・品質の点数にしない。
- Agency の出力(原本に届かない、拒否が届かない等)を人物の評価や AI の変容の認定にしない。

## 報告

報告には次を必ず付ける。

1. **確認用ページ**(下記)。業務の人に「1. モデルが業務に合っているか」を先に見てもらうよう頼む。
2. **確かめた範囲**: 回したコマンド、exit、verdict、上限(bounds)、Cut、relca の版(`ledger.producer.version`)、モデルの sha256。
3. **確かめていない範囲**: unsupported・inconclusive・invalid・vacuous の性質と理由、relca が `unchecked` に挙げた項目、作れなかった mutation の種類、回していないコマンド。
4. **仮定の一覧**: 自分が置いた値(業務の説明に無かった確率・時間・単価。`--assumptions` に書いたもの)と、relca が既定で埋めた欄。
5. **Cut**: 外した・保留したものと失うもの。Cut の外が作用しないとは主張しない。
6. 業務の人に聞くことの一覧。

### 確認用ページを作る

```sh
python3 skills/relca-model/scripts/review_page.py \
  --model m.relca \
  --check out/explain.json --check-exit 1 --check-stderr out/explain.err \
  --simulate out/simulate.json --simulate-exit 4 --simulate-stderr out/simulate.err \
  --compare out/compare.json --compare-exit 1 --compare-model configurations.relca \
  --mutate out/mutate.json --mutate-exit 4 \
  --assumptions out/assumptions.json \
  -o out/review.html
```

`--assumptions` の形(list か `{"assumptions": [...]}`):

```json
{"assumptions": [
  {"target": "Clerk.enter.duration", "value": "6min", "basis": "説明に無いので仮置き", "status": "placeholder"},
  {"target": "kernel.human_entry", "value": "0.99 / 0.94", "status": "measured", "source": "2026-10 の作業記録"}
]}
```

`target` はページの「モデルの数値・選択の宣言と出所」の表に出る名前を使う。例: `<Actor>.<Capability>.duration`、`kernel.<名前>`、`<Actor>.<潜在変数>`(初期分布)、`<Actor>.capacity`、`metrics.rate.<対象>`、`metrics.window`、`policy`、`arrivals`、`scenario.<名前>.arrivals`、`verify.probability#<k>`、`contract.<名前>#<k>`、`retry.<結果の名前>`(上限回数・間隔・尽きたときの行先)、`response_loss.<結果の名前>`、`<Actor>.<Capability>.idempotency`、`<Actor>.<Capability>.effect_scope`(同じ結果の名前が複数の process にあれば `retry.<process>.<結果の名前>`)。宣言に対応しない target と、重複した target は警告になる。
status は placeholder / confirmed(`confirmed_by` 必須)/ measured(`source` 必須)。必須の欄が無い・知らない status はすべて placeholder として扱う。どの status でも「確かめられた値」にはならない。申告の無い宣言は「出所の申告なし」として冒頭に数える。

`--check` には check か explain の JSON を渡す(explain なら反例が出る)。他は任意。`--no-mermaid` で外部 script を入れない。
diff の診断も載せるなら `--diff out/diff.json --diff-exit N --diff-stderr out/diff.err` を足し、N に実際の終了コードを渡す。diff に別のモデルを使った場合は `--diff-model` で渡す。diff の差分表はページで再現しない。
ページは単一の HTML で、流れ図だけ cdn.jsdelivr.net の Mermaid(版固定)を読む。読めなくても表が同じ内容を持つ。
冒頭に入力の sha256・verdict・exit・生成時刻・relca の版・上限、続いて「確かめられていないもの・警告」、その後に 3 段(業務との照合・結論・検査の効き)。

結論では平均・cohort・分位点を、集計対象、単位、算出根拠と一緒に読む。区間は両端を表示し、値を出せなかった理由や下限は分けて記す。simulate の平均は標本値で、打切りがあれば点を出さない。指標の合否は `metric_checks` の表、compare の採用候補と目的は `hard` と `objectives` の表で別々に読む。
負荷の表には処理件数、稼働率、待ち、作業などを載せる。`scenario.contest_not_filed` がある値はその異議が起票されない場合に限る。W-CONTEST-1 と「評価から外した」行も確認する。`simulation.same_time` はこの実行で順序による違いがなかったことの記録で、他の順序や標本経路での安全性を保証しない。

「確かめられていないもの」は、check・explain・simulate、compare と diff の最上位と各案、mutate の最上位と original から、報告の種類に応じて集める。最上位と各案をまとめた判定の照合は compare だけ。diff は変更前後を個別に読む。詳しくは [全体の verdict](references/results.md#全体の-verdict) を参照する。
Cut の未宣言・loses の欠け、構文解析の失敗、計算の切断、未判定の性質や指標条件、除外された行、数値を算出できなかった理由、mismatch も出力に応じて拾う。未知の根拠・理由・判定は「未解釈」として警告に残す。mutate の繰り返しに起因する未対応は未検出に数えない。
冒頭の欄には他に、relca が自分で申告した未検査(`unchecked`・`agency.unchecked`・`cut.unchecked`・`explanation.unchecked`)、全入力の W-* 警告、回していないコマンド、Agency の観点で not_derived・basis unknown のもの、mechanism_unmodeled の性質、AI の仮置き、人が確かめる必要のある AI 申告、出所の申告なしの宣言、ページが読まなかった最上位の鍵を出す。全部が空のときだけ「この入力の範囲で検出されたものは無い」と出す。
変異体の report は、Cut の保留と W-CUT-2 の診断を拾う。性質などの判定は走査しない。

再送と応答喪失のあるモデルでは、1.5 節に retry・response_loss・idempotency・effect_scope の宣言を JSON の `response.<結果の名前>` と照合して出す。結論の節では、反例の command のある段ごとに元の command・試行番号・呼出し元から見た応答・呼出し元が観測した結果・サーバ側の実際の結果(分析者用)・commit の有無(step.events)・retry の宣言位置と読み方の文を出し、`case_outcomes`(receipt_unknown・完了量 null・receipt_resolution・truncated の区別)、`response_receipts`(到達可能な観測の集合。実行の順ではない)、`response_analysis`、Agency の response_control の注意を別の表に出す。これらの欄の型違い・欠落・未知の語は「読めなかった情報」として冒頭に出し、補わない(試行番号を 1 で埋めない)。

ページでは、渡したコマンドの verdict と exit の食い違いや exit の未記録、モデルの sha256 不一致などを確認する。まとめ直した判定より軽いという警告は、判定を照合する報告に限る。
生成した HTML は成果物として渡すが、リポジトリには commit しない。

ページの流れ図と Capability の表は source を粗く読んで作っている(relca の解釈ではない)。解釈できない文は「未解釈」として残し、警告に出す。

## 検査

`python3 -B skills/relca-model/scripts/test_review_page.py`(relca の binary が要る。無ければ skip になるが、skip は合格ではない)。再送と応答喪失の検査(`ResponseRetryPageTest`)は relca を回さず、`scripts/testdata/ec-plain/` の固定 JSON を使う。この JSON は表示の対照用で、モデルを再実行する入力ではない。再実行できる再送の入力は `crates/relca-core/tests/support/response_loss_inputs.rs` と CLI 試験 `response_loss_retry.rs` にある。
examples/invoice の実出力と、verdict の書換え・exit の偽装・未知の鍵・別モデルの JSON・Cut の削除・HTML と Mermaid への注入・compare の案の内側の改ざん・truncated・mismatch・parse の失敗などの壊した入力で回る。
