# Rule 品質の手計算と閉じた移管台帳 (ADR 0052)

この手計算は実装前に書いた。設計 v0.2 §12.2・§12.5・§13.1 と
ADR 0045/0047/0049 の通常意味を使い、実装出力を期待値にしない。

## 核

- 入力の誤りと3判定の見逃しの積を難度で加重すると、人は93/125000、AIは231/125000。
  全流入分母は1で、postedだけに再正規化しない。1/1000に対し前者合格、後者違反。
- 全件免除Bは承認見逃しを掛けないので、(4/5)(1/50)(1/5)(3/20)
  +(1/5)(3/20)(1/2)(2/5)=81/12500。これは設計 v0.2 §12.5 の
  条件付き免除B(not_configured)の数値ではない。
- support/rule_integration_inputs::QUALITY の Hit は初期時点のRule.enabled。
  AsIsはtrueで期待件数1/Rate1、ToBeBはfalseで期待件数0/Rate0。
  終端doneは両方時刻1で確率1。標本4反復はhit4/0、unknown0、trials4。
- 同じ入力の3反復/信頼水準1/2、2案のhardでm=2、alpha=1/8。
  0hitのCP端は[0,1/2]、3hitは[1/2,1]。<=0.6はtrue不明/false支持、
  <=0.4はtrue違反/false不明。点1/0で判定しない。
- quantile_integration の Required も初期Rule値。trueは到着相対0、
  falseは全流入beyond_reached_mass、条件付きzero_denominator、下界なし。
  scenario Laterの到着5を分位点5にしない。legacyでも同じ。
- rule-basicの承認passのみではpostedは2+1=3。failのみでrequired=trueなら
  postedなし。required=falseなら承認failでも3でpostedだが承認invariantは違反。
- HeldのHitは到着直後からtrueで、外部待ちでも期待件数3、Rate1。
  drainでdone率1、horizon3でcut_off率1。Hitは打切りでも既に確定。

## 既存受入の閉じた変更箇所

下表は旧撤回の期待だけを書き換える。テストは削除/ignoreしない。
同じsupportを使うcoreとCLIの双方へ効く。関数名を維持した行は歴史的名称である。

| file | function/期待 | 新期待 |
|---|---|---|
| support/rule_integration_inputs.rs | assert_quality_assumptions | Ruleフラグに依らず通常4行、quality.ruleなし |
| rule_actor_integration.rs(core) | quality_rule_exhaustive/sample、to_be_b_quality、quality_execution | 必須qualityにRate1/0、実行は不変 |
| 同 | quality_rule_disclosure、held_quality | disclosureなし、Hit件数3/Rate1 |
| rule_actor_integration.rs(CLI) | rule_quality、quality_disclosure_wire、quality_day | 通常品質と旧行注入拒否、通常順の壊した対 |
| support/quantile_integration_inputs.rs | rule_expected/assert_rule_report | 実効Ruletrueは0、falseは不達/分母0 |
| quantile_integration.rs(core/CLI) | Rule構成/シナリオ/legacyの呼出し | 構成名を期待へ渡す |
| support/evaluation_integration_acceptance.rs | d04_rule_quality | 網羅Rate1/0、<=1/2のhardは違反/合格。標本4反復のm2/alpha1/80は閾値を跨ぎ両方不明 |
| support/evaluation_quality_table_oracle.rs | rule | 通常の性質/比/結末の手値 |
| support/quantile_onto_inputs.rs | assert_rule/assert_rule_quantile | RequiredRate1/0、>=1/2のhardは網羅合格/違反。標本1反復alpha1/8は両方不明 |
| mutation_integration.rs(core/CLI) | Rule品質/assumptions | 元のDone率は非決定の[0,1]/per_case_relaxation。実行済みReportは通常値、実際の拒否Reportはaborted |
| support/waits_integration_inputs.rs | assumptions | quality.ruleを期待に足さない |
| support/waits_refusal_private.rs | 仮定行の期待 | quality.ruleを期待に足さない |
| waits_mutation_integration.rs(CLI) | quality_rule_swap | 通常品質の順序を壊す対へ移す |
| invoice_phase1_artifact.rs | rule_contract/非atomic対照 | AsIs93/125000、A231/125000、B81/12500、disclosureなし |
| support/invoice_c8_review.rs | rule_quality/f2 | 宣言保持のまま通常数値、各数値を壊す対 |
| CLI src/fixture_match.rs | without_quality | 追加裁定で受入側が旧行必須gateを外す。残存行と成功時の全abortedを別々に拒否し、真の拒否を保持 |

## 新受入

### 取り込み前レビューの手計算 (m3・m4・m5、実行前に固定)

- m3: compare の hard は宣言した cohort.Rate <= 3/5 のまま、目的の順序は
  cohort.Rate、quantile.All のまま。各 values の configuration と順序も固定する。
  変わるのは撤回から復帰する計算セルだけ(区間・判定・根拠・理由・標本点)。
  bound の文字列を 3/5 から 2/5 にした偽出力は、数値が整形式でも拒否する。
- m4: Rule の存在だけによる aborted は許さない。一方 verdict=invalid かつ
  severity=error の E- 診断を持つ拒否は、U-SEM-0 と同様に aborted を許す。
  正常な Rule 出力に aborted を戻した負の対照は引き続き拒否する。
- m5: support/rule_quality.relca の変更権限 G に revocable_by p を足す。
  revocable_by は動的撤回を実行しない記録なので、ToBeB の静的改定 enabled=false は
  有効なまま。初期 Hit は偽で到達しないから N=0、全流入 D=1、Rate=0/1=0、
  expected_count=0。done は時刻1で確率1。全流入分位点は beyond_reached_mass、
  到達条件付きは zero_denominator。網羅 check は satisfied/exit0 で、root unchecked に
  dynamic_authority_revocation を残す。数値を1へ改竄した対と unchecked を消した対を拒否する。
  engine と独立参照を各々この手値へ照合し、CLI も実行する。実際の撤回の保証ではない。

rule_quality.rs(core/CLI)がT1〜T7の核と定数への置換、Ruleだけのモデル、
真の拒否、型/権限/潜在ガードの壊した対を守る。
参照の非依存はsourceからengine reducer参照がないことも検査する。
数値一致のみで独立性や網羅性を主張しない。

rules.relca 自体のconfigurationはToBeA/ToBeBだけである。AsIs/Pの受入は
phase1.relcaへ固定Ruleをinlineで追加しLedger.requiresで読む、明示した変形を使う。
旧5モデルにも新rules.relcaにもconfigurationを追加しない。

## 互換の凍結

旧binary SHA256はbf7b01b81cecf92a47d23736941600b2448ae29fb755d72fb818890d519c3e0b。
4670 ordered job、変更候補14 job、許可54 pointer。
具体的な閉じた表と採り直し手順はCLI tests/rule_quality_compatibility.md/json。
受入先行時点では新54写しを採り直していなかったが、実装後にRuleの3 stdoutだけを更新した。
採取元とSHA256はexamples/invoice/README.md。他の新51ファイルと旧51写しは不変。
取り込み前レビューm3では同じ旧binaryからcompare2件のhard/objectivesの定義だけを追加固定した。
4670 job・14候補・54 pointerと全jobの記録済みhashは維持し、新出力から許可を増やしていない。
K8の9名はacceptance_gap_names.rsの閉じた表で除外する。prefixによる除外ではない。
