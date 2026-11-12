# F2: 同一 posted call への全経路の論理和

ADR 0009 と計画の第4節から独立に計算。時間は分、確率は入力していない。
Before/After の S(entry)=S(j)=S(x)={Invoice}。
Before の J は j だけ(x は効果の引数祖先でも条件参照でもない)。After は j,x。
各対象 {Invoice}、独立源は空。群は Before 1判定1Actor、After 2判定2Actor。
O は両側同じ: Invoice空、Reader.direct={Invoice}、
Judge/Flag/Ledger.indirect={Invoice}、残りの direct/indirect は空。

| j | x | Before 到達 | After 到達 |
|---|---|---|---|
| pass | pass | yes | yes |
| pass | fail | yes | yes |
| fail | pass | no | no |
| fail | fail | no | yes |

Before Reach=j.pass、j の文脈は {} の1個、fail が止める all_paths/0。
After Reach=(x.pass AND j.pass) OR x.fail。
j の文脈: {x=pass} では fail が止める、{x=fail} では全 j で進む。
x の文脈: {j=fail} では pass が止める、{j=pass} では全 x で進む。
したがって両方 conditional/1、W-AGY-1、W-AGY-0 不在。

共通prefix3分。Before の j=fail(2組) / After の (fail,pass)1組は rejected3/3。
他はpost1を足し done4/4、result_at=null。切断なし、両側 satisfied/exit0。
同じCut、同じActor/capability集合なので differences=[]、mismatch=null。
diff.agency.from/to はそれぞれ側reportの完全agencyと等しい。左右を取り違えない非対称oracle。
