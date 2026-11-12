# ADR0047 取り込みの手計算

基点 b6951fb。期待を先に固定し、production の出力から数値を採らない。
入力は support/evaluation_integration_inputs.rs、公開 API と実 CLI の共通受入は
support/evaluation_integration_acceptance.rs。既存116dirsとは別のinline母集団。

## D01: 業務地平と計算切断

cmp7-clock の Flow は check4分、passなら post1分。到着0/6/12なので
posted5/11/17、returned4/10/16。各到着 Wrong=1/50、Registered=23/25。
H30なら平均Wrong=1/50。H14なら最初の2件のWrong和1/25、3件目は[0,1]、
3到着で割り[1/75,26/75]。通常値はnull/undetermined。
hard<=3/10はI、<=1/2はS、<=1/100はV。verify reachable postedは切断でIなので
looseでも採用でなくundetermined、strictはexcluded。rootはI/I/V。
clock14<H30、live_tasks切断、状態予算は計算停止なのでtruncatedを優先する。
目的は通常値のnullを保持し、hard用の保守区間を目的値へ流用しない。

## D02: 標本の未知とcross-plan補正

両案ともcorrect=true、check3+post1、1到着、R3、confidence1/2。
別processなのでsimulationのキー分離を満たす。hard1行、2案、1case、arrivals
なのでm2、alpha=(1-1/2)/(2*2)=1/8。H1は各case hits0/unknown3/trials3、
CP[0,1]、point null、reason undetermined。H5はhits0/unknown0/trials3、
CP上端は1-(1/8)^(1/3)=1/2。hard<=3/5はI→S、verifyもI→S、root I→S。
Registeredを分母にするH1ではm4、alpha1/16、分母下界0でinterval null、
reason denominator_may_be_zero、source_reason undeterminedを保つ。
2到着ならm4、同じpropertyのhard2行ならm4。verifyの族はhardの族と別。

## record / atomic / Held / Overflow / drain

record例はpass=9/10+1/50=23/25、Wrong=1/50、Wrong/Registered=1/46。
二案は同じPで網羅比較、硬い上限1/50はS、未使用のelseの宣言も消さない。
潜在の真値をprocess guardへ移す単一故障はE-POL-LATENT、表と評価を出さない。
cf-version-atomicは既存の実行をそのまま比較し、単独reportのproperties/metrics/quality
をbyte相当のJSONで運ぶ。新評価はatomicやversion_checkを再解釈しない。
Held例の到着0/1/2、1件上限、run5、H3では3件全てcut_off。
Rateは[0,1]、hard<=1はSでもverify Iなので採否undetermined。
H16とhorizon無しでは終了5/10/15、Rate1、hard<=1 S、全案admissible。
Overflow例は同時到着2、queue1、run5、H1。Overflow率1/2、cut_off率1/2、
通常endへ二重計上しない。Hit率[0,1/2]、hard<=1/2 Sでもverify I。

## D04: Rule の現在の境界

以下は品質撤回を用いていた時点の対照であり、現行の通常品質の手値はrule_quality_calculations.mdを参照する。
Rule.enabledの固定真/偽に関係なく品質はabortedのnull、checked空、quality.rule開示。
hardはunsupported、採否undetermined、目的null/abortedで通常0/1にしない。
通常0/1は将来義務。網羅でも標本でも実行自身は続き、単独reportをそのまま運ぶ。
Rule差分は現A6の独立手表rule_integration_inputs::differencesを使う。
古いdeferredの9行に加え、後続裁定のconcentration.single_actorの1行を含む10行。
Ruleはprocess J/人/生成根に変換しない。責務は両側全部を渡す。

## 検査境界

正常網羅/標本のchecked/uncheckedはcompareの公開契約に従い手で列挙する。
評価の失敗は表とcheckedを出さない。scope拒否、scenario選択、E-CMP-0の
実在案間pairsを保つ。A7のwaiting_tablesの並びはD05として後の取り込みへ残す。
私有adapterは既存evaluation_tests/spyで接続済み。公開の取り込み試験を
全私有経路・全入力・性能の保証へ昇格しない。

## 取り込み後レビュー R1 / m1 / m3 / m4 (0fe6226c)

以下を実行前の手期待とする。quality 表は property の宣言順、cohort の宣言順、
outcome の model-wide End 宣言順と rejected / unfinished / deadlock / optional cut_off 順。
property は expected_count (標本では直後に unknown_count)、cohort は value と
bounds_with_undetermined、outcome は expected_count と rate の順である。
全行の列は要求した案の順を保ち、status は reported (理由付き null も reported)。

- record: Registered=23/25、Wrong=1/50、Pass=23/25。cohort は WrongRate=1/50、
  Conditional=(1/50)/(23/25)=1/46、PassRate=23/25、bounds は全て null。
  outcome は posted=23/25、returned=2/25、残り3種=0。1到着なので件数と率は等しい。
  行数は3+6+10=19、列 Left / Right。
- Rule: Hit / Rate / done / rejected / unfinished / deadlock。網羅は11行、標本は12行。
  全量は aborted の理由付き null、bounds は null。標本の samples は0、集約値の
  mean/min/max 又は point は null。列 AsIs / ToBeB、真と偽を0/1へ埋めない。
- Held H3: Hit expected_count は undetermined の区間[0,3]、Rate は理由付きnull、bounds=[0,1]。
  done/rejected/unfinished/deadlock は件数・率0、cut_off は件数3・率1。13行、Left / Right。
- Overflow H1: Wrong / Registered のexpected_count は undetermined の[0,1]、Spill は[1,2]。
  WrongPerArrival の bounds=[0,1/2]、WrongPerRegistered は undetermined/null bounds、
  SpillPerArrival は undetermined/null、bounds=[1/2,1]。End の順は returned / posted / overflow spill。
  前二種とrejected/unfinished/deadlockは0、overflow spill とcut_offは件数1・率1/2。
  23行、Left / Right。未完了を通常のEndや品質0へ変えない。
- 通常の horizon H14: Wrong expected_count は undetermined/[1/25,26/25]、bounds を持つ
  WrongPerArrival は[1/75,26/75]。既存 D01 の手値と合わせて表からも照合する。
  Registeredの件数は[46/25,71/25]、条件付き比のboundsは[1/71,13/23]。
  outcomeはreturned件数4/25・率4/75、posted件数46/25・率46/75、cut_off件数1・率1/3、
  他は0。18行、列Check / Fast。quality_table::horizon14で全行を固定する。

初回の受入はexpected_countまでnullにする誤りでHeld/Overflowの2本が両targetでRED。
一次のquality.rs::sum/exhaustiveではpropertyの期待件数は未知込みの区間を保ち、
cohortの通常比だけを撤回する。上記へ訂正した。Spillも未完了case1の到達判定が
未確定なので[0,1]、確定Overflowのcase2が1で、合計[1,2]を2で割る。
実際のoverflow outcomeは到達propertyと別で、率1/2のまま。出力値の採取ではなく
この既存の集約契約に合わせた訂正であり、正しいproductionを直す要求ではない。

R1 の負対は表だけの空配列、1行欠落、1行重複、列逆転、値1箇所の変更。
全て validate_compare_wire が Err、元のwireは Ok を期待する。行逆転・分母改竄・
表全体の欠落も追加の負対とする。nested report/evaluation は変更しない。

m1: 凍結 CutOff 2 argv を含む manifest の閉じた集合は C5 / 品質 / Rule / 評価。
実際の byte gate と補助射影の接続を sentinel で検査し、関数名の文字列一致だけにしない。
評価だけは凍結 allowed による project への直接 dispatch (他3系列は project_for_args)。
旧manifest・旧byteを再採取しない。

m3: 同一の選択済みconfiguration 2つの実効horizonが None/Some(5) 又は Some(5)/Some(6)
なら tables は Err(report inventories differ)。一致する None/None と Some(5)/Some(5)
は正の対照。各reportは対応する実効IRから作り、scenario/report矛盾による別のErrを避ける。
srcを変更せず、既存のcfg(test) seamのevaluation_review_private_compareに私有moduleを接続する。

m4: End の必要在庫は選択processの字句集合である。metrics.overall.counts.end_counts
は両collectorが選択processから字句で作る完全な在庫なので、metricsのあるwireはこれを
使う。doneの全行削除と、未観測neverの全行削除を各Err、元wireを両modeでOk。
未選択processのunselectedは品質outcomeにはあるが、比較のEnd在庫へ足さない。
nested quality のoutcomeとAgency framingはmodel-wideの集合なので、未選択processだけの
Endも含む。観測されたterminalsやverify reachableの行も完全な字句集合ではない。
metrics無しの限界を、この入力群で検証済みとは扱わない。推測で試験の在庫を増やさない。

### 観測記録 (今回の有限の試験のみ)

production基点0fe6226cを保ち、照合器だけを強めた。全段check.shは実行しない。

| 対象 | 初回 | 最終の観測 | 根拠 |
|---|---|---|---|
| R1 record正wireと8負対 | 1 GREEN / 8 RED、exit101 | CLI受入の全100本GREEN | 当時の故障・復元ログ（別途保存） |
| record / Rule / H14 / Held / Overflowの手表 | Held/Overflowの手期待を上記理由で訂正 | core26 GREEN、CLIの共有受入もGREEN | b7postfix-finalgreen2-0.log / b7postfix-retry-0.log |
| m4 全done行削除 / 未観測never行削除 | 各RED、正wire GREEN | 両modeで各Err、正wire Ok | b7postfix-m4red-0.log / b7postfix-finalgreen2-1.log |
| m3 一致horizonの私有対照 | 1 GREEN | 1 GREEN (両modeでNone/None・5/5) | b7postfix-private-0.log |
| m3 不一致horizonの内部受入 | **4 RED、exit101** | **当時の失敗観測** | 同ログ。None/5と5/6、網羅と標本の各々でtablesがErrにならない |
| 強化照合器で既存fixtures | invalid比較の5入力が誤ってRED | 106 GREEN、exit0 | b7postfix-regression-0.log / b7postfix-retry-1.log |

invalid比較の5入力は実行reportにqualityが残っても比較の構築が未実施なので、
quality表が空で正しい。行輸送の期待をproperty_tablesがcheckedのときに限定し、
未実施なら空表のみを許すよう訂正した。正しいproductionの欠陥とは扱わない。
新しい専用の正負対でこの境界も固定した。byte互換の射影を意味照合へ足していない。

私有m3は既存evaluation_review_private_compareのseam経由で実行済み。未接続とは
報告しない。逆順5/Noneは同じ試験内で最初のNone/5の失敗より後なので、現基点の
RED観測では未到達。実装の修正後にはそこまで通ることを要求する。

m1の閉表と実入口の故障対を含むevaluation_compatibilityは5本、quality_compatibilityは
2本、rule_actor_compatibilityは5本、agency_second_half_compatibilityは4本が各1回GREEN。
ログは順にb7postfix-regression-1/2.log、b7postfix-retry-2.log、b7postfix-regression-4.log。
旧manifestの内容と旧fixtureの期待は変更していない。Rule入口の手wire作成中には
Rustマクロの括弧とclosureのmove漏れを直した (初回compile REDを意味論REDと数えない)。
最終のfmt / clippy --workspace --all-targets -- -D warnings / check-refs.pyは全てexit0
(b7postfix-finalstyle-0/1/2.log)。私有m3の4REDを含む全workspace GREENは主張しない。
