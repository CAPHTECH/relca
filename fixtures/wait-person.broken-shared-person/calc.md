# wait-person.broken-shared-person

ADR 0048; design v0.2 §22.3. Units: minutes.

T02: independent [0,3),[0,5), End5 work8 => service5 and joins2,0. Shared p gives a0..3/b3..8 or b0..5/a5..8. Service8 always; Task executor extrema a0..5,b0..3. Same-leaf total is3 or5, not marginal sum0..8. Average extrema a0..5/8,b0..3/8. Inline join controls: an observed-false outer branch keeps lexical f/a/b but never creates either Task, so End0 and both join_wait null/never_created. Horizon4 instead observes a.stop3 and b still running; both join waits null/unfinished, dwell service[0,4)=4, never a forecast of long.stop5 or join2.

[R] Hand interval arithmetic, not captured collector output. Stage C acceptance stops RED; production fault calibration not performed.
