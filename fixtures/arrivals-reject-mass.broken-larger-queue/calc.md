# F3 単一故障: queue capacity 1 → 2

全3件受理。独立D1,D2,D3の8組各1/8、総和1。
case3終了3,5,5,7,5,7,7,9。質量をまとめると3:1/8,5:3/8,7:3/8,9:1/8。
期限は到着2+within3=5、hit=1/8+3/8=1/2。eventual1。min=max。
case1 within3=1、case2=3/4、全query/safety satisfied。
case1 completed1..3/elapsed1..3、case2 completed2..6/elapsed1..5、case3 completed3..9/elapsed1..7。
各work1..3、result_at null、End確率1。拒否0、truncated false。受理時の正規化変更ではない。
