# F1 単一故障: Pの先頭callだけAIへ

AsIsFlowは不変、ToBePFlow先頭のClerk.enter()だけExtractor.extract()へ置換。intent無しも不変。
人の誤登録は(4/5)(1/100)(1/5)(3/20)(1/10)+(1/5)(3/50)(1/2)(2/5)(3/10)=93/125000。
AIは routine (4/5)(1/50)(1/5)(3/20)(1/10)=6/125000、difficult (1/5)(3/20)(1/2)(2/5)(3/10)=225/125000。
和231/125000>閾値125/125000。全schedulerでmin=max231/125000、to確率性質/root violated、diff violated/exit1。
正入力AI=(4/5)(49/50)+(1/5)(17/20)=477/500、posted=477/500+231/125000=119481/125000。
returned=1-posted=5519/125000。各難度の誤入力×[1-po×receipt+po×receipt×(1-approval)]の和とも一致。
人側posted122593/125000、returned2407/125000。完了者だけで再正規化しない。

時刻: AI入力0→1、po1→4とreceipt1→6、承認6→8、登録8→9。
posted elapsed9/work1+3+5+2+1=12、returned elapsed[6,8]/work[9,11] 分。
AsIsはposted17/17、returned[14,16]/[14,16]。全result_at=null。safety satisfied、切断なし。
静的差分はactor Extractor追加、relation Extractor.extract追加、relation Clerk.enter退役の3件。
各reason/loses null、intent無しなのでW-DIFF-1を出す。Clerkは照合で残りActor退役でない。
retire主張の行自体が無いのでW-DIFF-2無し。Cut共通、W-CUT-0無し。期待は手計算のみ。


## ADR 0024 の移管(2026-10-24)

各案の報告に W-LIN-2 が 1 件ずつ出る。Approver.approve(entry) の kernel approval_check は対象の誤り extraction_correct(entry を書く Clerk.enter が書く)を given に置き、他の判定の結果を given に置かず、reads に根拠源が無い(independent_sources が空)。同じ対象の判定は match_po・match_receipt・approve の 3 つ。match_po と match_receipt は PurchaseOrder・ReceiptRecord を読むので該当しない。
