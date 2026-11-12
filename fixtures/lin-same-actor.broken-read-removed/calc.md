# lin-same-actor.broken-read-removed — hand oracle before implementation

The calculation uses lexical call identity and source ancestry.
This is a lexical structural analysis, not proof of real-world independence.
Warnings do not alter the root verdict. No schedule is replaced by a random choice.
The mutation wrapper keeps W-LIN-2; Report.agency.unchecked does not.

F10 fault (M1 re-derivation): remove only check_b reads R. E(aV)={S,R},
E(bV)={S}. Neither is an argument descendant of the other. E(bV) is contained
in E(aV), so bV warns. E(aV) is not contained in E(bV), so aV no longer warns.
The broken read removes evidence but also makes aV unique: warning count is not
a quality score. Old lineage I(aV)={R}, I(bV)={} is unchanged.
3 min, Pr(done)=1, satisfied/exit0 unchanged.

Exact W-LIN-2 call inventory: ["bV"].
No other warning is expected (explicit Cut; no committing effect).
Every Task has positive duration; serial durations add to work and elapsed.
All routes finish before clock 30 min, safety satisfied, no truncation.
No oracle is copied from a new binary. Extra boundary cases live in dedicated tests.
