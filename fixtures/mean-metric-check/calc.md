# 期待値の判定

`mean-work` と同じ経路。費用は 21、41、83、103 円、確率は 3/8、3/8、1/8、1/8。期待費用 93/2 円は 50 円以下なので satisfied、exit 0。

一つずつ条件を替える対:

| 対 | 変更 | 手計算 |
|---|---|---|
| nondet | 結果から確率を外す | 全 pass で (21+41)/2=31 円、全 fail で (83+103)/2=93 円。50 円を跨ぎ inconclusive、exit 2 |
| tight | 費用の閾値だけ45円にする | 93/2 > 45、violated、exit 1 |
| defer | 影響不明の Foo を保留する | 費用は not_configured、inconclusive、exit 2 |
| clock | Lead を測る同じモデルを5分で切る | 切れた7分の経路は少なくとも6分。下界 (3×2+3×4+5+6)/8=29/8 分。4分以下は inconclusive、3分以下は violated、3分以上は satisfied |
