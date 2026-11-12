# F5 案件ごと独立な初期値

ADR 0013・0015、実装とは独立の積。Chooserは[0,1),[1,2)、Rはcase1から開始。

|x1,x2|質量|case1 hit/終了|case2 hit/終了|
|---|---|---|---|
|short,short|1/2×1/2=1/4|2|3|
|short,long|1/4|2|5|
|long,short|1/4|4|5|
|long,long|1/4|4|7|

和1、within3 はcase1 1/4+1/4=1/2、case2 1/4、各min=max。
<=1/2で両satisfied、safetyもsatisfied。completed=elapsedは2..4と3..7。
各workはChooser1+R(1または3)=2..4、result_at null、End確率1、拒否0、切断無し。
同じdraw共有ならSS/LL各1/2となりcase2 within3が1/2へ誤る。
業務ifはobserved routeだけを読み、潜在xを直接読まない。
