# ADR 0043 hand derivation

T10. Exactly ten changed meaning rows: Doc actors/people/anonymous, c1 direct, Clerk/Extractor direct, Clerk normal/fallback, Extractor normal, roots. No same-valued framing/refusal/group/concentration flags. A constant false Order is viewpoint, then item, then declaration key: retention normal Clerk, normal Extractor, fallback Clerk (the adopted order corrects the initial plan). empty state guard before To posted adds four not_comparable rows: posted reachable/selectors/read_only and global framing actors (complete=false). returned ends before this guard; L stays unchanged. Root structural diff diagnostics W-DIFF-1/3 are separate from side diagnostics.
Single edit: 'process ToFlow { e = Extractor.extract(); v = Approver.approve(e); if v == pass {} else { end returned }; k = Ledger.post(v); end posted' -> 'process ToFlow { e = Extractor.extract(); v = Approver.approve(e); if v == pass {} else { end returned }; k = Ledger.post(v); if flag == true {} else {}; end posted'.


The expected values above are algebraic/static hand oracles, not captured output. Stage C RED is not implemented semantics, calibrated mutation detection, real-world legitimacy, skill retention, or all-schedule safety.
