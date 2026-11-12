# M-O: 直接到達を間接で代用しない

ADR 0018本文・追記、ADR 0009・0011、ADR 0020追記。
Reader/ApproverのR二箇所×二種=4、G一箇所×二種=2、計6体。
Approver.readsだけ削除してもReader入力経由のInvoiceは残る。
required_sources={Invoice}不変、direct={Invoice}→{}、observation present→missing、
missing_sources={}→{Invoice}、W-RESP-1新規。原本種detected。
Evidenceの独立集合は元から空なので同体はundetected。
Reader削除はApproverの直接Invoiceが残るのでO悪化なし、未検出。
G削除はU all_paths→none、拒否種detected。Holdは契約なしで未検出。
元satisfied、run complete、全体unsupported/4。手書きR体はApproverのreads一節だけ削除。
