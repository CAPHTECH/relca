# ADR 0047: 生き残りの故障と遅延停止の手計算受入

段階 C。期待は下記の算術と ADR 0047 の意味、裁定 m1・m2、
固定した手値を使う。採取した実装の出力や一時記録はテストの入力にしない。
既存の閉じた116 fixture と互換 manifest は変更せず、専用 core / CLI target
`evaluation_mutation_fixes` で同じ手の oracle を実行する。
入力は既存 fixture への回数を検めた編集と、小さな独立の Probe である。

## O-nulldom

`cmp7-cost.broken-missing-rate` の目的を費用1行に限定する。
Check は各案件 Checker.check の開始単価が無いので、費用は null / not_configured。
Fast は2件とも quick 1分 + post 1分、占有1円/分、開始0円なので総額4円。
verify は両案 satisfied、hard は無いので両案 admissible。
確実な支配には両側の全目的の値が要る。従って non_dominated は宣言順
[Check, Fast]、dominated は空、対は missing_value。root satisfied、exit0。
対照は check の開始単価0円を明示する編集。Check の費用は6..8円、
Fast は4円で厳密に安く、Check は Fast に支配される。

## O-bwunull

`cmp7-clock` を到着[0]、cases1、clock3分、WrongPerRegistered <= 3/10 にする。
両configurationとも Flow。Checker.check は4分なので3分では未完了、
登録も誤登録も未確定[0,1]。分母下界0なので有限な比の上界を作れず、
value の両端も bounds_with_undetermined も null、理由 truncated。
hard の両セルは inconclusive、interval/basis は null、reason truncated。
採否は undetermined、理由は verify inconclusive と hard inconclusive/truncated。
root inconclusive、exit2。対照は clock30分。登録確率23/25、誤登録1/50、
比(1/50)/(23/25)=1/46 <=3/10 で hard satisfied、root satisfied、exit0。

## O-cachekey

`cmp7-case-multiplicity` を clock12分、上限19/20にする。
1反復・confidence1/2、2案×1行×2案件×1区間で m=4、
両端の alpha=(1-1/2)/(2×4)=1/16。全kernelは correct=true。
Good の第1件は4分に完了して Wrong の hits0/unknown0、第2件は
10分到着、check が13分まで掛かるため12分で hits0/unknown1。
Bad は2分と12分に完了し、両件 hits0/unknown0（clock一致の完了を含む）。
1反復の CP 上端は既知missなら1-alpha=15/16、unknownなら1、下端は0。
Good は[0,(15/16+1)/2]=[0,31/32]で19/20を跨ぎ inconclusive、
point null、reason truncated。Bad は[0,15/16]で satisfied、point0。
root inconclusive、exit2。対照 clock20分では全件既知miss、両案[0,15/16]、
satisfied、exit0。非自明な抽選や統計的検出力の主張ではない。

## R-m1 / R-m2

既存 eval-hard の網羅と eval-sim の標本の両方で compare.hard を1行だけ探す。
value は conservative-endpoints。affects に verify と hard の族を合わせた
同時のconfidenceを主張しない文と、violated は全schedule・全unknown解決で
違反するという量化を部分文字列で要求する。全文のbyteは固定しない。

## 移した3 returnと架空の対の禁止

Probe: A.run は固定1分、Flow は1回呼んで done。Left は Cut C、Right は
Cut D。C/D は共に include[A]、他の欄も同じで名前だけが違う。
1案件・clock10分・verify reachable done なので各reportは satisfied。

- plans: 宣言[Right,Left]、選択[Left,Right]。E-CONF-0 の後に案間 E-CMP-0。
- under: 宣言[Left,Right] under[Allowed]、horizon10分の Outside を明示選択。
  実在案のlowerは両方成功する。E-CONF-0 の後に同じ案間 E-CMP-0。
- cut: 宣言[Left,Right] cut C。宣言 E-CMP-0 の後に案間 E-CMP-0。

3つとも mismatch.pairs は Left→Right、C→D の1対。名前以外のCut差は空、
終端差はnull。checkedには選択・Cut照合・終端照合があるが評価や表の計算は無い。
root invalid、exit3、evaluation無し。対照は各宣言だけの失敗に同一Cutを
組み合わせたもの。invalid/3のまま、pairsはnullで架空の差を作らない。

Missingを選ぶ対照と、Rightだけ未知Actorを呼ぶBrokenへ変更する対照では、
lower成功した案どうしの完全なinventoryが無いので pairs はnull。
後者は AST から Left のlower成功、Right のlower失敗を直接確認してから
受入する。invalid/3、evaluation無し。部分metadataから対を推測しない。

## 観測の置き場と境界

以下の期待値だけでは、GREEN、各単一故障のRED、src復元の実行を再現したことにはならない。
8518009を基点として、新しいcore14・CLI14試験は各1回GREEN。
生き残り5件と元の位置へ戻した3returnの計8単一故障は、それぞれ対応する
core/CLIの1試験がcompile成功後のassertでRED（各exit101）。毎回src差分空と
元のSHA256へ復元した。これは8件の校正の観測であり、全故障の検出率ではない。
受入はproduction evaluatorの公開結果だけに手の値を照合するもので、
全55故障の再実行、全組合せ、未統合A7/C7/B6 horizonは検証しない。
READMEの並記専用・coreはVerdictのみという説明、designの実装不存在、
fixtures/READMEのB7未実装REDは現コードより古い。
ADR候補#3のeval-horizonという名前は現fixtureに無く、cmp7-clock系列がある。
文書は変更せず現コードと最新裁定の境界に従う。
