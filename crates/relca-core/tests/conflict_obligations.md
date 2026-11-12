# Shared-update internal seams and integration checks

These are finite test obligations, not a claim of execution. Public acceptance
is in conflicts.rs; fixture numbers are independently derived in calc.md.

## Internal linkage

The trace tests are linked from engine/trace.rs:
`#[cfg(test)] #[path = "../../tests/support/conflict_private_trace.rs"] mod conflict_private;`.
The authored module
checks joint one-commit paths (A01/A02/A24), conflict Finish without Commit or
writes (A16/A17), second-variable staleness (A19), ABA (A20), cancel versus drain
(A21), and an independent BFS minimum plus replay tampering (A05/F35).
Capability display names in Stage must retain the existing bare-name contract.

Additional internal linkages:

- engine.rs: `#[cfg(test)] #[path = "../tests/support/conflict_private_state.rs"] mod conflict_private_state;`
- lib.rs: `#[cfg(test)] #[path = "../tests/support/conflict_ir_internal.rs"] mod conflict_ir_internal;`

Representation adaptations must preserve the direct Eq,
HashSet2, fresh/stale result and malformed-IR assertions.

## A22 / F19: direct semantic identity

Create a running long Task with checked holder, clone the entire local State,
and change ONLY the clone's running Task stale vector from empty to [holder].
Assert direct State inequality and HashSet length2 (do not demand unequal hash
values). Complete each state one step: committed versus conflict. Repeat with
Global and both sample state-key paths. Also clone a post-loss state with only
lost_updates changed; it must not merge after the writer Task is removed.
An end-to-end state count is NOT a substitute for direct Eq coverage.

## Public IR defenses (F32 and adjacent malformed-state obligations)

Each entry must fail closed in engine and independent reference, not panic:
forged capacity2 with old Serialized pair table; empty/duplicate/out-of-bounds
version variable IDs; unknown reject value/type; overlap with ordinary/atomic/
requires results; checked start effect; kernel+version; LWW on latent/case/
per_case storage; missing/duplicate/extra NoLostUpdate properties; forged pair
table resource/operator evidence. Use otherwise-valid source controls so one
malformation does not mask another. Engine error U-SEM-0, reference SemanticAbort.

## Retained independent crossings

Do not infer a write receipt from value differences. A19/A20 include unchanged
and restored values. Add an existing-element InsertCase notification prefix.
Preserve start receipts across cancellation, test a nonchecked start marker,
requires rejection, atomic rejection, version rejection and a range stop as
distinct notification cases. F29 queued-mark mutation can be equivalent when
start-reset erases it: record equivalence, do not count it as a kill.

Test lock arithmetic in u128 at u64::MAX and all start/finish/atomic write
combinations. The static writer set includes uncalled capabilities. Self-pairs
and cross-pairs are separate obligations; cf-cross-pair isolates the latter.
S20's capacity2 commutative Add fork pair is in conflict_ir_internal.rs; the
L/V/W input pairs are fixtures and the unchanged shared InsertCase fork pair is
aset-static-fork-target. Thus preserving only the case-observed int-fork guard
cannot hide an ActorObserved-only exception.
Run the existing no-new-syntax census unchanged, including held-budget counts.

## Existing source-test populations

Use the explicit conflict_names.json
population (not an unrestricted prefix) alongside the existing caseset filter:

- syntax.rs fixture parsing census around line3439;
- engine.rs runtime fixture census around line1847;
- engine/simulation.rs sample fixture census around line1071.

The population filter does not change numeric state/time/work expectations.

## Deferred preceding/following integrations

A25 external admission, A26 one-case effect route, inline-enum/record rejects
and state_refusers expectations need separate integration checks, not
silently treated as covered by current tests. Integration must regenerate only
the new conflict compatibility manifest from the pre-A5 integrated executable.
