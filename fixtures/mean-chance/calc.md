# 確率で重み付けた経過の期待値

ADR 0078 の5.1。実装前に計算した期待値である。
pass/2分、pass/4分、fail/5分、fail/7分の確率は 3/8,3/8,1/8,1/8。
全流入の経過は (3×2+3×4+5+7)/8 = 15/4 分。
Done 到達だけでは (3/8×2+3/8×4)/(3/4)=3分。
Reworked 到達だけでは (5/8+7/8)/(1/4)=6分。
Done の全流入は到達確率3/4なので値も下界も無し、beyond_reached_mass。
reachable done には証人があり satisfied/exit 0。mean 自体は合否を変えない。
故障対は結果の非決定化、clock、opaque defer の1変更。固定待ちの対は [mean-work.broken-wait](../mean-work.broken-wait/) にある。
