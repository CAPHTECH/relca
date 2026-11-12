# Explicit-limit internal test obligations

Public fixtures and tests are intentionally RED. They do not establish these
private state, graph, clock or malformed-IR obligations. Design v0.2 §22.3.
ADR0037 / ADR0035 dependent input specifications are in the task deferred list.

| ID | Independent assertion / hand oracle | Calibration target |
|---|---|---|
| B01 | P2 readmission happens after instant transitions and before clock; A=(0,0,2), E=(0,0,2), S=(0,2,4), C=(2,4,6) | K01, K02, K24 |
| B02 | Held has received flag, initialized local state, no Task/control execution; joint initial product at0 is eight tuples each1/8, no new initialization on readmission | K14, K16, K26 |
| B03 | Held/Overflow/admitted_at/peak/monitor marks distinguish Eq/Hash and reference simulation signatures; no dedup of semantically different receipt/admission states | K25 |
| B04 | Clock3 snapshot: received3/admitted2/completed1/unfinished2/rejected0/deadlocked0/not_arrived0; A=(0,2,null); public check counts remain null/truncated | K10, K19 |
| B05 | Clock2 performs all same-tick completion/readmission: case2 admitted_at2 persists; clock0 Taskless repeated end completes all3 without cut or self-loop | K28 |
| B06 | Rejected slots never initialize, receive or admit; old rejection state census and budgets0..held+1 unchanged | K30 |
| B07 | Malformed IR independently fails closed in engine U-SEM-0 and reference SemanticAbort: zero wait queue/concurrent, mismatched actions/EndIds, out-of-range EndId, missing/extra overflow mark, process using overflow end, disallowed Admitted trigger and unsorted/mismatched arrival vector | K31 |
| B08 | Response Admitted matches only admission bundle, never received/task event; deadline checked addition must abort rather than wrap | K05, K09 |
| B09 | Never create Task for overflow; resource/time/cost inventory remains never_created rather than free work; pre-existing required-rate inventory retained | K20, K29 |
| B10 | Concurrent frees at BusinessEnd even with contest route occupied2..7; route keeps its own resources but not the case slot | K27 |
| B11 | Whole-path counters aggregate before extrema; count maxima cannot be sums of per-case maxima; conservative cut bounds include unclassified arrivals | K10, K18, K19 |
| B12 | New-only initial distribution provenance uses independent-per-arrival; simulation keys case1/2/3 init n1 with no admission site; replay independently rejects swapped FIFO, tampered admitted time, impossible received flag | K24, K26 |

## Frozen source-population tests

The following src tests scan fixture directories. Add the **exact** limits name
helper (not a prefix-only bypass), preserving their OLD numerical census. New
positive-time limits runtime inputs belong to explicit_limits, not old census.

- syntax::tests::every_fixture_obeys_its_expected_syntax_diagnostics:
  read_dir around3081 currently excludes policy/resource/etc. It observes a
  lim-clock unsupported-concurrent refusal against the new runtime oracle.
- engine::legacy_census::e_legacy_state_identity, population near1792..1828:
  exclude lim names from the OLD population; observed199 vs frozen124 is
  exactly the75 new directories, not permission to change124.
- engine::simulation::simulation_tests::s_engine_migration_classification_preflight,
  population around1057..1063: exclude exact lim names. Observed (16,223) vs
  frozen (16,148) is again75 additional directories; six new old-syntax
  controls also show up as successful unclassified simulations.
- Other src read_dir sites around engine.rs178/270/882/2144 are prefix-selected
  duration/state or closed old lists; inspect any observed failure before changing.

Do not change old expected numbers, allow additional old output bytes, modify
Agency outside the overflow framing exclusion, or count compile failures as
semantic mutation kills. K01..K32 are planned calibrations, not performed here.
