# 構成ごとの契約の適用範囲: 独立の手計算

基点 main / 47a9270e、採択 ADR 0071。期待は以下の計算と採択内容から手で定める。
`m1.relca` は起草時の m1 を既存 DSL で書いた入力。`inputs.py` の編集は入力の生成であり、
binary の観測値を期待へ変換するものではない。`cli.py` が受入、`contract_scope.rs` が起動入口。

## 保存するもの / 変えるもの

- 保存: applies_to 省略時の全構成への適用、範囲内の活性化と契約の違反・vacuous、
  明示 verify、性質の model 全体の ordinal、Cut、通常の実行時間・仕事量・Agency。
- 変更: 範囲外の契約だけを検査の集合から外し、contract_scope と unchecked に残す。
  verdict の種類は増やさず、範囲外を satisfied や vacuous に変換しない。
- U10 の W-CONF-1 は旧入力にも意図して加える。旧出力 byte 不変の要求に対する例外。
  対象を呼ぶ構成、1構成だけのモデルには加えない（依頼者の裁定）。
- 静的拒否の code / reason、diff の status、ページの集計は ADR 末尾の裁定を正本とする。

## 分数による再計算（binary 未使用）

時間の単位は min。1分を `1/1 min` と置く。非決定の result を確率へ読み替えない。
この m1 は result が1値だけなので、取りうる実行の時刻は次の1組に限る。
実行者も各 Actor で1人に固定されている。

| 受入 | 計算 | 手で決まる結論 |
|---|---|---|
| A1 Old | pay: [0/1,1/1]、done = 0/1 + 1/1 = 1/1。仕事量 = 1/1 | reachable成立。適用する契約0節、範囲外1節。foldはreachableだけでsatisfied、exit0 |
| A1b Oldがapproveも呼ぶ | approve: [0/1,1/1]、pay: [1/1,2/1]、done = 1/1 + 1/1 = 2/1、仕事量2/1 | 範囲外のままなのでsatisfied。呼出しを外している警告W-CONF-2、masks_call=true |
| A2 New | approve終了1/1、pay終了2/1、仕事量2/1。bossはMgrのgrant集合の要素 | 契約satisfied、exit0 |
| A2b grantをclerkへ | 時刻はA2と同じ。{boss}と{clerk}の共通部分は空集合 | approve終了1/1にauthority_basis違反、exit1 |
| A3 単一構成Newの死枝 | pay終了1/1、p=ok。条件p==badは偽、approve実行回数0/1 | activation無し。範囲内なのでvacuous、exit5。字句上は呼ぶのでnever_startedのみ、W-CONF-1無し |
| A3の対: 非呼出し | pay終了1/1。approveは字句上も0回 | 範囲内なのでvacuous、exit5。複数構成ならnot_calledとW-CONF-1 |
| A4 compare | reachable行1 + 契約行1 = 2/1行。Oldの適用節数0/1、Newは1/1 | 契約行を消さずOldはnot_applicable/null、Newはapplied/satisfied。foldはsatisfied、exit0 |
| A5 New→Old | 適用集合{Controls}→空集合、差は1/1契約のremoved | preserve無しならsatisfied/0、W-DIFF-1/3。preserve有りなら保つ対象が適用されずinvalid/3、E-DIFF-0 |
| A5逆方向 | 空集合→{Controls} | added、Oldの非適用をvacuousにしない。satisfied/0 |
| ordinal | Controlsの1節 + Laterの1節 = 2/1節。OldでControlsを除いても元番号は2 | Laterは常にcontract Later #2 permitted。active配列の添字で#1へ詰めない |

起草時のA1〜A5とこの再計算の数値・分類に食い違いはない。

## 他の種類、全域、waive

`clause_cases` の body は全て既存DSL。Oldの範囲外ではどれも判定に入らない。
Newでの意味は以下。対象が複数あるとき、1つでもOldで字句上呼ぶならmasks_call=true。

| 種類 | Newの手値 | Oldでのmasks_call |
|---|---|---|
| authority_basis / permitted Screen.approve | bossへのgrant / permitあり、satisfied | false |
| at_most_once Desk.pay.commit | コミット回数1/1 <= 1/1、satisfied | true |
| requires_event pay.start after approve.finish | 1/1でapprove終了がpay開始に先行、satisfied | true |
| separation approve.finish, pay.finish | boss != clerk、satisfied | true |
| excludes approve.finish, pay.start | 前件の後に禁止された開始が起きる、violated | true |
| event invariant approve.finish -> flag==true | 時刻1/1にflag=false、violated | false |
| state invariant flag==true | 初期状態でfalse、violated | true（Capabilityを参照しない） |
| response received -> done by 1min | Newのdone=2/1 > 1/1、violated | true（Capabilityを参照しない） |

全構成[Old,New]を挙げたauthority_basisは省略と同じ。Oldはvacuous/5、Newはsatisfied/0。
全構成列挙それ自体には警告しない。OldへのW-CONF-1は非呼出しの案内であって冗長な列挙の警告ではない。

waive模型: Officer.releaseが1/1で終わり、deliveryの期限2/1より前にwaiveする。
Wait.runは4/1 min。done=1/1+4/1=5/1、仕事量5/1。
NewだけのresponseにNewだけのwaiveは部分集合で有効。両方範囲外のOldでも実行は5/1。
responseの範囲{New}に全構成または{Old}のwaiveは部分集合でなくE-CON-0。
全構成responseに{New}のwaiveは有効だが、Oldにはwaiveが無くresponse期限違反になる。

## simulate / compare宣言評価 / sensitivity

simulate用の操作は `outcomes kernel Certain` と `kernel Certain { * -> { 1: ok } }` に手で書き換える。確率質量は1/1であり、
未入力の非決定へ一様分布を仮定する変更ではない。4回の反復は全て同じ実行となる。
Oldのdoneは4/4回、Newも4/4回。標本で反例の無いsafetyや契約は証明ではなくinconclusive/2。
Newのbad grantは全4回とも時刻1/1に違反し、violated/1。範囲外契約は標本の性質からも除く。
samplingでA3の「0回」を全スケジュールの非活性の証明として扱わない。

宣言評価用にpayがpaid=trueを書き、到着を[0min]とする。
到着数=1/1、Paid案件数=1/1、PaidPerArrival=(1/1)/(1/1)=1/1。
Old/Newともhard >= 1を満たす。適用するverifyもsatisfiedなので両案admissible。
非対称なcontract_scopeを理由にundeterminedへ変えてはいけない。仮定に非対称性と案と契約名を出す。

sensitivityの同一範囲2点はquery reached doneの確率min=max=1/1。
同じ問いのgateは、選択後に消える契約だけでなく宣言のapplies_toを比較する。
非適用のまま{New}→{Third}へ変更してもsensitivity_contract_mismatch/E-CMP-0で閉じる。

## evidence_basis（ADR 0070との組合せ）

Documentはsource document。approveがreads [Document]なら終了時の集合は{Document}、
要求集合との差は空集合でsatisfied。readsを削れば集合は空、差は{Document}でviolated。
Newでは時刻1/1に判定する。Oldはこの契約の範囲外なので、欠落があっても判定に入れず
contract_scope / uncheckedへ載せる。evidence_basisの未対応コマンドのgateも、
非適用の要求まで検査したものとして拒否してはいけない。
適用中のevidence_basisに対するcompare/diff/sensitivity/conformの従来の未対応は保存する。

## conform / mutate / explain / ページ

conform用はverify宣言を除き、全域のBaseline契約を足した旧DSL。Baselineは
`excludes Desk.pay.finish, Screen.approve.start` であり、ログにScreen開始が無いので成立する。
Desk.payはcommitsで、ログのfinishがcommitを表す。
1組のstart/finishは0/1→1/1、正。二重の組は[0/1,1/1]と[2/1,3/1]、
commit数2/1 > 1/1なので範囲内のat_most_once違反。範囲外ならこのmonitorを照合せず、
全域のBaselineと1人に固定したpermissionの照合が残る。BaselineはDeskのcommit回数を
制限せず、2回ともScreen開始無しなら成立する。contract_scopeとuncheckedで非適用を示す。

主なconform受入は少なくとも1つの範囲内の要求を持つ。範囲内の要求が0件の場合は
既存lowerの `at least one verification request is required` / unsupportedを保ち、
範囲外の契約を要求の存在へ数えない（追加裁定）。permissionのログが正しくてもexit0にはしない。
既存lower.rs:5277-5286はASTにstate invariantがある場合の例外を持つため、
範囲外のstate invariantだけが残る場合も要求0件として拒否する受入を別に置く。
対象Capabilityのないこの節はmasks_call=trueとW-CONF-2も要求する。

### sensitivity の要求0件と診断の順序

基点の `crates/relca-core/src/sensitivity/run.rs:51-68` は構文解析と
lower_with_selectionの失敗でrefuse_modelを呼び、その点の処理をcontinueする。
target解決はその後の同ファイル77-78行でContract::from_model / validate_targetsを呼んで行う。
解決の本体は `sensitivity/contract.rs:187-208`。したがってlower拒否が先であり、
順序を変更する要求ではない。manifestの構文・kindの検証は先に通し、意味上のtarget解決だけが未実施になる。

要求0件のsensitivityだけは、宣言できるtargetがprobability_queryに限られ、それを
解決可能にするとverifyが1件になる。この受入では構文上有効な未解決targetを置く（追加裁定）。
1点p0を選び、全診断は親にU-SEM-0 / error /
`sensitivity_model_invalid: point "p0": at least one verification request is required` が1件、
子にU-SEM-0 / error / `at least one verification request is required` が1件。
両方のspanは模型全体の0から末尾の閉じ括弧まで。未解決targetの診断は0件。
`run.rs:246-273` の既存refuse_modelがこの親子の複製とprefixを作る。
受入は全配列を完全一致し、原因の部分一致だけでは通さない。

mutate --kinds authority_expired: Oldの適用中のauthority_basisは0/1節、候補0件、
元検査はsatisfiedだが変異実行0件なのでvacuous/5、reason=no_selected_mutant。
NewはMgrが1/1候補。失効させると時刻1/1の契約違反に変わりdetected、変異試験自体はsatisfied/0。

explain Oldでは反例0件、omittedに範囲外のid/null/not_applicable_configuration_scope。
Newのbad grantの反例は時刻1/1のScreen.approve Finishまで。actorとcapabilityは別欄。
確認用ページは範囲と理由を表示し、非適用契約1節を未確認の1項目として数える。

## 故障校正の対と実装単位

| 故障 | 捕まえる受入 | 単位 |
|---|---|---|
| 未知名を黙認 | Static.test_unknown | 1 |
| 範囲外も検査する / 省略を全域にしない | A1 / 旧入力 | 2 |
| W-CONF-2欠落 / 非適用をsatisfiedとして性質に残す | A1b / scope検査器 | 2 |
| 死枝を自動not_applicableにする | A3 | 2 |
| 除外節でordinalを数えない | ordinal | 2 |
| 片側の契約行を落とす | A4 | 3 |
| 説明できないinventory不一致を許す | inventory故障専用入口（実装への一時注入時に実行） | 3 |
| preserveの範囲外を許す | A5 | 3 |
| 範囲外のauthorityの候補を残す | mutate Old候補0 | 4 |
| explain omittedを落とす | explain Old | 4 |
| gateから範囲を外す | sensitivity変更点 | 4 |
| conformに非適用monitorを残す | conform Old二重commit | 4 |
| ページで非適用を未確認へ数えない | PageAcceptance | 5 |

新構文が未実装の現HEADでproductionの新しい分岐を壊したと主張しない。
GREENの旧DSL入力は一時的に1分を2分へ変えて実CLI試験が落ちることを確認する。
verifyのようにDSLを読まない入口は返されたverdictだけを一時的に壊す。
手書きの新出力の対に対する検査器校正は実装の故障注入とは区別して記録する。

## 出力の形を確認した一次資料 / 不一致

- report.rsのProperty / CaseOutcome、ir.rsのproperty_id、compare.rsのPropertyRow / PropertyCell、
  diff.rsのPropertyComparison、conformance/mod.rsのunsupported_request、mutation.rsの0件規則。
- evidence_basisの既存試験で、simulateの無反例契約はinconclusive、案件終端はcase_outcomes、
  explainのactor/capabilityは別欄と確認した。`if true`ではなく`p == bad`を使う。
- verifyの通常のRefusalは `ledger/settings.rs:117-125` の型で、空のdiagnosticsを
  serialize時に省く。checkの空配列と混同せず、欄の不在と既存stderrの全文を照合する。
- at_most_onceはcommitにだけ書ける（lowerのE-CON-0）。初回の旧DSL検算でfinishを
  拒否されたため、新受入を書く前にcommitとcommitsへ修正した。要求は緩めていない。
- 設計書にはinline outcomesの例があるが、現行parserはoutcomes kernelだけを受ける。
  初回検算でinlineの形はU-SYN-0となったため、確率1/1を変えず名前つきkernelにした。
- ADRの起草時D2のW-CUT-0は現基点では修正済み。構文拒否にW-CUT-0を期待しない。
- 設計書冒頭は実装が存在しないと記すが、現行コードとREADMEは実装済みのCLIを提供する。
- ADR 0017本文の全案共通性質の前提には同文書のreceipt_resolution追記という例外がある。
- ADR 0071の「旧golden差分0」とU10はそのままでは両立しない。裁定でW-CONF-1の
  意図した追加を例外にした。旧入力の保存は意味と新status欄無しを確認し、byte同値とは呼ばない。
