# Q4 閉じの手計算と拒否境界

実行より先に固定する期待。既存の受入・snapshotから期待を写さない。

## minor-1: 別々の予約と閾値

従来の制御予約は binding数 n、終端数 e に対し
(n + e + 129)(n + 1)。条件待ちの追加walkは別に予約する。
旧予約の上限1,000,000を二つのwalkの和と比較しない。
条件walk自身にも独立した1,000,000の上限を設ける。

contestの宣言validatorを直接呼ぶ私有probeはbinding枠936/937、終端2個。
n=936なら999,779、n=937なら1,001,784。
前者は条件待ちの有無によらず宣言validatorが受理し、後者は両方が
U-SEM-0 / invalid responsibility/contest static IRで拒否される。
現コードのinvalidというhelper名は公開verdictのinvalidではなくunsupportedである。
既存の正常contest模型から枠だけ増やし、WaitUntilの有無だけが異なる対を作る。
他のIR整合や実行を認定するprobeではなく、公開CLIの大模型を完全探索できたとも
主張しない。公開判定には制限付きAuthority等の先行拒否もあるため、
その優先順を変えて閾値へ到達させることはしない。

diffの私有override検査も、同じ小模型のn=3/e=1の予約532と
その直前531を直接与えて境界を検査する。条件待ちは最終Endの後に追加する
（実行可能な条件待ちの認定ではなく、追加walkの会計だけを分離するprobe）。
条件walk自身の上限は別の私有probeで検査する。

## minor-2: IfとWaitUntilの混在

三つの逐次形を用いる。Ifはapprovalがfailならreturned、passなら継続。
1. If → before=post → WaitUntil → posted=post → End
2. WaitUntil → If → posted=post → End
3. Ifの生存腕内でWaitUntil → posted=post → End

Reader1分、Approver1分、post一回1分。1/3のpass経路は4分/3分で終わり、
fail経路は2分でreturned。2のfailは待ちでdeadlock、root violated/exit1。
他はsafety satisfied/exit0。どの形もApproverの候補効果の一部が条件待ちの
後ろなのでrefusal行全体をnot_derived/condition_wait_control/effects空へ撤回。
特に1では待ちより前のbeforeも空にする。Ifの部分的な保証を混ぜない。
一変更の対はWaitUntilを固定wait0分に置き換え、既存Ifの支持presentを要求する。

## 未確認箇所の閉じ方

diffとcontestはWaitUntilを受理する実CLIの観測を作業記録へ残す。
ただし現在の条件解放をIfの決定支持と認定する規則はない。
diffのintent関連候補・override・maintain、contestの終端/commitの候補と
制限付きAuthorityの照合は、その解放を黙って落としてはいけない。
> 当時の裁定: この閉じでは新しい決定支持規則を発明せず、選択した片側でもWaitUntilがあるdiffは
> U-SEM-0/condition_wait_diff_unsupported、WaitUntilとcontestの組は
> U-SEM-0/condition_wait_contest_unsupported、いずれもexit4として拒否する。

P3aの通常fold裁定では、比較機能の未対応と子の検査結果を分ける。
WaitUntilのdiffはU-SEM-0/condition_wait_diff_unsupportedを保持し、比較の表を作らない。
rootは子のverdictとこの未対応を通常の優先順位で畳む。この入力のwait側はg=failの枝で
到達deadlockを持つので、子の確定violatedを残しrootもviolated/exit1となる。
子がsatisfied/unsupportedだけならrootはunsupported/exit4であり、成功にしない。
contestとの未対応は引き続きU-SEM-0/condition_wait_contest_unsupported、exit4である。
Ifだけの対応済み対照はsatisfied/exit0。contestは終端とcommit、制限付きAuthority
あり/なしの組、diffはfromだけ/toだけ/両方をそれぞれ実CLIで守る。
選ばないprocess内にだけ条件待ちがある場合まで拒否範囲を拡大しない。
