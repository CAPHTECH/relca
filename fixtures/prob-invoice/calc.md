# F1: As-Is と非AI並列化

ADR 0013 と設計 v0.2 §12.5 の説明用仮定。受付1件が分母、登録条件付き率ではない。
難度は受付で一度だけ routine4/5・difficult1/5。各抽選は given の下で条件付き独立。

- human/routine誤登録: (4/5)(1/100)(1/5)(3/20)(1/10)=3/125000。
- human/difficult誤登録: (1/5)(3/50)(1/2)(2/5)(3/10)=90/125000。
- 和93/125000、閾値1/1000=125/125000に対し93<=125。min=max、確率性質 satisfied。
- 正入力: (4/5)(99/100)+(1/5)(47/50)=49/50。
- posted=49/50+93/125000=122593/125000、returned=1-posted=2407/125000。

returned は誤入力後、二照合の少なくとも一つがfail、または両pass後承認fail。
難度dごとに err(d)×[1-po(d)receipt(d)+po(d)receipt(d)(1-approval(d))]、難度の重みを掛けて足す。
正入力では全検査pass。両照合が異なる変数だけを書き、難度/真値は照合中変化しないので、全schedulerで同じ確率。
各終端min=max、safety/root satisfied、exit0、切断なし。

時刻(分): AsIs 入力0→6、po6→9、receipt9→14、承認14→16、登録16→17。
posted elapsed/work17/17。returned は照合後14または承認後16、elapsed/work[14,16]。
P は入力0→6、po6→9とreceipt6→11、承認11→13、登録13→14。
posted elapsed14/work6+3+5+2+1=17。returned elapsed[11,13]/work[14,16]。
result_atは両側null。Cut共通、Actor/Capability集合不変、differences空、W-DIFF-1/2無し。
Agencyの構造上の導出と確率の到達性を同一視しない。実装出力から期待値を採取していない。


## ADR 0024 の移管(2026-10-24)

各案の報告に W-LIN-2 が 1 件ずつ出る。Approver.approve(entry) の kernel approval_check は対象の誤り extraction_correct(entry を書く Clerk.enter が書く)を given に置き、他の判定の結果を given に置かず、reads に根拠源が無い(independent_sources が空)。同じ対象の判定は match_po・match_receipt・approve の 3 つ。match_po と match_receipt は PurchaseOrder・ReceiptRecord を読むので該当しない。
