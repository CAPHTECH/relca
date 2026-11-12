# 請求書の例

受付、入力、照合、承認、台帳への登録を有限モデルにした例です。
人による入力、照合の並列化、抽出処理への入力移転を比べます。
確率と時間は検算用の仮定であり、実業務の測定値ではありません。

## 実行する

リポジトリのルートで `cargo build -p relca-cli` を実行します。
以下のコマンドは同じ場所から実行してください。stdout は JSON、stderr は診断、
終了コードは最上位 verdict に対応します。0 以外も、この例で意図した結果です。

| モデル | 何を示すか | コマンド | 期待 verdict / exit | 限界 |
|---|---|---|---|---|
| `as-is.relca` | 人が入力し、二つの照合を逐次実行して承認・登録 | `./target/debug/relca check examples/invoice/as-is.relca` | satisfied / 0 | 受付一回の一案件。実業務全体の保証ではない |
| `to-be-p.relca` | 入力は人のまま、照合を並列化 | `./target/debug/relca check examples/invoice/to-be-p.relca` | satisfied / 0 | 人作業の総量の減少や処理能力の安定性を示さない |
| `to-be-a.relca` | 抽出処理へ入力を移転 | `./target/debug/relca check examples/invoice/to-be-a.relca` | violated / 1 | 時間が短くても誤登録の確率上限に違反する |
| `to-be-c.relca` | 観測できない難度を業務判断の条件に使う負対照 | `./target/debug/relca check examples/invoice/to-be-c.relca` | invalid / 3 | E-POL-LATENT で拒否。実行結果の比較には使えない |
| `configurations.relca` | AsIs・ToBeP・ToBeA を共通宣言の下で比較 | `./target/debug/relca compare examples/invoice/configurations.relca --plans AsIs,ToBeP,ToBeA --mode exhaustive` | violated / 1 | 各案の性質と構成差を並べる。比較順位や経営要約は生成しない |
| `phase1.relca` | record・開始前件・atomic な台帳・品質・待ち・保守責務を含む AsIs | `./target/debug/relca check examples/invoice/phase1.relca --configuration AsIs` | satisfied / 0 | 保留した時間・費用・品質の数値は not_configured |
| 同上 | 同じ宣言で照合だけを並列化した ToBeP | `./target/debug/relca check examples/invoice/phase1.relca --configuration ToBeP` | satisfied / 0 | 保留した量を比較上の改善とは読めない |
| 同上 | 同じ宣言で入力を移転した ToBeA | `./target/debug/relca check examples/invoice/phase1.relca --configuration ToBeA` | violated / 1 | Core の確率性質の違反と品質欄の未設定は別に読む |
| `to-be-a-phase1.relca` | 拡張した ToBeA の単独解析と反例の説明 | `./target/debug/relca explain examples/invoice/to-be-a-phase1.relca` | violated / 1 | 確率性質の違反を一本の原因経路にはしない |
| `rules.relca` | 承認規則を読む ToBeA | `./target/debug/relca check examples/invoice/rules.relca --configuration ToBeA` | vacuous / 5 | 非活性の契約を satisfied に数えない |
| 同上 | 変更前の規則で授権を検査し、承認を全件免除する ToBeB | `./target/debug/relca check examples/invoice/rules.relca --configuration ToBeB` | violated / 1 | 無条件の承認 invariant を破る。低リスク案件だけの条件付き免除ではない |
| `busyday.relca` | 4分間隔120件の人工負荷、1000反復、960分の業務 horizon | `./target/debug/relca simulate examples/invoice/busyday.relca --configuration AsIs --scenario BusyDay` | unsupported / 4 | 未解決の非決定を一様乱数で埋めない。負荷下の平均/P95や安定性は得られない |
| `quantile-compare.relca` | 終端時間の分位点目的と品質 hard の分離 | `./target/debug/relca compare examples/invoice/quantile-compare.relca --plans AsIs,ToBeP,ToBeA --mode exhaustive` | violated / 1 | 目的の宣言はあるが、Cut の保留と交差する量の数値順位を出さない |

`commands.json` と `commands-phase1.json` には、check 以外も含めた35件の呼出しと期待 exit があります。
通常の3案の simulate は unsupported / 4、既定の mutate も未対応種を残して unsupported / 4 です。
未解決の非決定や未完了の変更生成を、合格や検出成功に読み替えないでください。

## 時間と品質を読む

計算は [calc.md](calc.md)、出力の共通の読み方は [結果のガイド](../../docs/guide/results.md) を参照してください。
基本3案の posted の Core 経過時間は17/14/9分、人作業は16/16/10人分です。
誤登録の確率は AsIs と ToBeP が93/125000、ToBeA が231/125000で、上限は1/1000です。
これらは全流入に対する模型の確率であり、登録条件付き誤り率ではありません。

Cut で再提出・異議の発生率と所要時間を保留しています。
関連する Workload の時間・費用や品質・分位点・目的の数値は理由つき null になります。
calc.md の数値対照は defer 行だけを外して検算するもので、現入力の null を数値に読み替えるためではありません。
単価と保守作業時間は未設定です。0円・0分として扱いません。
事業価値、経営要約、実際の技能、実在の当事者の受諾、AI を介した変化の認定は対象外です。
外部 verify は未接続で unsupported / 4。stderr の outside Phase 1 はその対応範囲外を指します。

## 出力欄と入力の対応

以下の ID は試験が照合する識別子です。`ID: /pointer` は `commands.json`、
`ID → /pointer` は `commands-phase1.json` の呼出し結果に対する JSON pointer です。
配列は添字だけでなく名前・id も確認してください。宣言の存在と実行時の保証は別です。

| ID | 対象 | 種別 | 出力欄または未対応理由 | 境界 | 実物 |
|---|---|---|---|---|---|
| I12.1-receipt | 受付一回と終端 | output | /cut /terminals /metrics/cases/0/counts | exception は未到達 | as-is-check: /cut, /terminals, /metrics/cases/0/counts, /diagnostics/6 |
| I12.1-cut | 除外と保留 | src | InvoiceCut と horizon | horizon は計算上限と別 | as-is-check: /cut; p1-as-is-check → /cut_context/horizon |
| I12.1-plans | 三案の比較 | output | /plans /tables /evaluation | 採否と順位は別 | compare-exhaustive: /plans, /tables; p1-compare-exhaustive → /evaluation |
| I12.2-state | 観測と潜在の分離 | src | 型・状態・source・kernel の宣言 | 業務判断から真値を読まない | as-is-check: /assumptions, /agency/origin |
| I12.2-people | 実行者と占有 | output | /metrics/people /metrics/actors | 個人別最大の和は総負荷ではない | as-is-check: /metrics/actors, /metrics/people; p1-as-is-check → /metrics/people |
| I12.2-record | record と入力者 | src | Entry record と input_by、finish executor.id | 使用していない構文へ一般化しない | phase1.relca、to-be-a-phase1.relca |
| I12.2-requires | 開始前件 | src | requires と明示の else、状態前件 invariant | 拒否は開始・finish・commit ではない | p1-as-is-check → /properties |
| I12.2-ledger | 重複拒否の機構 | src | keys/set/atomic と post(case.id) | 通常例は再送の受入ではない | p1-as-is-check → /properties |
| I12.2-authority | 授権と責務 | output | /properties /responsibilities | 宣言した責務を所要時間へ変換しない | as-is-check: /properties/5, /responsibilities, /contest_paths; p1-as-is-check → /responsibilities; rule-a-check → /actors |
| I12.3-kernels | 難度と見逃しの表 | src | human_entry/ai_entry/po_match/receipt_match/approval_check | 検算用仮定である | as-is-check: /assumptions, /agency/origin |
| I12.3-independence | 共通原因と条件付き独立 | output | /agency/common_causes | 現実の独立性の認定ではない | as-is-check, to-be-p-check, to-be-a-check: /agency/common_causes |
| I12.4-controls | 登録前の統制 | output | /properties の invariant/at_most_once/authority_basis | 性質ごとに読む | as-is-check: /properties/2, /properties/3, /properties/4, /properties/5 |
| I12.4-vacuous | 職務分離の非活性 | output | /properties の separation verdict | satisfied に数えない | to-be-a-check: /properties/6/verdict |
| I12.4-returned | 差戻し | output | /terminals /cut /quality/properties | 受付一回の処置で、決済の最終解決ではない | as-is-check: /terminals/0, /cut; p1-as-is-check → /quality/properties |
| I12.4-quality | 品質の分母 | output | /quality/properties /quality/cohort | 登録条件付き誤り率ではない。保留と交差する数値は null | as-is-check: /metrics/unchecked; p1-as-is-check → /quality/properties, /quality/cohort, /quality/outcomes_per_arrival |
| I12.5-time | 終端時間と人作業 | output | /terminals /metrics/actors | 平均/実測ではない | as-is-check, to-be-p-check, to-be-a-check: /terminals/1/elapsed, /terminals/1/work, /metrics/actors; p1-as-is-check → /metrics/cases, /quality/quantiles |
| I12.5-capacity | 名目能力 | src | calc.md の60/7・15件/時の算術 | runtime の安定性検証ではない | calc.md |
| I12.5-risk | 誤登録確率と上限 | output | /properties の probability と verdict | AsIs/P satisfied、A violated | as-is-check, to-be-p-check, to-be-a-check: /properties/1/probability, /properties/1/verdict |
| I12.5-decomposition | 並列化と入力移転 | output | /plans /evaluation | 目的の良さで違反を相殺しない | compare-exhaustive: /plans, /tables; p1-compare-exhaustive → /evaluation/hard, /evaluation/objectives |
| I12.5-busyday | 人工負荷 | output | /scenario /diagnostics | 未解決非決定により simulate 拒否 | as-is-check: /metrics/unchecked; busyday-as-is-simulate → /scenario, /diagnostics |
| I12.5-c | 潜在条件の拒否 | output | /diagnostics の E-POL-LATENT | 観測可能な代理は未宣言 | to-be-c-check: /diagnostics/0/code, /verdict |
| I12.5-b | 規則の静的改定 | output | /actors /properties /differences | B は全件免除。条件付き免除の分布は未設定 | as-is-mutate: /unchecked; rule-b-check → /actors, /properties; rule-diff → /differences |
| I12.6-structure | 構成差分と intent | output | /differences /agency/differences | 未計算の種類は unchecked | diff: /differences, /terminals, /diagnostics, /unchecked; p1-diff → /differences, /agency/differences |
| I12.6-origin | source への経路 | output | /agency/origin /agency/viewpoints/observation | 通常と異議の route を区別 | as-is-check, to-be-a-check: /agency/origin; p1-to-be-a-check → /agency/viewpoints/observation |
| I12.6-responsibility | 責務と観測 | output | /responsibilities /diagnostics | 人の落ち度や作業時間へ転換しない | as-is-check: /responsibilities, /diagnostics/1; p1-as-is-check → /responsibilities |
| I12.6-refusal | U の条件 | output | /agency/refusal /agency/viewpoints/effects | atomic の拒否と process の判定は別 | as-is-check, to-be-a-check: /agency/refusal/0/judgements; p1-to-be-a-check → /agency/viewpoints/effects; rule-a-check → /agency/viewpoints/effects |
| I12.6-lineage | 共有 entry と根拠源 | output | /agency/lineage /agency/lineage_groups | Actor 数を独立根拠数としない | as-is-check, to-be-a-check: /agency/lineage, /agency/lineage_groups/0/independent_sources |
| I12.6-wlin2 | W-LIN-2 | output | /diagnostics /agency/common_causes | 検出力の数値の認定ではない | as-is-check, to-be-p-check, to-be-a-check: /diagnostics/0, /agency/common_causes |
| I12.6-l | L の呼出し回数 | output | /agency/viewpoints/retention | 実際の技能と縮退依存は未確認 | to-be-a-check: /diagnostics/4; p1-to-be-a-check → /agency/viewpoints/retention |
| I12.6-maintenance | 保守の責務 | output | /responsibilities /differences | 保守 Task と所要は未設定 | as-is-check: /assumptions; p1-to-be-a-check → /responsibilities; p1-diff → /differences |
| I12.6-burden | 人作業と異議負担 | output | /metrics/overall/named_people_work | 異議率と実測所要は保留 | as-is-check, to-be-a-check: /metrics/overall/named_people_work; p1-to-be-a-check → /responsibilities |
| I12.6-concentration | 共有する生成根 | output | /agency/refusal /agency/lineage | 組織の実効性や点数を推定しない | to-be-a-check: /agency/refusal/0/judgements, /agency/lineage; p1-to-be-a-check → /agency/viewpoints/effects |
| I12.6-contest | 異議の経路 | output | /contest_paths /responsibilities | 構造と期限内応答を区別 | to-be-a-check: /contest_paths, /responsibilities; p1-to-be-a-check → /contest_paths |
| I12.6-contest-response | モデル内応答 | output | /contest_runtime /properties | 返答は終端を覆さず、受諾や救済を保証しない | as-is-check, to-be-p-check, to-be-a-check: /contest_runtime/0, /properties/9 |
| I12.6-ai-path | 生成物の経路 | output | /agency/lineage /via_model_paths | 読取り経路の追跡である | to-be-a-check: /agency/lineage, /agency/unchecked; p1-to-be-a-check → /via_model_paths |
| I12.6-704 | 承認者の直接読取り | output | /agency/origin /agency/viewpoints/observation | この配置だけで統制の形骸化を断定しない | as-is-check, to-be-a-check: /agency/origin; p1-to-be-a-check → /agency/viewpoints/observation |
| I12.6-705 | 検出力の仮定 | output | /diagnostics /agency/common_causes | モデル外の情報源は未検査 | as-is-check, to-be-p-check, to-be-a-check: /diagnostics/0, /agency/common_causes |
| I12.6-706 | 実際の技能 | unsupported | 実際の技能と縮退依存は未対応 | 回数から削除候補と認定しない | p1-to-be-a-check → /agency/viewpoints/retention |
| I12.6-708 | via model | output | /via_model_paths | 実運用の変化の認定ではない | to-be-a-check: /agency/unchecked; p1-to-be-a-check → /via_model_paths |
| I12.3-mutation | 故障注入 | output | /mutations /run /unchecked | invalid_mutant や未完了 run を検出成功にしない | as-is-mutate, to-be-p-mutate, to-be-a-mutate: /mutations, /unchecked, /run; p1-a-mutate → /mutations, /run |
| I12.5-simulation | 非決定性の拒否 | output | /diagnostics /verdict | 推定値や全スケジュールの安全性を主張しない | as-is-simulate, to-be-p-simulate, to-be-a-simulate: /diagnostics, /verdict |

## 回帰検査

現在の stdout・stderr・exit は `crates/relca-cli/tests/snapshots/` と完全一致で照合します。
手計算や独立参照の代わりではありません。意図した出力変更時だけ、
`CARGO_NET_OFFLINE=true TMPDIR=.tmp ./scripts/update-snapshots.sh` で更新します。
個別の内容は `python3 -B scripts/snapshots.py --show invoice-p1-as-is-check` で確認できます。
