# M-G: 割当契約なし

mut-guardからcontract blockだけ削除。構文・実行経路・二ガードは不変。
ADR 0020の割当限定規則。各G体でUがall_paths→noneに悪化しても
Holdのrequires_event/invariantは未宣言なのでundetected/no_assigned_check。
other_changesにU悪化と警告を残す。拒否種の二体はdetectedのまま。
R二体は未検出、全6体。元satisfied、run complete、全体unsupported/4。
