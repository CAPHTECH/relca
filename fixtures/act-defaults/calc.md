# F2: 全欄の既定 (ADR 0023、裁定 1)

c は呼ばれない。Task は 0 個、終了 0 分、作業量 0 分。
正時間 Capability が無いので容量は不要。容量の欠落を 0 や無限へ変換しない。
boundary=A、control=environment のみ値を補い、capability=[c] は宣言。
他の 9 欄は null/default。仮定は A.boundary/A.control/A.permission と schedule。
なし/未指定の欄を仮定一覧へ増やさない。W-CUT-0 のみ。
