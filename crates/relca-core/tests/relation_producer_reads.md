# ADR0051 読取りの根の制約と IR 全型表

基点2237c2d0の `ir.rs` を列挙した。選択 process 全体の字句上の
結果 binding 参照だけを数える。死枝・End後も含む。生産者は通常 Call の
Actor.Capability、比較は両側で読まれる同名 binding の積集合である。
実行時の採用・到達・共有stateの reaching definition の保証ではない。

| IR の型 / 欄 | 結果bindingを読むか | 理由 / 再帰先 |
|---|---|---|
| Stmt::Wait(Tick) | 読まない | 固定時間だけ |
| Stmt::Call(Call).args | 読む | 実引数、ただし CURRENT_CASE は案件IDで結果でない |
| Call.binding / capability / span | 読まない | 定義 / 生産者 / 位置 |
| Stmt::Fork.id | 読まない | fork識別子 |
| Stmt::Fork.branches | 各Call.argsを読む | 各bindingは定義。forkだから無条件に読むわけでない |
| Stmt::Fork.join | Joinの表による | 下記 |
| Stmt::If.condition | Condの表による | 下記 |
| Stmt::If.then_body / else_body | 再帰 | 到達可能性で枝を捨てない |
| Stmt::End(EndId) | 読まない | 終端名。後続の走査は止めない |
| Cond::Eq(ResultMatch).binding | 読む | 結果値を比較 |
| ResultMatch.value | 読まない | 比較定数 |
| Cond::And / Or | 全子へ再帰 | 短絡評価の実行保証でない |
| Cond::StateEq(StateMatch) | 読まない | 観測stateの比較、結果bindingでない |
| Cond::IntCompare(variable,operator,value) | 読まない | stateと定数 |
| Cond::Contains(set,element) | 読まない | state集合と案件ID/parameter。Element::CurrentCase / Parameterはprocess結果でない |
| Join::All | 読まない | 完了を待つだけ、値の照合無し |
| Join::Any.matches[].binding | 読む | where result の採用条件が各枝の結果を照合 |
| Join::Any.tie | 読まない | Tie::Byは優先順位のID、Tie::Nondetは選択。値を照合しない |
| Join::Any.remaining | 読まない | Remaining::Drain / CancelThenDrainは残枝の処置 |
| Join::Any.if_none | 読まない | 不採用時の終端名 |

本体との照合で欠けていた読む位置は Join::Any.matches だけだった。
Call引数とIfのEq、And/Orは既存の本体が数える。stateの三型を結果読取りに
昇格させない。Capability内部・契約・異議合成routeはこのprocess表の外。

## 手計算 (production diffを呼ばない)

最小対は Before の x=A.one / spare=A.two を After で交換する。
提供されたレビュー1の案はno_match終端の宣言が無くlowerがE-NAME-0になる。
受入では両processのEnd後suffixに `end no_match` を補い、元のjoinと
字句上の読取りを変えずに両側lower成功を先に要求する。
両側に join any f where result == pass を置くと読む集合は {spare,x}。
既存行0、新行は辞書順 spare:A.two→A.one、x:A.one→A.two の2行、
intent/注記無しなのでW-DIFF-1/W-DIFF-3各2。逆向きはpayloadだけ逆。
同じ生産者なら0、両側join allなら読む集合が空で0。
change relation x reason transferはxだけreason=transfer、W1=1/W3=2、
E-DIFF-0無し。tie nondetでも2、tieの順を変えるだけなら生産者同値で0。

独立oracleはこの全型表を実装し、productionのShape/走査関数を使わない。
先に全Statementを平坦化し、各定義bindingについて全読取り位置を照会する
二段の関係照合である。未知のStmt/Cond/Joinはpanic、無視する既定枝は無い。
互換censusとinline367は同じ規範表を共有するがproductionとは独立である。

## 取り込み前の閉じた追加許可表

基点2237c2d0をoffline buildしたbinaryのSHA256は
`9e70b81540dba9b391c62621bb58f3408d45008bc48995d71a1cc40d3cc306ff`。
canonical2397のうちdiff全108をこのbinaryで機械的に列挙・実行し、独立IR表と
全producer payloadを照合した。join条件の追加で行が増えるcanonical jobは0。
次の**追加許可0**は手で固定した値であり、新出力との差から許可を学習しない。
既存 `relation_producer_compatibility.json` の20行・W1=20/W3=18は不変。

| 閉じた母集団 | 入力数 | 既存producer行 | 今回追加を許す位置・行・警告 |
|---|---:|---:|---|
| canonical完了diff | 102 | 20 | 空集合(0) |
| canonical拒否/Cut不一致diff | 6 | 0 | 空集合(0) |
| 非diff | 2289 | 対象外 | 空集合(0)、既存byte gateを維持 |
| 既存inline正例 | 48 | 49 | 空集合(0) |
| 既存inline比較済み0行 | 232 | 0 | 空集合(0) |
| 既存inline静的拒否 / Cut不一致 | 70 / 17 | 0 | 空集合(0) |

全108の独立censusと保存した旧20行の等値は新しいcore testで恒久検査する。
既存inline367も新oracleで再集計し、48+319=367、行49、W1=47/W3=46を維持する。
367は旧入力表の閉包であり、今回の新しいjoin最小対を367へ混ぜて件数を偽らない。
新しい手計算例は別のcore/CLI受入で、2行と0行の対を検査する。

互換の採り直しでは、同じPRE-K1 binaryによる元manifest再採取と
修正前HEAD binaryによるdiff108列挙を区別する。今回のHEAD108では追加許可0、
元PRE-K1表にも新oracleによる変更は0だった。新production出力で表を作り直さない。
