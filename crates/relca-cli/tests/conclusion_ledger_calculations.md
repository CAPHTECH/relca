# Q3 結論台帳の手期待と義務

正本は ADR 0058。設計 v0.2 §19 と v0.1 §15 の台帳要求を具体化する。
受入作成時点の production は b74737a2 で ledger を出さない。RED は意図した接続待ちであり、受入作成を production 合格と呼ばない。

## 期待の作り方

- ledger_expected.rs は台帳だけの独立した手期待。producer の値は CARGO_PKG_VERSION と一致し、DSL の0.2を製品版へ昇格させない。build_identity は not_recorded/package_version_only。
- Spec の選択、Actor 順序、via、Rule の宣言/実効版、seed0、要求反復4、confidence3/4、上限を、下記入力と ADR から固定する。
- ledger_links.rs は型ごとの閉じた局所版の場所と版集合を持つ。同じ名前の任意 payload を走査して版を拾わない。局所版は report の元欄にも存在しなければならず、既知の別機能の版も拒否する。
- context/subjects の参照場所は所有 Report の既存鍵と既定の型の構造から列挙する。実際の ledger の値は期待作成に読まない。通常の数値・判定・配列順を再計算する器ではない。
- 値の欠落・余分な鍵は全 object 深さで検査し、serde default で埋めない。手組み有効 wire と単一故障の対で独立 validator 自体も校正する。

## 手値

| 入力 | 算術/分類と台帳の値 |
|---|---|
| serial-time | 所要3+5=経過8、work8。bounds1/2/8、process Check、Office版未宣言、網羅seedはnot_applicable、state_budget1000000。satisfied/0 |
| clockだけ8→7 | 終了8が計算clock7を超えるので終端無し、truncated=true、inconclusive/2。Actor、実効bounds、版の既知性を消さない |
| sim-keys | 要求4反復、seed0、confidence3/4、bounds1/1/2。既存calcの初期hit4とkernel hit2を保持。simulationは有限探索のRoomを使わないのでstate_budgetはnot_applicable |
| 同じsim-keysのcheck | simulation宣言があっても網羅検査はseed/反復を使わない。宣言が存在することと実行設定を分ける |
| qual-late-abort.broken-nondeterminism | 既存calcの3回目で未解決非決定に遭遇し、attempted3/completed2/requested4。seed0と要求4は既知。unsupported/4であって成功/4反復完了でない |
| act-via | Invoice/Extract/Reader/Judge/Ledgerの5 Actor。Extractだけdoc@v3、他はnot_declared。文字列をv3へ勝手に短縮しない |
| rule-diff | ApprovalRuleのdeclaredは両側r1、effectiveはAsIs r1とToBeB r2。改定後に承認failでも登録可能なのでToBeBの契約違反は既存どおりviolated/1 |
| config-diff-parallel | AsIs経過8、ToBeP経過max(3,5)=5、双方work8。差分targetはOffice.long→Office.short、警告はW-DIFF-1二件→W-DIFF-3二件の順。rootが先頭側の設定を代表しない |
| qual-compare-sim | CheckのFlowとFastのQuickは別process、seed1/要求4/confidence1/2、bounds2/2/30。到達のwitnessがあるのでsatisfied/0。標本の安全性証明とはしない |
| mut-evidence | 元と各変異体は既存どおり元source hash。AST編集はsite/operatorで区別する。台帳をassigned_changes/other_changesの検出材料にしない。mutant_limit128 |
| explain serial-time | Reportはflatten、一つのledger。capture_limit200000、explanation版invoice-counterexample-v1、最短/再生の限定を既存欄へ参照する |
| scn-static-config-scenario | Fast/Lightはprocess Q、実効bounds3/1/30。入力の未選択/選択失敗と明示選択を混ぜない |
| resp-origin/resp-contest | 責務/異議経路の既存局所版をcomponentsへ接続。Agencyの版として代用しない |
| conform正常 | Clerk開始0/終了1、Approver開始1/終了2、Poster開始2/終了3。承認成功後の開始が既知。conformant/satisfied/0。元のQ2の三値契約を変えない |
| conform欠損 | 正常ログからClerk.startの一行だけ削除。missing_startによりunknown/inconclusive/2。他の時刻・ID・因果辺は不変 |
| conform引数不正 | E-LOG-0/invalid/3。入力bounds/Actor版はunavailable、既知の最大event10000/行65536/順序100000は保持する |
| verify | 既存の固定stderr、unsupported/4。引数なし/正常path/不在pathのstdoutは同じ。モデルを読んで埋めるfallbackは作らない |

## 閉じたwireと負対照

wire target のGREEN群は、全深さの欠落/追加鍵、空版、未知schema、DSL版による代用、架空build、外部backendの偽接続、入力不明時の既定bounds、seed0消失、要求4→完了2、budget0→既定値、局所版不一致/二重偽造、Actorの版捏造、Rule改定の向き、他configuration/他mutantへの誤参照を検査する。
production decoder の正例は手組みの既存Report hostに手期待ledgerを載せる。欠落/未知版/壊れた参照/局所版不一致を同じhostから一つずつ変え、production自身が拒否することを要求する。独立validatorの拒否だけでこの義務を代用しない。
U-LEDGER-0 のerror登録とunsupportedへのfoldも別のproduction義務とする。

## 数値とraw回帰

旧snapshotから新しい意味期待は作らない。上記の手値と既存独立受入を意味のoracleとして保つ。
ledger_delta.pyは今回の一回限りの移行に使える検査器で、saved raw stdout/stderr/exitに対し、正規Reportの先頭ledgerメンバーだけをbyte位置で除いた残りの完全一致を要求する。objectの再serialize、sort、警告/理由/配列順の正規化をしない。
ledger自体もproducer/bundle/外部未接続/元の局所版との一致を検査する。ただし全欄の独立手期待はRustの専用validatorが担当し、deltaの軽量検査だけを意味の網羅と呼ばない。
手組みのroot/子Reportへの2正例と、通常数値1byte・空stderr・exit・版・重複鍵・root欠落・子欠落の7故障が校正対象。payload内の同名ledgerは除去しない。
既存snapshotが新契約へ更新された後は消去をせずraw完全一致へ移る。旧binaryや歴史manifestは要求しない。現時点の全job検査は最初のledger欠落で止まるため、全jobの差分監査を実施済みとは報告しない。
conformの追加3jobはscripts/snapshot-extra-jobs.jsonの新規入力として別集計し、golden本体の再採取は実装段まで行わない。

## 実装側で残る私有seamの確認

本段ではcrates/*/srcを変更しない。report::check_using等の私有closure seamに任意budgetを渡した場合の実値/不明の実配線は、外部integration testから呼べない。
独立wireではbudget0既知とunavailable/execution_settings_unavailableを別に校正する。内部seamで既定budgetを誤記録しない接続試験と、U-LEDGER-0の実際の発行/非0 exit、seed/子の向き/verify fallbackのproduction故障注入には別の実行根拠が必要である。ここを既存wire試験だけで検証済みとしない。
