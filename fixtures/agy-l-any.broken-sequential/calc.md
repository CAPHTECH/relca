# A12 agy-l-any.broken-sequential

Hand oracle before implementation (ADR 0034; design v0.2 §5.6, v0.2 §22.3).

Any success pp,pf,fp reaches done; ff reaches if_none returned. Both lexical siblings count once including cancellation, hence both 1..1 and flags true. Sequential short circuit makes B 0..1, no cancellation. Old DSL registers returned via an End after done; the reachable returned sink belongs to if_none, not that unreachable suffix. The IR-only case without a registering End is a separate private obligation.

This side: satisfied; exit 0; diagnostic codes in report order/multiset: []. No new Agency diagnostic.

All ordinary durations are fixed minutes. P is always not_derived. L basis unknown, fallback unchecked; zero exercise does not imply unnecessary. Empty derived collections differ from not_derived. The expected viewpoints payload is complete, ordered and independent of actual output.
