# F4 故障 — 間接は直接 O でない

Judge の reads [Record] だけ削除。Reader は原本を読み続けるので required=[Record]、p の indirect=[Record]。
direct=[] のため O missing、missing_sources=[Record]、行の警告は W-RESP-1 一つ。
U の j→out all_paths は変わらず present。root safety satisfied、returned3/done6分のまま。
Actor 名や同じ原本の間接参照を直接 O の代用にしない。
