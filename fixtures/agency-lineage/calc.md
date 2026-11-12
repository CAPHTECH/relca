# F1: 3 判定、2 Actor、独立な根拠源 2

ADR 0009 と計画の第4節の手計算。実装出力は使用していない。時間単位は分。

系譜: S(entry)={Invoice}; S(po)={Invoice,PurchaseOrder};
S(receipt)={Invoice,ReceiptRecord}; S(approval)={Invoice}。
J は実引数非空かつ posted の祖先にある po,receipt,approval。
対象は全て {Invoice}。独立源は順に {PurchaseOrder},{ReceiptRecord},{}。
和集合は2源、判定数3、異なる Actor は Clerk と Approver の2。

O: source3行は direct/indirect とも空。Reader は direct={Invoice}。
Clerk は direct={PurchaseOrder,ReceiptRecord}, indirect={Invoice}。
Approver は direct={}, indirect={Invoice}。
Ledger は direct={}, indirect={Invoice,PurchaseOrder,ReceiptRecord} (祖先を2hop以上伝搬)。

拒否: Reach(posted)=po.pass AND receipt.pass AND approval.pass。
文脈は po: {(receipt=pass,approval=pass)}, receipt: {(po=pass,approval=pass)},
approval: {(po=pass,receipt=pass)}。他二値の残り3組は E に届かないため分母外。
各1文脈で自身を fail に変えると全経路が不達。全て all_paths/0。
無引数 entry は J でなく、効果自身 posted もこの効果の J でない。

結果8組 PPP,PPF,PFP,PFF,FPP,FPF,FFP,FFF は確率ではない。
前処理4呼出し=elapsed/work4。PPPだけ登録1を足し done5/5、他7組 rejected4/4。
result_at は null。容量は逐次で高々1、clock32・live_tasks4 は切断なし。
reachable done と safety は satisfied、全体 satisfied/exit0。W-CUT-0 のみ必要、W-AGY-0/1 不在。
