# 値の版の同一性: 手計算と検査の境界

正本は ADR 0076（段階1採択）。期待値は実行前に、以下の呼出し位置と時間から決めた。
`inputs.py` は手書きの入力と期待値だけを作り、binary を呼ばない。
時刻の単位は分。全ての通常操作の時間は `1/1 min`、待ちはない。
非決定分岐に確率は無い。経路の数の比を確率と呼ばない。

## 1. 五組の再計算（独立に分数で足す）

| 入力 | 経路と分数の計算 | 版の比較 | 期待 |
|---|---|---|---|
| a1_fixed | d:[0/1,1/1], f:[1/1,2/1], k(f):[2/1,3/1], a(f):[3/1,4/1], p:[4/1,5/1]。総時間=5×1/1=5/1、作業量=5/1。fail枝は3×1/1=3/1でrejected | a開始前の最後のA=k、A引数=f、B引数=f | satisfied、exit0 |
| a1_unfixed | 同じ時間。B=aの正常終了4/1が最初の違反。終端doneは5/1 | f≠d。ただしenumは両方draft | violated、exit1 |
| a2_reuse（逐次のa5正も兼ねる） | d:[0/1,1/1], k(d):[1/1,2/1], p(d):[2/1,3/1]。総時間=3/1、作業量=3/1。fail枝は2/1 | d=d。A終了とB開始の数値時刻が同じ2/1でも、事象順は終了→開始 | satisfied、exit0 |
| a2_reused | d:[0/1,1/1], d_dup:[1/1,2/1], k(d):[2/1,3/1], p(d_dup):[3/1,4/1]。done=4/1、rejected=3/1 | d≠d_dup。値draftは同じ | violated、exit1 |
| a3_recheck | 最初のkがpass: d,k,pの3/1。fail→修正→再確認pass: d,k,f,k2,pの5/1。再確認fail: d,k,f,k2の4/1でrejected。doneのelapsed/work範囲は[3/1,5/1] | pass枝はd=d、修正枝は最後のk2のf=f。readyは新しい版でない | satisfied、exit0 |
| a3_no_recheck | pass枝=3/1、fail→修正枝=d,k,f,pの4/1。doneのelapsed/work範囲は[3/1,4/1] | pass枝d=d、修正枝A=kのdとB=readyのfが不一致。dはfの祖先でも一致でない | violated、exit1 |
| a4_latest_f | d,k(d),f,k2(f),p(f)の5/1。k2がfailなら4/1でrejected。最初のkの結果はどちらでもよい | 最後のA=k2のf=f | satisfied、exit0 |
| a4_latest_d | 同じ時間。p終了5/1で違反 | 最後のA=k2のf≠d。過去のk(d)は救済しない | violated、exit1 |
| a5_parallel | d:[0/1,1/1]、k,pは[1/1,2/1]で並行。elapsed=1/1+max(1/1,1/1)=2/1、work=1/1+1/1+1/1=3/1 | p開始1/1で正常終了したA無し。p終了2/1でkも終了していても不可 | violated、exit1 |

ADRの手値との食い違いは無い。型Docは一つの定数draftだけとし、版を値で代用する故障を逃さない。
草案のmerge模型にあるrepairedをdraftへ置き換えても、時間と呼出し版の意味は同じ。

## 2. 追加の境界

- no_check: d,pで2/1。examined=null、violated。確認の無い承認を静的に省略しない。
- unused_B: d,kのみで2/1、done/rejectedの両方。B正常終了ゼロなのでvacuous/exit5。
- f7_rejected_A: dに1/1、requires拒否したkは時間0/1、pに1/1、合計2/1。kのelseはpassだが正常終了したAではなく、violated。
- rejected_B: kがpassならpはrequires拒否（時間0/1）、failならrejected。両方2/1。B正常終了ゼロでvacuous。
- failed_result_A: kは正常にfailを返すが、pは実行する。d,k,p=3/1。版はd=dなのでsatisfied。passという名前を意味にしない。
- refused_producer: requiresで拒否したWriterのelse=draftもdの版を作る。dの時間0/1、kは1/1、pは1/1なのでdone2/1、rejected1/1。Aが拒否された場合とは異なり、確認kは正常終了するのでsatisfied。
- a5_slow_B: 並行のAを1/1、Bを2/1にする。d終了1/1、A終了2/1、B終了3/1。elapsed3/1、work4/1。開始時点ではAが無くviolated。B終了時点照合の故障なら、全ての交互順でAが既に終わっており誤ってsatisfiedになる。同時刻の交互順だけでF4の故障が残ることを防ぐ対照。
- nested_merge: a3_recheckの修正枝で二重のyieldを通す。新しい操作を足さないので[3/1,5/1]、rejected4/1。版はfのまま。
- clock2: a2_reuseはkが終わるがpの終了3/1に届かない。inconclusive、truncated=true。未完了をvacuousにしない。
- cases2: 到着[0/1,0/1]、同じ契約を案件ごとに評価する。他案件のAで救済しない。case idは1,2。
- 同じCapabilityの別param: `same_version Pair.use(left), Pair.use(right)` は静的に受理する。先のuseがない最初の正常終了でviolatedになる。

## 3. 既存DSLの手書き対照

`witness(name)` は版の比較の手値を、既存の観測enum `version_ok:VersionTruth` と
`invariant TARGET.finish -> version_ok == yes` に置き換える。
通常の固定経路では初期yes/noだけで手値が決まる。
`a3_no_recheck` は初期yes、修正枝だけ既存のEditor.fixのfinishがnoを書き込む。
追加の呼出しは無い。原模型の時刻を変えず、pass枝を違反にしない。
この対照は新しい版追跡を実装するものではない。既存のparser/lower、正常終了activation、
時刻、終端の出力、独立referenceが手値を表せることを確かめるための有限の書換えである。
新構文の試験をこの対照で代用しない。

## 4. 固定するJSONの形

既存のevidenceの形を確認した場所:
`tests/evidence_basis/cli.py` のcontract（casesは必要時のみ）、terminals、explain、simulate、conform。
`src/explain.rs` と `tests/evidence_basis_explain.rs` は証跡を反例ごとに置く。

- checkのpropertyは `id` と `verdict`（複数案件では既存と同じ`cases`を追加）。
  案件行はcase_id、arrival、verdict。ここでは到着は全て0/1。
- simulateで反例の無い安全性契約はinconclusive/exit2。違反標本ならviolated/exit1。
  終端の標本は `case_outcomes[0].terminals`、未完了数は`unfinished_count`。
  契約のcasesにも1案件の行があり、arrival=0、basis=sample/witness、
  witness=null/replication:1（決定的な違反は初回の標本で出る）を持つ。
- explainは `explanation.counterexamples` からidで選ぶ。`status=verified`、
  `replay.status=verified`。stepsのeventsはactorとcapabilityを別々に照合する。
- 新しい証跡はその反例の `version` に、ADRの案どおり次のobjectを置く。
  capabilityは完全修飾名。bindingはstepsで使う呼出し識別子。
  段階1は繰り返し・再送を拒否するので、案件内のbindingは実行を一意に指す
  （暗黙の実行回数1）。新しい数値の回数欄は要求しない。

```json
{"binding":"p","capability":"Approver.approve","parameter":"d","version":"d_dup",
 "examined":{"binding":"k","capability":"Checker.check","parameter":"d","version":"d"}}
```

確認が無い場合のexaminedはnull。合流の版はreadyでなくf/d。
checkにversion証跡を追加しない。既存出力の全域を変更しない。

## 5. 静的診断と閉じる入口

| 対 | code | messageに含むreason | 根拠 |
|---|---|---|---|
| 未知Actor/Capability | E-NAME-0 | undefined Actor.Capability | lower.rs:6553のcapability_path |
| 未知param（両側） | E-NAME-0 | undefined parameter | lower/response_loss.rs:155のundefined idempotency key parameterと同じundefined … parameterの形 |
| 異なる型、形が同じ別名型 | E-TYPE-0 | same_version_type_mismatch | lower/merge.rs:119のmerge_type_mismatchと同じ機能名付きの名目的な型不一致 |
| 同じ対象 | E-CON-0 | same_version_same_target | ADRの固定reason |
| repeat | U-SEM-0 | loop_feature_combination | ADR、既存repeatの境界 |
| join any、応答喪失、再送、result retry、recovery、degrade | U-SEM-0 | same_version_feature_combination | ADRの4.5 |
| 異議経路の対象 | U-SEM-0 | same_version_contest_route | その性質だけunsupported |
| compare（両mode）/diff/sensitivity（両mode） | U-SEM-0 | same_version_unsupported_command | ADRの4.5 |
| conform（event-contracts） | U-LOG-1 | reason=unsupported_contract | evidence_basisと同じログ診断。not_checkedにunsupported_request、uncheckedにsame_version |
| mutateのmerge/repeat | U-SEM-0 | merge_feature_combination / loop_feature_combination | 既存mutateの範囲は広げない |
| verify | U-SEM-0 | external backend is not connected | 既存の外部backend拒否を保持 |

invalidはexit3、unsupportedはexit4。診断だけでなくverdictとexitも見る。
旧入力対照では新しい節を除いた本体をparser/lowerへ通す。既存の別制約に落ちたら入力を直す。
異議経路の対だけは、引数付きrouteが既存の`contest route requires arguments`を出す。
これは従来どおり保存する（2026-11-15の確認）。契約の無い対でも全体violated/exit1、
到達性はunsupported、terminalsは空。engine.rs:4843-4913のabortは、見つかっていない
到達性をunsupportedにし、未完了探索の終端の極値を取り下げる。
新しい契約ではsame_versionの性質だけunsupportedとsame_version_contest_routeを要求し、
他の性質を成功へ揃えない。

## 6. mutateとF9

- `a2_reuse`: 元の契約はsatisfied。対象 `summary_reused:Main:body/0:d->p` は
  d_dupを作りpだけ付け替える。d≠d_dupなので契約はviolated。
  assigned_changesにid=`contract V #1 same_version`、direction=worsened。
  result=detected。旧入力では同じsiteがundetected/no_assigned_checkのまま。
- 同じ模型でevidence_missingは契約の行を割り当てるが、readsを落としても版は変わらない。
- privateのF9は元のreportのlineageを複製し、変異体側だけindependent_sourcesに
  新しい根拠源を足す。同じ変異体のsame_versionのpropertyをsatisfied→violatedとする。
  本物のcompare_summaryを通し、undetected/independence_creditedと契約の悪化の両方を要求する。
  一時的なsrcの子module登録で実行し、登録は受入commitへ含めない。
  機能の実装時にその登録を本登録し、悪化をdetectedで上書きする一時故障で校正する。

## 7. 故障との対応

| 故障 | 検出する受入 |
|---|---|
| F1 enum比較 | a1_unfixed、a2_reused、a4_latest_d |
| F2 祖先なら許す | a3_no_recheck |
| F3 過去のどれかのA | a4_latest_d |
| F4 終了時点のAで照合 | a5_parallel、a5_slow_B（終了時刻を厳密に分ける） |
| F5 合流欄を版にする | a3_recheck、nested_merge |
| F6 導出へ戻す | 五組全てのagency/responsibilities/via_model_pathsと警告のbyte比較 |
| F7 拒否を確認に数える | f7_rejected_A、rejected_B |
| F8 旧入力にも記録を確保 | 新規のlegacy完全出力保存試験。既存snapshotを参照して既存fixtureのstdout/stderr/exitを完全一致で照合（編集なし） |
| F9 規則の失敗を上書き | same_version_mutation_privateの人工report対と本物の比較器 |

GREENの校正は入力のdurationを1→2へ変える一時注入、または観測stdoutの一箇所を
破損する一時注入で行う。期待を書き換えない。srcに故障を残さない。
REDの主因は現HEADでsame_versionがU-SYN-0になること。
F1〜F7/F9のproduction故障は機能実装前には入れられず、GREEN後に校正する。
