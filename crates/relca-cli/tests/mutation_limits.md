# Mutation の上限と選択の手計算 (ADR 0068)

`support/mutation_limits.relca` は呼ばれない Probe.read に reads [A,B] がある。
到達可能性で箇所を減らさない既存の規則により、evidence_missing と
original_access_removed はそれぞれ A と B の 2 候補。合計は 2 + 2 = 4。
if、authority_basis、2 消費者を持つ作る側、commit、cohort は無いので、
対応済みの他種は 0 候補。未対応の規則版と品質分母は列挙できず null。

既定の全種では limit 3 が最後の原本削除を not_run にし、completion は
inconclusive、reason は mutant_limit。4 ちょうどと 5 は complete。
未対応種があるため verdict は既存の優先順どおり unsupported (exit 4)。

原本削除だけを選べば予算は 2。1 は inconclusive (exit 2)、2 と 3 は complete。
この模型には責務や異議経路の割当ての検査が無いため 2 つとも未検出で exit 1。
選外の Evidence は 2 候補と読めるが実行せず not_selected、unchecked に残る。
Authority だけを選べば該当なしで complete、vacuous (exit 5)、reason は
no_selected_mutant。これはfoldがsatisfiedになる空集合だけの裁定である。未対応の
規則版だけを選べば unsupported (exit 4)。選択は切断ではない。

空リスト、空要素、重複、未知名、余白は invalid。上限は ASCII 数字のみ、
1..=4096。指定しなければ 128。ledger の受理した計算上限も同じ整数になる。

## 別の列挙器の境界

`crates/relca-core/tests/mutation_options.rs` は以下も照合する。

- `mut2-summary`: reads が2つなので Evidence と原本削除で2+2、終端 guard が1つなので
  Hold と拒否で1+1、e の消費者が p と a の2つなので2番目の付け替えが1。
  合計7。要約だけなら必要予算は1で、残る6候補は実行しない。
- `mut2-expiry`: authority_basis が名指す ApprovalGrant が1つ。予算1で検出。
- 応答喪失の共通模型 `service(g, none)`: 明示した commit の箇所は1つで、再送と
  unknown はそれぞれ1候補。unknown だけなら1で完了、両種なら1では後者が not_run。
  再送は G 形の原子的なキー重複排除により at_most_once が保たれるので未検出。
  unknown だけの変異では受領が未解決になることを割当ての検査が検出する。
- 分母の小模型: distinct な end yes と end no があるため、arrivals の分母から
  外す候補は2つ。予算1で1つが not_run、2で完了。選外でも2と表示する。
- 元の構文が不正な場合と読めない入力: 列挙を始められないので全候補数と合計は
  null。使用上限と not_selected の範囲だけは残す。

追加の応答模型の最初の期待では再送を detected と書いて失敗した。
これは本変更の契約ではなく、既存の比較の手計算を誤ったものだった。
キーの重複排除で割当ての性質が悪化しないことから undetected に訂正した。
候補数・予算・選択・切断の期待は変更していない。
