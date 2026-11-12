# F1 単一故障: receipt の根拠源を再利用

基準は ../agency-lineage/calc.md。変更は match_receipt の reads 内の
ReceiptRecord を PurchaseOrder に置換する1箇所だけ。via model は対象外。

S(po)=S(receipt)={Invoice,PurchaseOrder}, S(approval)={Invoice}。
全対象 {Invoice}、独立源は {PurchaseOrder},{PurchaseOrder},{}。
3判定2Actorでも和集合は {PurchaseOrder} の1源。
O は Clerk.direct={PurchaseOrder}, Clerk.indirect={Invoice}、
Ledger.indirect={Invoice,PurchaseOrder}。ReceiptRecord の空行は保持。

拒否の文脈は po:{(receipt=pass,approval=pass)}, receipt:{(po=pass,approval=pass)},
approval:{(po=pass,receipt=pass)}。各 fail が止める all_paths/0。
PPPだけ done5/5、他7組 rejected4/4、result_at=null、truncated=false、satisfied/exit0。
独立源の和集合2→1を検出し、警告は期待しない。期待は手計算から固定した。
