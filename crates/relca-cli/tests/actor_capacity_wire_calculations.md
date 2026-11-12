# Actor 容量0の理由と照合器の手値

ADR 0043 の Actor 容量0追記に従う受入。製品の導出や snapshot から期待を採取しない。
試験は `agency_viewpoints.rs` の `actor_capacity_*`。

- `agy-f-select` の手表では Checker.check の j が done と returned の両方を選ぶ。
  returned の F 行だけを未導出にした手組み wire は complete=false、actors=[Checker]。
  done の selector が残るからである。両行を未導出にすれば selectors の和は空であり、
  actors=[]。欠落・余分な Actor、complete=true、未導出行に残した reachable は拒む。
- L の未導出行は normal を持たず、保持の basis=unknown、fallback=unchecked、
  contest_route=false、join_any_cancellation=false を保つ。normal=0..0 を足す形は拒む。
  block 全体の失敗は derivation_limit のままであり、O/P/F/L の block 理由に新語を許さない。
- diff では F の reachable/selectors/read_only/actors と L の四項目にのみ、
  not_comparable の側の not_derived 理由として新語を認める。L の値が元の行単位で撤回されるため、
  normal 以外の項目にも同じ理由が運ばれる。O・origin・effects・refusal・lineage の同形は拒む。
- capacity=1 の Checker しかいない模型は理由の必要条件を満たさない。
  check/simulate の F/L、diff の両側の Agency コピーと差分行、compare の Agency 対に、
  型としては正しい新理由を入れても、Actor 表に容量0がないので拒む。
  これは必要条件の検査であり、行の正確な依存解析の再実装ではない。

## 既存失敗12入力の expected.json

以下12入力の expected.json に F/L の位置はない。したがって旧数値を新しい未導出の意味へ
写す変更は不要であり、模型・expected.json・calc.md は変更しない。
閉じた wire の語彙を追従させ、既存の独立した時間・確率・性質・診断の手値を維持する。

- done-nondet.broken-nondet-option
- done-probability.broken-block-failure
- done-recovery.broken-zero-capacity
- done-support.broken-positive-support
- explain-deadlock.broken-zero-capacity
- resource-contention.broken-zero-capacity
- req-no-capacity.broken-zero
- lim-deadlock-held.broken-zero-capacity
- qtl-deadlock.broken-zero-capacity
- qual-deadlock-single.broken-zero-capacity
- qual-deadlock.broken-zero-capacity
- sim-deadlock.broken-zero-capacity

done-support のみ、Judge の done 行が残り、容量0の Blocked に依存する bad 行が撤回される。
その F の集計は残る done の selector の Actor、Judge だけになる。
他の入力の全終端行が撤回される場合は actors=[]。
