# 追加行列の手導出

core responsibility_extensions の matrix_inputs は次の独立な入力を構成する。
出力を期待値に写さず、全入力の責務行を除いた skeleton も先に検査する。

|入力|導出|
|---|---|
|union|support(r→ok)={r,ok}。SetR の Record と SetOk の Other の和が必要。Guard は両方を直接読むので O present、required/direct/indirect=[Record,Other]。ok=true の SetOk を呼んだ実際の経路でも、抽象前状態で SetR が破壊可能な点を維持する|
|union-missing|Guard の Other 直接readだけを外す。SetOk.emit→Reader.read→Guard.check の実引数系譜により Other の間接到達は残る。O の missing=[Other]。required/indirect は和集合のまま|
|zero-mass|原子的kernelの正質量1の後状態は11だけ。0質量の後10は候補にしない。U effects=[]|
|positive-mass|両枝を1/2へ変更。後10への正質量の破壊が現れ、SetBoth.put/out が候補になる。原子的branchを混ぜない|
|two-paths|j2分から short put3分への鎖は5、j2→relay4→long put3は9。両方を足した14でも最短5でもなく max9。Choice1分/Reader1分は j より前なので除外。最大 witness=[j,relay,long]|
|repeat-capability|同じ Relay.forward を別call relay/again として二回経る。2+4+4+3=13分。CapabilityIdで一回にまとめた9は誤り|
|unused-reads|先に宣言した Other を読む未呼出Capabilityを Effect に足す。全宣言readsを含め required=[Other,Record]、p の direct は Record だけなので missing=[Other]。呼出効果は put/out のみ|
|two-effects|j の後、guard の前に unprotected=Effect.put(j) を置く。unprotected は両結果で進む none、guard 後 out は all_paths。全効果foldは missing、保護済み一件で他の欠落を消さない|

時間phase専用の非公開予算・単一抽象状態のfalse→false・requires接合の旧Agency oracleは
別の内部試験を必要とし、この行列で確かめたとは主張しない。
