# F7 fault: unknown state-guard refusal is omitted, not invented

Single edit: j==pass becomes decision==pass. Initial decision=none; receipt0,
Source0..1, Judge1..2. Judge.finish2 writes pass/fail together with j. pass takes
post2..3/done3/e3/w3; fail takes rejected2/e2/w2. All post-Judge state values
remain unchanged. result_at=null; safety/root still satisfied/exit0.

Origin is unchanged: Source direct Invoice; Judge direct PurchaseOrder/indirect
Invoice; Invoice, PurchaseOrder, Ledger have empty direct/indirect sets. But
the effect path reads state, so its whole refusal row is omitted. There is no
other effect supporting j as J: lineage and lineage_groups are empty. Unknown
is not reachable=false or stops=none. rules=agency-p1-state-1, the seven old
unchecked entries remain and state_guards is appended once. W-AGY-0/1 absent.
This single fault is detected by Agency, not by exit or time changes.
