# F8: givenは正常終了時

両Task start0。Writer finish1でflag=true。Sampler finish2は現在のtrue行を読み hit=true/false各1/2。
bad=(1)(1/2)=1/2、good=(1)(1/2)=1/2。全開始順で同値、min=max1/2。
join@2、elapsed2、work=Writer1+Sampler2=3 分、result_at=null。
safety satisfied、>=1/2 は最小1/2>=1/2で satisfied、root/exit0。truncated=false、W-CUT-0。
開始時falseで抽選すると誤って0となる。独立手計算の期待で実装出力未使用。
