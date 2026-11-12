# ADR 0051 hand calculations (written before acceptance code)

The minimal input has both A.one and A.two active in both processes. Before:
x=A.one(), spare=A.two(), y=R.use(x). After:
x=A.two(), spare=A.one(), y=R.use(x). Both actors, both capabilities, all
resources, times and ends remain identical. Only x is read; spare is not.
Thus exactly one new row: relation/x/modified, A.one -> A.two, no reason or
annotation. W-DIFF-1=1, W-DIFF-3=1. Both elapsed/work=3. Both verdict satisfied.
The single-fault control replaces After with Before: no row, no W1/W3.

Variants keep this arithmetic unless explicitly stated:

- Two readers of x: elapsed/work=4, still one x row. Fork readers with R capacity2:
  elapsed3/work4. A duplicate argument counts x only once.
- Condition-only read: x selects done/other, spare remains unused. x row1.
- Dead else branch or suffix after end: lexical x read still counts once.
- Rename x in After to z: common read names empty, row0.
- Remove the use of x in both sides: both result bindings unconsumed, row0.
- Same producer, changed reader: row0. Same capability at different sites: row0.
- Reorder declaration sites of x and spare: x's producer still changes once;
  compare names, not binding indices. Cross-Actor A.one->B.one with both actors
  active and grain document on B also changes x once; the label is not meaning.
- Fork both producers at capacity A1: the two positive-time calls serialize,
  then R.use(x) finishes at3. Both call-mode sets stay fork, so only x changes.
- Join-any-only read: fork x/spare with their producers exchanged, no arguments
  or If result conditions. The where-result matches read both bindings: exactly
  spare:A.two->A.one then x:A.one->A.two, W1/W3 each2. Same producer or join all
  gives0. Directed change on x gives W1=1/W3=2, reason=transfer, no E-DIFF-0.
  Full IR position table, the repaired terminal declaration in the review input,
  independent oracle and closed compatibility recount: [relation_producer_reads.md](relation_producer_reads.md).
- An observed flag written by A.one/A.two and used only through state conditions
  is not a process-binding read. With no x argument, producer row0 and no W1/W3.
- Two consumed bindings x/spare both switch: rows sorted spare,x, each one.
- change relation x reason transfer: one x row, reason transfer, W1=0,W3=1.
- preserve Ctl on relation x plus change: W1=0,W3=0; Ctl's safety contract
  stays satisfied. A global preserve attaches but does not cover W1.
- Same producer with change: E-DIFF-0/invalid. Unknown binding: E-NAME-0/invalid.
- override or maintain on bare x: U-SEM-0/unsupported; no fabricated U/duty.

Invoice hand nucleus (legacy phase0 fixture and current phase1 artifact):
AsIs/P both bind entry=Clerk.enter; A binds entry=Extractor.extract.
po, receipt and approval keep their producer; ack/registration keep theirs.
entry is read by subsequent calls. AsIs->A and P->A therefore have exactly
one producer row; AsIs->P zero; A->P one with the direction reversed.
This is not the proposal's total seven rows (design v0.2 §12.6); prior ADRs
already chose different granularity for time and Permission.

Wire controls are hand-assembled, not sampled from production serialization.
Valid new row has producer objects on both sides; legacy relation added has
neither. Single-field corruption must fail: missing side, null, empty producer,
one/three-part producer, same producer both sides, unknown field at row or value,
added/removed with values, modified with Capability target.
Producer annotations admit preserve only; otherwise well-formed override,
maintain and keep_fallback annotations are attachment faults, not new semantics.

Compatibility uses canonical invocations only. Census independently walks old
selected IR and is calibrated against these hand inputs. It does not call
production diff; raw old reports only locate allowed root edits. Non-diff outputs,
side reports, Agency, exit, and untouched bytes have no allowance.
