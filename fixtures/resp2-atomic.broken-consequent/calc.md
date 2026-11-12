# F13 故障 — 原子的に悪い組へ

finish の ok=true→false だけを変更。後状態10は P=false、前00は P=true なので SetBoth.put/out が破壊候補。
担い手の判断も guard も無いため U missing、effect.status=none、judgements=[]、W-RESP-2 を追加。
O missing/Record と W-RESP-1 は不変。root safety satisfied、done3分、時間 unchecked。
