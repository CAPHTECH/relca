# 呼出し箇所の分割

レビューのt5cと同じ呼出しを使う。Aはcheckを2回とpostを2回で2+2+1+1=6分。
BはBot、Audit、post2回で1+1+1+1=4分。経過と作業量は6/4分。
Clerk.checkは全て消え、Bot.checkが追加されるのでreplaceの照合は成立する。
ActorはAudit/Bot追加とClerk削除の3行、能力・authority・resourceも各3行、Doc1、producer v/wの2行で15行。
置換だけではv/wの両方がBot.checkからの値になるが、実際のBでwはAudit.logからの値。
Auditの4行、Docの{Clerk}→{Audit,Bot}、wのClerk.check→Audit.logの計6行は残す。
9行を覆い、W-DIFF-1/2/3=6/0/6、satisfied / exit 0。呼出し箇所の対応を勝手に補完しない。
故障対ではBのwを元のClerk.checkへ戻す。
