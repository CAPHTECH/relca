# P13: aset-finish-snapshot.broken-false-toggle

Hand oracle fixed before implementation (single edit). Unit: min.

At start0 gate=false selects the reject branch. Toggle ends1 and writes true; post ends3, evaluates true and commits in the then branch. W3+1=4, keys{1}. Fault leaves false: reject only, keys{}, commit0 and at_most_once vacuous. This distinguishes finish evaluation from start snapshot with capacity1 and tests commit_when=true.

Single mechanism edit: 'finish{gate=true}' -> 'finish{gate=false}'.

Static failures have no analyzed time or invented zero counts. Computational cuts are not business rejection. Atomic refusal is a normal finish, not a requires refusal. A RED checkpoint is not semantic verification.
