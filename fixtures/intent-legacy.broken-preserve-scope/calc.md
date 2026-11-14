# 範囲外の保持には相手別の案内を付けない

単一故障は既定retire Clerkをpreserve Controlsへ変えたこと。C/Bの4/2分、10行は同じ。
Controlsの適用先はAだけなのでBで保持できず、E-DIFF-0 / invalid / exit 3。
これは変更の照合不一致ではないため intent from の案内を付けない。
global preserveの注記は10行に付くが変更理由を覆わない。W-DIFF-1/2/3=10/0/0。
preservation.propertiesは空で、範囲外の契約をsatisfiedとして数えない。
