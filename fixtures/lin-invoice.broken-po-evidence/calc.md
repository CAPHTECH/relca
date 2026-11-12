# lin-invoice.broken-po-evidence — hand oracle before implementation

The calculation uses lexical call identity and source ancestry.
This is a lexical structural analysis, not proof of real-world independence.
Warnings do not alter the root verdict. No schedule is replaced by a random choice.
The mutation wrapper keeps W-LIN-2; Report.agency.unchecked does not.

F1: G(entry)={poV,receiptV,approvalV} by writer call identity, not raw subject_sources. I(poV)={PurchaseOrder}, I(receiptV)={ReceiptRecord}, I(approvalV)={}; args containing the other results do not declare given correlation. Four selected kernel users of Invoice.difficulty (ai_entry is unused).

Wrong post = (4/5)(1/100)(1/5)(3/20)(1/10) + (1/5)(3/50)(1/2)(2/5)(3/10) = 93/125000 < 1/1000.
Correct extraction = (4/5)(99/100)+(1/5)(47/50)=49/50. Posted=49/50+93/125000=122593/125000; returned=2407/125000. Posted 6+3+5+2+1=17 min/work. Return after checks at14 or approval16 min/work. All probabilities per arrival, not renormalized.
Fault: remove only match_po reads PurchaseOrder. I(poV) becomes empty, adds poV warning. All probabilities and durations unchanged.

Exact W-LIN-2 call inventory: ["poV", "approvalV"].
No other warning is expected (explicit Cut; no committing effect).
Every Task has positive duration; serial durations add to work and elapsed.
All routes finish before clock 30 min, safety satisfied, no truncation.
No oracle is copied from a new binary. Extra boundary cases live in dedicated tests.
