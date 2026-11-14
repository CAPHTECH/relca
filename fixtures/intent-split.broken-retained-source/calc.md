# 元の呼出しが残る部分置換

単一故障はBのwの呼出し先をAudit.logからClerk.checkへ戻したこと。Aは6分、Bは1+2+1+1=5分。
Clerk.check removedが無く、E-DIFF-0 / invalid / exit 3。覆いは0。
Botのactor/relation/authority/resource各1、Docの{Clerk}→{Bot,Clerk}、vの生産者の計6行。
W-DIFF-1/2/3=6/0/6。wの生産者は両側Clerk.checkなので差分は無い。
