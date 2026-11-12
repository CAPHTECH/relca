# F1 故障 — 向きだけ反転

正例の (r -> ok) を (ok -> r) に一回だけ置換。
P=!ok||r、SetR は後件 r を true にするだけなので、どの P=true 前状態からも P=false にできない。
O の support と原本は変わらず Record、O present。U の候補は空、not_derived/no_reachable_effect。
句なし時間は unchecked。実行は returned3/done6分、根 safety satisfied のまま。警告 W-RESP-2 を捏造しない。
