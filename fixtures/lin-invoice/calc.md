# lin-invoice — hand oracle before implementation

## M1 boundary derivation (ADR 0024 follow-up)

E(poV)={Invoice,PurchaseOrder}; E(receiptV)={Invoice,ReceiptRecord};
E(approvalV)={Invoice,PurchaseOrder,ReceiptRecord}. Approval consumes both
judgement results, so it is excluded from the other-judgement union when
testing either poV or receiptV. Each of those then has a unique record source;
neither warns. Approval excludes no other judgement, and its source set equals
their union: approvalV alone warns. This is not a contradiction in the ADR.

The reread reproduction changes only approval's signature/actual args to entry
alone and adds reads [PurchaseOrder]. Then E(approvalV)={Invoice,PurchaseOrder};
it no longer descends from poV. Therefore BOTH poV and approvalV have identical
sources, and both are contained in the other-judgement union. This yields TWO
warnings under the literal M1 rule, not the requested one-warning reread oracle.
The reread expectation is [poV,approvalV], with no change to the source-set rule.

This is a lexical structural analysis, not proof of real-world independence.
Warnings do not alter the root verdict. No schedule is replaced by a random choice.
The mutation wrapper keeps W-LIN-2; Report.agency.unchecked does not.

F1: G(entry)={poV,receiptV,approvalV} by writer call identity, not raw subject_sources. I(poV)={PurchaseOrder}, I(receiptV)={ReceiptRecord}, I(approvalV)={}; args containing the other results do not declare given correlation. Four selected kernel users of Invoice.difficulty (ai_entry is unused).

Wrong post = (4/5)(1/100)(1/5)(3/20)(1/10) + (1/5)(3/50)(1/2)(2/5)(3/10) = 93/125000 < 1/1000.
Correct extraction = (4/5)(99/100)+(1/5)(47/50)=49/50. Posted=49/50+93/125000=122593/125000; returned=2407/125000. Posted 6+3+5+2+1=17 min/work. Return after checks at14 or approval16 min/work. All probabilities per arrival, not renormalized.

Exact W-LIN-2 call inventory: ["approvalV"].
No other warning is expected (explicit Cut; no committing effect).
Every Task has positive duration; serial durations add to work and elapsed.
All routes finish before clock 30 min, safety satisfied, no truncation.
No oracle is copied from a new binary. Extra boundary cases live in dedicated tests.
