# F8 単一故障: Writer所要時間1 → 3分

Sampler finish2ではflag=false、false行のhit=trueは0、falseは1。Writer finish3が後からflag=trueを書いてもhitを変えない。
reached hit=true はmin=max0、good確率1、badは未到達なので行無し。
join@3、elapsed3、work=Writer3+Sampler2=5 分、result_at=null。
>=1/2 は最小上界0<1/2より violated、safety satisfied、root violated/exit1、切断なし。
W-CUT-0 は中立。終了時と開始時を分ける手計算で、実装出力未使用。
