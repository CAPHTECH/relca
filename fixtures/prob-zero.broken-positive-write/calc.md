# F7 単一故障: 正確率pass枝の定数 false → true

確率は1+0=1のまま、passだけでdone@1、elapsed=work=1 分、result_at=null、done min=max1。
正常終了の同bundleでdanger=true、その後にfinish invariantを評価するので violated。
reached danger=true の min=max1、<=0 は1>0で violated。safety satisfied。
root violated/exit1、truncated=false。0枝の100分slowはやはり実行しない。
W-CUT-0 は中立。確率2tokenの変更ではなく定数1tokenだけの故障。実装出力未使用。
