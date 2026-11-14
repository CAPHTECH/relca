# 相手別 intent と能力差し替えの手計算

基点は main / 6c6617cb。期待値の出所は ADR 0073 の推奨本文、同 ADR の採択追記、
および下記の集合計算であり、実行結果から値を転記していない。
`inputs.py` は新しい受入入力と、旧 DSL への明示的な書換えを生成する。
`cli.py` は実 CLI の JSON を、production の新しい型定義を使わず照合する。
`intent_pairs.rs` は旧書換えの parser と lower を直接通す。

## 保存・変更・境界

- 保存: 構成内の既定 intent、個別の理由、global preserve、keep fallback の静的検査と Agency、
  既存の名前としての replace / with / intent / from / to、既存 diff の stdout / stderr / exit。
- 変更: 相手ごとに 1 本の注記を選ぶこと、replace による明示的な導出、compare の未照合の開示。
- 境界: 射影が説明できない行、対象外の種類、部分置換は自動承認しない。
  実行時の性質・終端・確率を intent から書き換えない。
- 段階: `--stage pairs` が要望 12、`--stage replace` が要望 11。
  後者だけの実装を待つために前者の期待を緩めない。

## A1: 分母 1 の集合の個数と、実行時間

確率の選択はない。手値の整数も分数 n/1 として計算する。

| 射影 | from (A) | to (B) | from′ |
|---|---|---|---|
| 活性 Capability | Clerk.check, Desk.post | Bot.check, Desk.post | Bot.check, Desk.post |
| 呼ぶ Actor | Clerk, Desk | Bot, Desk | Bot, Desk |
| check の Permission | clerk | op | op |
| check の Actor の容量・実行者 | 1, clerk | 1, op | 1, op |
| Doc の読み手 | Clerk | Bot | Bot |
| 後続が読む v の生産者 | Clerk.check | Bot.check | Bot.check |
| 終端 | done | done | done |

差分の行の同一性は (kind, target)。change が変わっただけでは行の消滅と数えない。
次の表の 10 行を元の差分として出す。直接の行は 2/1、導出の行は 8/1。

| kind | target | change | via |
|---|---|---|---|
| actor | Bot | added | derived |
| actor | Clerk | removed | derived |
| relation | Bot.check | added | direct |
| relation | Clerk.check | removed | direct |
| authority | Bot.check | added | derived |
| authority | Clerk.check | removed | derived |
| resource | Bot | added | derived |
| resource | Clerk | removed | derived |
| visibility | Doc | modified | derived |
| relation | v | modified | derived |

個数は 2/1 + 2/1 + 2/1 + 2/1 + 1/1 + 1/1 = 10/1。
from′ と to は上表の全射影で一致し、残る行は 0/1。
10/1 − 0/1 = 10/1 行を覆う。loses があれば W1/W2/W3 = 0/0/0。

時間は A が 2/1 + 1/1 = 3/1 分、B が 1/1 + 1/1 = 2/1 分。
直列 1 案件なので work も同じ。10/1 分の clock には届かない。
時間の差は実行結果の比較に出るが、新旧の check は異なる Capability なので、
静的な time 行は増えない（共通の Desk.post のモードは両側 sequential）。

### A1b: loses がない

覆う集合は変わらない。W-DIFF-2 は宣言 1 本について 1/1 件。
直接 2/1 行でなく導出 8/1 行に W-DIFF-3 を保つ。
W1/W2/W3 = 0/1/8。loses は replacements にだけ記録し、added の行の loses は null。

旧 DSL の 10 行の個別 intent への展開は、覆う集合だけを検算するもの。
add には loses を書かず、retire 2 行にだけ loses を記すので旧規則の W3 は
10/1 − 2/1 = 8/1。これは新しい replace の注記規則の代用ではない。

## A2 / A3: 無関係の追加と読み手の部分説明

A2 は Clerk.note を両側で呼び、B だけで Audit.log を呼ぶ。
元の 10/1 から Clerk の actor と resource の removed が消える:
10/1 − 2/1 = 8/1。Audit の actor / relation / authority / resource が各 1/1 増える:
8/1 + 4/1 = 12/1。

from′ の活性は Clerk.note, Bot.check, Desk.post。B にはさらに Audit.log がある。
従って 8/1 行を覆い、Audit の 4/1 行を覆わない。W1/W2/W3 = 4/0/4。
Clerk の actor/resource の removed は出してはいけない。
直列時間は A = 1/1 + 2/1 + 1/1 = 4/1、B = 1/1 + 1/1 + 1/1 + 1/1 = 4/1 分。

A3 はさらに Audit.log が Doc を読む。元の行数は 12/1 のまま。
from の Doc の読み手は {Clerk}、to は {Audit, Bot}、from′ は {Bot}。
from′ と to の visibility Doc が残るので覆うのは 8/1 − 1/1 = 7/1 行。
覆わないのは 12/1 − 7/1 = 5/1 行。W1/W2/W3 = 5/0/5。
読み手の差に Bot を「含む」だけでは覆わない。

## A4 / 部分置換 / 連鎖

- Desk.post → Bot.check: Desk.post は両側で活性。removed がないので E-DIFF-0 / invalid / 3。
  loses もないので W-DIFF-2 を出す。部分的に行を覆って成功へ進めない。
- B で Clerk.check を呼び残す: removed がないので E-DIFF-0。
- A でも Bot.check を呼んでいる: added がないので E-DIFF-0。
- Clerk → Bot と Bot → Robot の連鎖: 同じ Bot が added と removed の両方になる照合は成立せず E-DIFF-0。
- 同じ能力同士は E-CON-0、未宣言の Actor/Capability は E-NAME-0。
- 同じ置換元の重複、replace とその元の retire / 先の add は U-SEM-0。
  既存と同じ duplicate intent target の分類であり、汎用的な新構文拒否では合格しない。

## A5: 3 案で理由の流出を防ぐ

C の Robot.check は executor boss、原本 Doc、所要時間 1/1 分。
C→B も A→B の Clerk を Robot に読み替えた 10/1 行。
相手別の replace があると、A→B の replacements の理由は自動化、C→B は移行。
覆った全行の reason / loses は、直接の removed も含め常に null。
理由は covered_by の from / to から宣言へたどり、行に重複させない（採択追記の裁定）。
`intent_block` はそれぞれ {from:A,to:B}、{from:C,to:B}。
どちらも覆い 10/1、W1/W2/W3 = 0/0/0。

A→C と B→A の相手別を宣言しなければ、逆向きへ流用しない。
理由なしの 10/1 行、W1/W2/W3 = 10/0/10、intent_block は null。
明示的な空の相手別は既定を打ち消す。相手別がないときだけ既定を使い、
{from:null,to:B} と表示する。visibility Doc だけの既定なら W1 は 10/1 − 1/1 = 9/1。

段階 1 は replace をまだ使えないので、各相手に `claims` の旧 10 行を置いた別の受入を持つ。
この段階の W3 は旧注記規則で 8/1。相手を無視して最初の塊を使う故障と、
既定との和を取る故障の両方を、C→B の理由で検出する。
preservation も選んだ塊だけから作り、Default でなく Selected の #2 permitted だけを載せる。
名前解決は選んだ A でなく、相手別の to 側 B の process の binding を使う。

compare は intent の照合を行わない。構成内または相手別の intent があれば
unchecked に intent を 1 回、なければ 0 回。既存の differences の未検査も保つ。

## 同時の多対一置換（追加裁定）

A の活性を Clerk.check, Robot.check, Desk.post、B を Bot.check, Desk.post とする。
Robot の結果 z は後続が読まない。v は Clerk.check の結果で、Desk.post が読む。

元の差分は actor 3/1 + Capability relation 3/1 + authority 3/1 + resource 3/1
+ visibility 1/1 + producer 1/1 = 14/1。
2 本の replace を同時に当てた from′ は B と一致し、14/1 行すべてを覆う。

| 宣言 | 帰属する行 | 個数 |
|---|---|---|
| Clerk.check → Bot.check | A1 の 10 行 | 10/1 |
| Robot.check → Bot.check | A1 の Clerk を Robot に替えた行から producer v を除く | 9/1 |

Bot の actor/relation/authority/resource の added は、どちらを除いても消える。
単独で消す宣言を両方帰属させる。visibility Doc はどちらの宣言も必要。
v は Clerk の宣言だけが必要。共通する 5/1 行を引いて
10/1 + 9/1 − 5/1 = 14/1 行、(行,宣言) の組は 19/1 個。
covered_by は from 名昇順の配列。宣言の順を逆にしてもこの集合は変わらない。
各要素と covers の要素が、via まで一致して 1 対 1 に対応する。

理由が異なる追加の対も置く。Clerk の理由/loses は自動化/人の目視、Robot は移行/保守契約。
行の reason/loses は全て null、各 replacements の文はそれぞれのまま残す。
replacements も from 名昇順に出し、試験側で並べ替えない。
宣言順を逆転した全 JSON の比較から除くのは次の 4 path に限る:
`/from/report/model/sha256`、`/to/report/model/sha256`、
`/replacements/0/span`、`/replacements/1/span`。
hash は入力全体の SHA-256、span は実際の UTF-8 の宣言範囲と別途照合する。
現在の ledger のリンクは位置でなく表の path なので、除外する ledger path は 0 本。
他の欄（同名の hash/span や ledger を含む）を黙って除外しない校正も置く。

## 覆ってはいけない種類と、from 側の値

いずれも元の差し替え 10 行は覆うが、次の行は残す。

| 変形 | 元の行数 | 覆わない行 | replace 後の W1/W3 | verdict / exit |
|---|---:|---|---:|---|
| Ctl を B だけへ applies_to | 11/1 | contract_scope Ctl added | 1/1, 1/1 | satisfied / 0 |
| B の終端を other に | 12/1 | meaning end.done removed、end.other added | 2/1, 2/1 | satisfied / 0 |
| B の共通 Desk.post を fork 内へ | 11/1 | time Desk.post modified | 1/1, 1/1 | satisfied / 0 |
| other に異議経路を結ぶ | 15/1 | meaning 2 行、contest 1 行、route の authority/resource 各 1 行 | 5/1, 5/1 | violated / 1 |
| B だけ Policy を r1→r2 へ amend | 11/1 | rule Policy modified | 1/1, 1/1 | satisfied / 0 |
| 両 check に qualified_executors | 12/1 | qualification の removed/added | 2/1, 2/1 | satisfied / 0 |
| 両 check に別の performance profile | 12/1 | performance の removed/added | 2/1, 2/1 | satisfied / 0 |
| 両 process に Manual 縮退経路 | 11/1 | degradation Manual modified（producer が変化） | 1/1, 1/1 | satisfied / 0 |

異議の例は A の終端に other がなく静的な異議経路を満たせないため、全体は violated。
差分注記によってその違反を満足へ変換してはならない。
qualification / performance / degradation は旧 intent で名指せないことも変えない。

個別の visibility の intent を replace の前後どちらに置いても、その行の理由は個別のもの。
覆いは 10/1 − 1/1 = 9/1 行で、Doc は covered_by も covers も持たない。
個別側にも loses があるので W1/W2/W3 = 0/0/0。

Scoped authority の対は、Bot.check の Grant を全域に宣言し、authority_basis 契約だけ B へ適用する。
元の差分は基本 10/1 + contract_scope 1/1 = 11/1。
from′ の Bot.check は Permission op と Grant は持つが、Scoped の basis 契約は持たない。
従って from′→B には authority Bot.check modified と contract_scope Scoped added の 2/1 行が残る。
元の authority 行は added のまま出力し、modified へすり替えない。
覆うのは 10/1 − 1/1 = 9/1 行、W1/W2/W3 = 2/0/2。

## 静的な拒否と入口

同じ pair の重複は E-SYN-0、from=to は E-CON-0、未知の from/to は E-NAME-0。
未宣言の target、違う namespace、未知の preserve selector、to にない binding も、
その原因の code と reason を照合する。選択されない塊も check で検査する。
相手別 keep fallback は、verify を除く check / explain / simulate / mutate /
sensitivity / conform / compare / diff の 8 入口で U-SYN-0、reason に fallback、exit 4。
単に最上位の intent を知らないという U-SYN-0 では受入を満たさない。

各入口の旧扱いと追加裁定の表は ADR 0073 の採択追記にある。
解析の手値は A 3/1 分、B 2/1 分。simulate は確率 1/1 の kernel と 4/1 反復なので
B の案件終端 done の標本数は 4/1。確率要求の 2 点はそれぞれ min=max=1/1。
authority_expired は authority 宣言のない基体なので候補 0/1、mutate は vacuous / 5。
conform は Desk.post のコミットが 1/1 回で at_most_once に適合する。
verify は新構文でも未接続の同じ stdout/stderr/exit。診断 code は足さず ledger の理由も保つ。

## 故障校正の強さと限界

`Calibration` は手書き wire を正例にし、A1b（W3 消去）、A2b（Audit の覆い）、
A3b（Doc の部分説明）、A4b（照合せず satisfied）、A5b（別相手）、A5c（既定の理由）を
1 か所ずつ壊して validator が拒否することを確かめる。これは validator の校正であり、
未実装 production に同じ故障を注入した実行結果だとは主張しない。

GREEN の旧入力は `--fault` の input/wire 故障で不合格になることを別に実行する。
parser/lower の GREEN は RELCA_INTENT_SUBSTRATE_FAULT で負の所要時間を注入する。
故障を外した通常実行を残し、production のソースは一切変更しない。
実装後に A2b/A3b/A4b/A5b/A5c の production 故障を直接再検証する作業は未実施 [U]。

## 文書とコードの照合範囲

- wire は Difference / ProducerValue、diff/intent.rs、lower の名前解決、CLI verify の本体を確認。
  producer は {producer:Actor.Capability} であり、calls の配列ではない。
- verify は code のある構文診断を通常は出さない。未接続の stderr と ledger を検査する。
- contest の静的な path と実行時 contest_response を同一視しない。
- ADR 草案の手値 A1〜A5 とこの再計算の数値に食い違いはない。
  起草時未定だった wire の複数帰属、同時射影、(kind,target) の同一性、入口の扱いは
  確認への回答を受け、推奨本文でなく追記節に固定した。
- ADR 本文の D1〜D5 は起草時の指摘として保存。そこに列挙された全範囲を
  今回独立に再調査したとは主張しない。
