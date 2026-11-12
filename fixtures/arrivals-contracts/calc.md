# F11 契約monitorを案件別に

唯一の時間経路[0,2),[2,4)、質量を入力していないので確率鍵無し。
各finish bundleのwrite後hit=trueなのでinvariant各satisfied。
commitは全体2回でも各caseで1回なのでat_most_once各satisfied。
Receivedは各時刻0、期限4に対してEnd2/4は期限内、response各satisfied。
completed=elapsed2/4、各work2、result_at null。全範囲最小=最大、拒否0、切断無し。
safety satisfied、root satisfied。case1のEndでcase2の義務を閉じない。

## 正式24入力外の公開oracle (arrivals_fifo_queue)

以下も実装の出力からでなく、ADR 0015追記の層・相対期限・既存契約意味から算出。
各「唯一」は確率を入力しない限り確率鍵を足す意味でない。

| ID | 経路と判定過程 |
|---|---|
|A-no-task-end|同着0/0・queue1。受付層ではcase1を予約してW=1、case2Reject。Mixedでcase1 End0/work0。Rejectの時間nullと区別。reachableはcase1 satisfied、case2 violated。|
|A-ever-started|到着0/1/3、A1分→B3分。A区間[0,1),[1,2),[3,4)、B区間[1,4),[4,7),[7,10)。時刻3でB待ちcase2も既StartなのでW=0、case3受理。completed4/7/10、elapsed4/6/7、各work4。|
|A-same-case-tie|同caseのshort1/long3を容量1でfork。short先ならwithin1 hit1、long先なら0。非決定min0/max1、全完了4/work4。binding順への固定は禁止。|
|A-person-global|2cases×(A1/B1)、人pは全体に1人。4Taskを1つずつ処理、総作業4、各casework2、各caseが先に2Task終えると2、最後に終えると4。各completed min2/max4。Permission/Authorityは検査であり候補filterでない。|
|A-separation-case|公開正例A(p)→B(q)は各case satisfied。B(p)だけに替えれば各violated。p→q/q→pの交差traceそのものは内部試験の引渡し義務。|
|A-requires-case|案件ごとChooseがA-only/B-onlyを非決定。別案件Aが先に終わってもB-only案件のB.startに同案件先行A無し。各caseに違反経路があるので各violated。ChooseをA-onlyに固定した対照は各vacuous。|
|A-liveness|到着5/6のみでもdeadlock無し。容量1・2分ならEnd7/9。容量0なら最後の到着後Queuedのまま、W-DEADLOCK/root violated、safety satisfied。|
|A-live-per-case|同着2cases各Task1、bound1なら合法。容量2・4分で各End4/work4。各caseをfork2に変えた対照はstrict超過なのでtruncated/inconclusive。|
|A-shifted-cuts|到着5/6、各D={1,3}各1/2、容量2、clock7。within2でcase1期限7なのでhit1/2既知、case2期限8なので[1/2,1]。within1は期限6/7で各1/2。>=1/2は満たすがsafetyはcutによりinconclusive、global truncated true。|
|A-unarrived-cut|到着0/8、clock7、initial hit=true、within0。case1受付でhit1、case2未到着は未知[0,1]、terminal無し。Reject件数の切断時保守幅[0,1]。|
|A-completed-before-cut|到着0/3、clock4、固定2分、hit finish。case1 End2/work2/確率1を保存、case2未完了でwithin2は[0,1]。全完了leafが無くてもcase1を失わない。queue0・到着0/5対照はcase1Reject1を保存、case2未知、拒否件数[1,2]。|
|A-single-new|arrivals5だけのcases1はEnd7、elapsed/work2。policyだけならarrival0。queue0だけならarrival0でReject。新句無し旧モデルには新鍵/新仮定行無し。|
|A-static-cancel-read|F容量2/1分pass、S容量1/3分ok、forbidden。1caseでは両0開始、Sは必ず結果okなのでif/引数が定義済み。2casesでは他案件S占有の間に未開始Sをcancelしうるので結果読取りU-SEM。observed slowの現在値は常に定義済みで受理。|
|A-vacuity-fold|queue1・同着0/0でcase1はReceived→Endが期限内、case2はRejectedでReceived無し。responseはsatisfied/vacuous、ADR追記のfoldでvacuous/exit5。queue0は両vacuous。|
|A-u64-relative|Task無しEnd、到着MAX−1/MAX、within5。arrival+5を加算せず相対比較できる。hit初期falseなので0、End elapsed/work0、cut無し。|

各時間範囲は上記の許された経路のmin/max。確率を入力したoracleの0/1は既知失敗/既知hit、
切断の[0,1]は未知であり1/2へ置換しない。A-shifted-cutsの1/2は独立durationの周辺の
1/2と、他案件の枝の質量和(1/2+1/2)=1との積で、各案件ごと1/2×1=1/2。
