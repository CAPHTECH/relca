# A23 agy-no-end-after-end

Hand oracle before implementation (ADR 0034; design v0.2 §5.6, v0.2 §22.3).

O is lexical so Worker reads Invoice even after an unconditional end. F/L are path-based: Worker 0 and late unreachable; Gate 1, done has no guard. Conditional early end enables suffix only for fail: Worker 0..1 and g selects both ends.

This side: satisfied; exit 0; diagnostic codes in report order/multiset: ['W-UNREACH-0', 'W-UNREACH-2']. No new Agency diagnostic.

All ordinary durations are fixed minutes. P is always not_derived. L basis unknown, fallback unchecked; zero exercise does not imply unnecessary. Empty derived collections differ from not_derived. The expected viewpoints payload is complete, ordered and independent of actual output.
