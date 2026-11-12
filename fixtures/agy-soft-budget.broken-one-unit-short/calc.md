# A16 agy-soft-budget.broken-one-unit-short

Hand oracle before implementation (ADR 0034; design v0.2 §5.6, v0.2 §22.3).

Identical DSL controls. The private limit injection (not a CLI flag) must compare each O/F/L used dimension k against k-1 and zero. Full expected payload is A06; shortage drops only that whole block to derivation_limit; report remains satisfied/0, no new diagnostic. P and all legacy fields stay equal. See agency_viewpoints_obligations.md.

This side: satisfied; exit 0; diagnostic codes in report order/multiset: []. No new Agency diagnostic.

All ordinary durations are fixed minutes. P is always not_derived. L basis unknown, fallback unchecked; zero exercise does not imply unnecessary. Empty derived collections differ from not_derived. The expected viewpoints payload is complete, ordered and independent of actual output.
