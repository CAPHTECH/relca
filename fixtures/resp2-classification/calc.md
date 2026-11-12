# F10 既存条件の対照

対象 (r == true) は旧構文。Effect は r=true を書くため P を真→偽にできず、U effects=[]/no_reachable_effect。
原本は引数祖先の Record。Judge が直接読むので O present。rules は p1-1、時間 unchecked の旧形。
根 safety satisfied、returned3/done6分。故障は == を > に変える一編集だけ。
