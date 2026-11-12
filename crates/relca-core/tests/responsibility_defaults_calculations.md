# 未宣言責務の手計算(ADR 0054、実行前)

設計 v0.2 §5.4規則3、§5.6のU/Eを使う。構文を一つも増やさない。

- `binding`: Input.seedが1分で固定passを返し、Judge.check(e)は1分でpass/failを返す。passだけでPoster.post(1分、commit)へ進む。
  U={Judge}、E={Poster}、責務なしなので警告は宣言順[Judge,Poster]。
  returnedのelapsed/workは2、doneは3。safetyはsatisfied、exit0。
- `non-stopping`: bindingのガードだけをv==pass || v==failに変える。Judgeは引数eを持つ判定だが全結果がcommitへ通るのでstops=None、Uなし。
  E={Poster}で警告[Poster]だけ。legacyのstopsの絞り込みを外す故障はJudgeを余分に警告する。
  実際にresponsibility.rsのlegacy候補でstopsの条件だけを外し、全casesを回すとcore/CLIともこの対だけが落ちた。
  atomic/version_checkの専用試験は同じ故障下で通った。unreachableもこの故障の検出対ではなかった。復元後にfocusedを再検査する。
- `people-only`: people p/qだけを足す。全Actorのcapacityとexecutorsの不在は不変で、U/Eと警告[Judge,Poster]も不変。
  responsibility_people_undeclaredはpeople無し側だけに残る。Judgeだけを落とした偽vectorは手値に一致しない。
- `named`: people p/qとJudgeのexecutors=[p]を追加し、capacity 1を人数による既定へ置換した帰属追加の対。U/Eは不変。
- `duty`: Judge.executors=[p]、responsible p for Judge.checkならJudgeは宣言あり。
  警告は[Poster]。責務1行を削除した対は[Judge,Poster]へ戻る。
  responsible qではJudgeを覆わず、executors=[p,q]の片方pの責務なら覆う。
- `state`: Poster.openはActorの観測bool=true。postはrequires Poster.open==true else failを持つ非atomic commit。
  process Uはstate_guardsでnot_derivedでもE={Poster}はderived。警告[Poster]。
  1分でdone、safety satisfied、exit0。commitsだけを消した対はEを持たず警告[]。
- `unreachable`: Judge.checkはpassだけ。if v==failの枝にPoster.postが字句上ある。
  実際は2分でreturned。Eは字句上のcommitの所在なので[Poster]、Judgeは止める異なる値を持たずUなし。
  そのcallを枝から消した対(Posterの宣言は残す)は警告[]。
- `repeat`: Poster.postを2回、各1分。Actorの候補は集合なので[Poster]が1回、doneは2分。
- `atomic`: agy2-atomic-uのpeopleを削除。UのApproverとatomicのLedger、EのLedger。
  和は[Approver,Ledger]。Clerkは引数も追加の読取り親も無い生成根としてatomic_judgementsで候補から外れる。
  Clerkのstopsを計算して除いた対ではない。stopsの絞り込みの故障は別に実行して検出対を記録する。
- `rule`: bindingのifがRule Rのbool=trueも読む。U={R,Judge}、E={Poster}、[R,Judge,Poster]。
  fixed-trueのrequiresだけではRule Uを省略する既存の形があるため、その形をRule Uの正例には使わない。
- `version`: version_checkのLedgerは状態拒否者かつE。同Actorなので[Ledger]が1回。
- `plain`: commitもUも無い1分のcall。警告[]。未対応の全Eを推測する試験ではない。

coreはlowerのdiagnostic全rowを手のActor名とIRのActor宣言spanへ照合する。
実CLIはcode/message/severity/span、完全なW-RESP-0集合、stderrの一致、exitを別に照合する。
exhaustiveと参照は警告が時間・work・verdictを変えないことを手値へ照合する。
simulationのsafetyは全スケジュール証明にならずexit2、非決定のbinding模型は使わずplain/stateを使う。
compare/diffの同じ模型の両選択は同じ警告。mutateの各reportも個別の模型でありwrapperの警告を創作しない。

## 取り込み前レビューの追加受入

- 1人pをJudgeとPosterに割り当て、Judge.checkだけの責務を宣言すると、現規則は人単位なので両方のW-RESP-0を抑制する。
  responsibilitiesはpの1行だけ、uncheckedにresponsibility_assignment_scopeを出す。責務を消す対は[Judge,Poster]。
  対象Capabilityの完全被覆を検査したと解釈しない。
- `mixed_diagnostics`: 手書きのOrder模型は匿名Posterの1分commitだけ。全診断は順にW-CUT-0(span null)、W-RESP-0(Poster、span 51..159)。
  先行2行の長さ28+23=51、Actor行は108 bytes。message/severity/codeを含む完全vectorとstderr全行を固定し、相対順だけを交換する偽出力を拒む。
- `effects_limit`: requiresを外した独立のcommitを1回と100回にする。各callに引数は無い。
  両側にpeopleを11,000人宣言するが、Posterはcapacity 1・executors無しのまま。effectsの各carrierが全peopleを走査するだけで
  100×11,000=1,100,000 workとなり、既定1,000,000を超える。単一call側の静的構成は同じである。
  legacyの既知のE={Poster}は両側に残る。それだけではeffects全体で導けない候補の不存在を証明しない。少数側のeffectsはDerived。
  上限側だけuncheckedに`responsibility_default_candidates: process Flow: U/E not_derived (derivation_limit)`を1件要求する。
  process名・導けないU/E・理由を同じ開示に残し、一般的なexecutors未検査だけで代用しない。
  clock 1は実行の切断であって静的導出上限とは独立。試験の失敗を避けるために期待を緩めない。
