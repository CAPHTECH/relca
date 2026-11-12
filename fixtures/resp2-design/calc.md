# F9 設計の2行 — 限定した受理

逐語の行 responsible a1 for (registered -> approval == pass) と responsible s1 for Extractor.maintenance を置く。
registered は観測 bool、approval は観測 Verdict、Extractor と Invoice は宣言済み。
二つの責務は宣言順 id1/id2、両方 p1-2、時間句なしは unchecked 一鍵のみ。
Guard は approval を結果で書き、Extractor は registered=true を書く。process は returned3/done6分。
未対応の record/unknown duration 等を含む設計全文が通るとは主張しない。safety satisfied。
故障対は2行目の Extractor だけ Unknown に変える。
