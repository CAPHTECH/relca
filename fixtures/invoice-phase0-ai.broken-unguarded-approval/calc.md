# 請求書例(Phase 0 の文法で書ける範囲)の手計算

設計 v0.2 §21 の初期受入として、一つの模型で逐次・並列・AI抽出の案を比べ、原本のreader、停止する経路、source集合の差を示す。v0.2 §12 の例を初期文法へ縮小し、実行者別の人・分と確率は出さない。時間の単位は分、結果はnondetの全候補を展開する。

## 時間と作業量(ADR 0001: 1 tick = 1 分、作業量は資源占有の和)

| 構成 | 経路 | done(経過 / 作業量) | rejected(経過 / 作業量) |
|---|---|---|---|
| AsIs(Serial) | enter 0→5、po 5→8、receipt 8→11、approve 11→14、(全 pass なら)post 14→15 | 15 / 5+3+3+3+1 = 15 | 14 / 14 |
| ToBeP(Parallel、Clerk capacity 2) | enter 0→5、po と receipt を 5→8 で並行、approve 8→11、post 11→12 | 12 / 15 | 11 / 14 |
| ToBeA(Extracted) | extract 0→1、po と receipt を 1→4 で並行、approve 4→7、post 7→8 | 8 / 1+3+3+3+1 = 11 | 7 / 10 |

rejected は po・receipt・approval のどれかが fail の経路(全 8 組のうち 7 組)。どれも同じ時刻で終わるので min = max。result_at は join any を使わないので null。性質: reachable done(全 pass の組)、reachable rejected(fail を含む組)、safety(Clerk は同時 2 件まで、他は 1 件)はすべて satisfied。

## Agency(ADR 0009。3 構成とも同じ導出の手順)

- **系譜:** 判定は実引数を取る呼出しのうち効果 post に効くもの = po、receipt、approval(enter / extract は引数が無いので生産者)。対象の根拠源は entry の祖先 = {Invoice}(enter も extract も reads [Invoice])。独立な根拠源 = reads − 対象: po {PurchaseOrder}、receipt {ReceiptRecord}、approval {}。対象 {Invoice} の 1 群に判定 3・Actor 2(Clerk、Approver)・独立な根拠源 2(PurchaseOrder、ReceiptRecord)。**判定 3 を独立検証 3 と数えない**(設計 v0.2 §22.1「系譜の二重計上」)。
- **O:** 行は宣言した 7 Actor。direct は選んだ process の呼出しの reads、indirect は実引数の祖先の根拠源。Ledger.post(po, receipt, approval) の祖先 = {Invoice, PurchaseOrder, ReceiptRecord}。
  - AsIs / ToBeP: Clerk direct {Invoice, PurchaseOrder, ReceiptRecord}(enter・match_po・match_receipt)、indirect {Invoice}。Extractor は呼ばれないので direct {}。
  - ToBeA: Clerk.enter が呼ばれないので Clerk direct {PurchaseOrder, ReceiptRecord}、Extractor direct {Invoice}。Invoice の direct 集合は Clerk から Extractor へ変わる。人の実行者単位は出さない。Approver は 3 構成とも indirect {Invoice} だけ。
- **U:** 効果は post(commits)。post へ至る充足可能な経路は 1 本で、条件は po = pass ∧ receipt = pass ∧ approval = pass。
  - po の文脈 = その経路で束縛される他の条件の束縛 (receipt, approval) の値 = (pass, pass) の 1 つ。po = fail でどの経路も post へ進まないので止める。止められない文脈は無い → all_paths / 0。receipt と approval も同じ。

## 故障: ToBeA の条件から approval を外す(設計 v0.2 §22.2「拒否経路の切断」)

- approval は post の実引数に入るので判定のまま。post へ至る経路の条件は po = pass ∧ receipt = pass で、approval を読まない。
- approval の文脈 = 条件が読む他の束縛 (po, receipt) の値 = (pass, pass) の 1 つ。approval がどの値でも post へ進む → none / 1。ToBeA の report に W-AGY-0(判定するが止めない)。警告なので結果は satisfied のまま。
- 時間: rejected は po か receipt が fail の組(4 組のうち 3 組)だけになるが、同じ時刻(7 / 10)で終わる。done 8 / 11。approval = fail でも post へ進む組が done に入る。
- 検出: agency.to の approval の stops / bypasses と、to 側の W-AGY-0。
