# F02 single fault

Initial 1 → 2 is the only edit. Finish at minute 1 attempts 2 + 1 = 3.
Range stops this branch before any effect/event/terminal. The attempted 3 is
not a stored state. See the paired derivation for the unknown continuation.

Shortest explain path, in existing trace transition units: initial(received)
at0, queue edge1 at0, start edge2 at0, advance edge3 to1, failed finish
attempt edge4 at1. Thus transition_count4 and five stages. No join or
normal finish is legal before that final attempt. The same four-edge prefix
applies to F03, F14 and F25; F25's flag remains false throughout.
