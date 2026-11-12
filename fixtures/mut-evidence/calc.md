# M-E: 独立根拠の削除

ADR 0009、ADR 0020本文・追記、設計 v0.2 §22.2。
Reader.enterのInvoiceとClerk.match_poのPurchaseOrderがreadsの二箇所。
R二箇所をEvidenceとoriginal_access_removedの二種で各々編集するので4体、G0。
poはpostの引数なのでJ。元subject={Invoice}、independent={PurchaseOrder}。
Clerk.readsを一つ外す体はindependent={}、Evidence detected。
Reader.readsを外す体はsubject={}、PurchaseOrderは残り減少しないのでundetected。
責務宣言もcontestも無いため原本種の両体はundetected/no_assigned_check。
元も全変異体もsafety satisfied、終了3min、検出は業務違反でなく系譜減少。
run complete、六種unsupportedにより全体unsupported/4。
手書き体は対象の reads [PurchaseOrder]; または reads [Invoice]; の一節だけを削除。
