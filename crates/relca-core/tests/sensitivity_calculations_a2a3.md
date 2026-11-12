# Q7 merge 前の独立受入: A2・A3 と review close

基点 7008f3a4。ADR 0060 の 1.1・1.2・5・8 節の契約から計算する。
以下の分数は production の本体や実出力を読む前に固定した手計算である。
既存 sensitivity_calculations.md は変更しない。全数値は合成仮定であり測定値でない。
per_run / duration given の後半受入をここで代行しない。

## A2: 同じ周辺、異なる同時見逃し

wrong は案件の誤入力で P(wrong)=1/50、補数49/50。
Cause は同じ宣言を全点に残す per_case Actor で、bad=1/10、good=9/10。
wrong と Cause は独立の初期抽選とする。
二判定 A/B は一分ずつ順序付きで実行し、それぞれ miss / detect を返す。
根拠系譜の観測用に、先行する Reader.read が Record から Entry を作り、
両判定へ渡す。双方 miss のときだけ Poster.post を呼ぶ。追加の作業は各1分、
最大合計4分、clock=6分。追加の確率抽選はなく、下表の値は変わらない。
独立点では各 kernel に miss=1/10、detect=9/10 を明示し、Cause は読まない。
共有原因点では両 kernel が Cause.bad を読み、bad なら miss 確率1、good なら detect 確率1。
順序相関点では A は独立点と同じ、B は A の結果 given を読む。
A=miss なら B=miss 確率1、A=detect なら B=detect 確率1。
optional の none 行は完全表のため明示するが、順序により実行時には選ばれない。

| 点 | P(A miss) | P(B miss) | P(双方 miss) | P(wrong かつ双方 miss) |
|---|---:|---:|---:|---:|
| independent | 1/10 | 1/10 | (1/10)(1/10)=1/100 | (1/50)(1/100)=1/5000 |
| common | 1/10 | 1/10 | P(Cause.bad)=1/10 | (1/50)(1/10)=1/500 |
| ordered | 1/10 | (1/10)1+(9/10)0=1/10 | (1/10)1=1/10 | (1/50)(1/10)=1/500 |

これは条件付きの積と周辺化の違いであり、Actor 名から独立性を推測していない。
元の根拠 source Record を全点で残し、二判定はそれを直接読む。
common のみ Cause の潜在を kernel_reads で読む。観測 reads は全点 Record のまま。
gate はこの潜在読取 Actor の差、given の差、
kernel_reads latent の差を仮定変更として通す必要がある。
観測 reads 集合の差は許容しない。
A の reads [Record] だけを削る一差分は同じ問いの変更として全点実行前に拒否する。
同一 suite の確率問い合わせは誤通過・双方見逃し・A見逃し・B見逃しの四つ。
前二つの finite extrema は1/5000..1/500、1/100..1/10。
後二つは全点1/10。制約閾値は全点で同一、どの点も満たす。
grain と名前の変更は通常の独立 check で照合し、名前変更を同一 suite の gate に通さない。
順序付き B given を fork 兄弟の相互 given に置換した模型は拒否されるべきで、
有効な相関点として扱わない。

## A3: 繁忙の期待件数は率ではない

容量1、FIFO tie_by case_id、各 Task 1分、取消なし。
通常流入は時刻0に1件。繁忙は時刻0に2件。業務 horizon はなく全件 drain。
最後の終了はそれぞれ時刻1、時刻2で、clock=4 に達しない。
kernel は wrong=true を通常1/100、低品質1/25、false をそれぞれ99/100、24/25とする。
品質 property Wrong は reached(wrong==true)、cohort はその expected count / arrivals。
I_i を案件 i の誤通過 indicator とすると E[sum I_i]=sum P(I_i=1)。
件数のこの加算に、案件間の独立性は必要でない。

| 点 | 案件数 | 一件の誤通過 | expected count | cohort 分母 | cohort rate |
|---|---:|---:|---:|---:|---:|
| normal | 1 | 1/100 | 1/100 | 1 | 1/100 |
| busy | 2 | 1/100 | 1/100+1/100=1/50 | 2 | 1/100 |
| degraded | 1 | 1/25 | 1/25 | 1 | 1/25 |
| busy_degraded | 2 | 1/25 | 1/25+1/25=2/25 | 2 | 1/25 |

全点で不確定性・切断なし。quality expected_count の lower/upper は上表の件数で一致し、
basis=exact、reason=null。率だけを件数欄へ転記すると busy 二点で必ず失敗する。
label だけ変えた busy 点の子 report でも件数/率/品質は不変。
新しい品質 CI や件数の scalar sensitivity summary は要求しない。

## m2: aleatory の独立な値 oracle

A2 common 模型の初期分布は case.wrong: true=1/50, false=49/50、
Cause.bad: true=1/10, false=9/10 (instance per_case)。
A/B の kernel given はどちらも Cause.bad、true 行は miss=1、false 行は detect=1。
条件 true と false を逆にする、または1/10と9/10を逆にする出力は拒否する。
項目の source span は手入力中の Capability / latent 宣言の位置を指すことも確認する。
これらの値は wire の採取値でなく上記入力仮定そのものから決まる。

## m8: 確率値の変更と非決定への変更を区別

A2 independent の wrong 分布だけを1/25・24/25へ変更した対は許可。
誤通過は(1/25)(1/100)=1/2500、他三問は不変。
分布を nondet { true, false } へ置換する対は ADR 0060 の 1.2 節 の許容リスト外。
順序の両向きで suite 全体 invalid / sensitivity_contract_mismatch を要求し、実行数0。
非決定を一様分布へ変換しない。kernel outcomes の nondet 切替えと同じ問い変更である。

## m1 と U1

m1 の traffic wire は次の具体的な入力と期待で固定する。
回答では分布到着の traffic.arrivals は明示 null、traffic.distribution は IR の
全支持列と正確な有理数を保持する。受入の全支持列は [2,2]:1/4、[2,5]:3/4。
宣言無しの単一案件は既存意味の [0] を保持する。
fixed_every 2min during [2min,6min) は [2,4] で、実行の到着時刻と照合する。
U1 は同じ Flow / quality / query を持つ固定到着と分布到着の二点で到達した。
固定到着 [2,2] は通常 simulate で実行される。分布到着に同じ quality を宣言すると
通常 simulate は stochastic_arrivals_feature_combination で driver 実行前に拒否する。
両方とも parse/lower と感度の Contract 射影は通り、同じ問いの比較でも差は無い。
後ろに分布点を置いた suite は前の固定点を実行せず、evaluated=0、refused=1、
not_run=1、exit4 とするのが契約。基点の実測では evaluated=1 となり RED。
入力は support の traffic / with_quality。観測ログと故障校正には別の実行証拠が必要である。
