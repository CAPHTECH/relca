# clock だけを0にする

正解版のclock2を0に変更。ADR 0014・ADR 0013 による手計算。
次候補1へ進む前に全質量1をclock切断。D2,D3ともclock0より大きい。
両問いは下界0、上界0+1=1、min0/max1、閾値1/2と3/4はいずれも inconclusive。
root inconclusive/exit2、truncated=true。完了した正経路なし、terminals空。
elapsed/work の0行を作らない。W-CUT-0 のみ、W-BUDGET-0 は禁止。
base query1 exact1/2との変化を検出する単一故障。
