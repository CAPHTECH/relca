# lin-same-actor — hand oracle before implementation

The calculation uses lexical call identity and source ancestry.
This is a lexical structural analysis, not proof of real-world independence.
Warnings do not alter the root verdict. No schedule is replaced by a random choice.
The mutation wrapper keeps W-LIN-2; Report.agency.unchecked does not.

F10 (M1 correction): same Actor A has two judgements on e, each reads R.
The ancestor source sets are E(aV)=E(bV)={S,R}; neither consumes the other's
result, so neither is excluded as a descendant. E(aV) is contained in E(bV)
and vice versa: both warn. The independent-source union remains {R}, size 1;
the old lineage is unchanged. This is source reuse, not Actor-name independence.
3 min, Pr(done)=1; the two warnings do not change satisfied/exit0.

Exact W-LIN-2 call inventory: ["aV", "bV"].
No other warning is expected (explicit Cut; no committing effect).
Every Task has positive duration; serial durations add to work and elapsed.
All routes finish before clock 30 min, safety satisfied, no truncation.
No oracle is copied from a new binary. Extra boundary cases live in dedicated tests.
