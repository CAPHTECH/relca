# A18 agy-invoice.broken-ai-transfer

Hand oracle before implementation (ADR 0034; design v0.2 §5.6, v0.2 §22.3).

Only AsIsFlow entry producer changes; parallelization is NOT part of this fault. Direct Invoice moves Clerk c1,c2 → anonymous Extractor, but contest c1,c2 and matching sources remain. Indirect Invoice persists through matching. Counts enter/extract 1/0 → 0/1; approval/post 0..1. posted requires all three pass, returned permits a new approval binding to fail (existential Q4): poV/receiptV read_only, approvalV some_contexts. committed is singleton so exception unreachable. Error mass AsIs=93/125000=.000744; AI=231/125000=.001848 versus .001. Roots satisfied/0 → violated/1. All joins are all, flags false.

This side: violated; exit 1; diagnostic codes in report order/multiset: ['W-LIN-2', 'W-RESP-1', 'W-RESP-0', 'W-RESP-0', 'W-UNREACH-0', 'W-UNREACH-1', 'W-UNREACH-2']. No new Agency diagnostic.

All ordinary durations are fixed minutes. P is always not_derived. L basis unknown, fallback unchecked; zero exercise does not imply unnecessary. Empty derived collections differ from not_derived. The expected viewpoints payload is complete, ordered and independent of actual output.
