# ADR 0043 hand derivation

T07/T19. keys initially empty. All insertions are the current case.id. First post without start/earlier insertion cannot reject: state_refuser Ledger on_normal_path=false and v all_paths/0. Legacy refusal remains [] with state_guards. A second post sees prior insertion, or its own start inserts before its test: unknown, process_judgements not_derived/state_guards, no j/u keys. pass first commit at3, second normal rejection at4; fail returned2. Counts first post0..1, repeated post0..2; self-start commits0 but starts0..1.
Single edit: 'commits; writes keys;' -> 'commits; writes keys; start { keys += key };'.


The expected values above are algebraic/static hand oracles, not captured output. Stage C RED is not implemented semantics, calibrated mutation detection, real-world legitimacy, skill retention, or all-schedule safety.
