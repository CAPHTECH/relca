# 統制を外す向きの差分

from New は承認と支払で2分、work2分、Controls satisfied。to Old は支払だけで1分、work1分。
両側 reachable は satisfied だが、to の Controls は非適用で null。
通常の Screen Actor・Screen.approve 関係・その根拠・Screen の資源が removed。
加えて Controls の適用が removed。構造差分はこの5種類で、単価や未知の品質を補わない。
意図と注記が無いので W-DIFF-1 / W-DIFF-3、全体は satisfied / exit0。
壊した対は to Old に preserve Controls だけを足す。検査しない契約を保つという矛盾を E-DIFF-0 で拒否する。
