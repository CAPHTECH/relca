# F4 開始前の受付層

ADR 0015追記を優先。質量1の経路。case1は[0,2)、case2は1に受付し未開始。
時刻2の受付層ではW=1、case3拒否。その後finish/startの混合層でcase2を開始し4に終了。
End1 completed2/elapsed2/work2、End2 completed4/elapsed3/work2、result_at null。
case3 Reject確率1・時間4欄null。hit確率(1,1,0)はすべて>=0、各satisfied。
共有容量超過無し、拒否数min=max=1、truncated false、root satisfied。
reachable doneを足した場合だけcase3 violatedとなる対照は公開テストに置く。
