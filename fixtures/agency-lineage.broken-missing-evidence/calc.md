# F1 単一故障: match_po の reads 項目を削除

基準は ../agency-lineage/calc.md。実装出力から期待を写していない。
モデル変更は reads [PurchaseOrder] の削除1箇所だけ。

系譜 S(po)={Invoice} となり po の独立源は空。receipt の独立源は ReceiptRecord のまま。
対象 {Invoice} の3判定2Actor群の独立源和集合は {ReceiptRecord}、長さ1。
O は Clerk.direct={ReceiptRecord}、Ledger.indirect={Invoice,ReceiptRecord}。
source Actor 行を消さず PurchaseOrder の空行も残す。

Reach は po.pass AND receipt.pass AND approval.pass のまま。
文脈は po:{(receipt=pass,approval=pass)}, receipt:{(po=pass,approval=pass)},
approval:{(po=pass,receipt=pass)}。各 fail が止めるので all_paths/0。
PPP の done5/5、他7組 rejected4/4、result_at=null、truncated=false、satisfied/exit0。
検出は独立源2→1と到達集合の減少。系譜欠落に警告を捏造しない。W-AGY-0/1 不在。
