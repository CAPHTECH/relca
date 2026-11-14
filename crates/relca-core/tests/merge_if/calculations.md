# 値を返す if: 独立手計算と受入境界

根拠は ADR 0069 の採択した段階 1。期待値は出力採取から作らない。
単位は分。各 Actor の容量は 1。write=1、review=1、fix=2、send=1、audit=1。
模型は直列なので、各経路で work=elapsed、queue=external=intentional=0。
metrics の per_minute と per_start は全て明示的に 1 JPY。
以下の期待確率は分数で再計算した。simulate の標本率とは区別する。

check の wire 指標は期待値でなく経路の min/max である（2026-11-14 に確認）。
上記の E[...] は終端確率と経路時刻から導く手値で、新しい平均の欄は要求しない。
M1/M5 の case work=3〜5、cost=6〜9、fix starts=0〜1、send starts=1。
M2 は work=2〜5、cost=4〜9、send starts=0〜1。M3 は work=3〜6、cost=6〜11。
M4 は work=3〜5、cost=6〜9。枝だけにある Task の starts は 0〜1。

## M1: 修正と既存値の複写

- fail の重みは 1/4。write [0,1)、review [1,2)、fix [2,4)、複写 4、send [4,5)。done_fixed=5。
- pass の重みは 3/4。write [0,1)、review [1,2)、複写 2、send [2,3)。done_as_is=3。
- 総確率=1/4+3/4=1。E[elapsed]=5/4+9/4=14/4=7/2。
- E[fix starts]=1/4、E[send starts]=1/4+3/4=1。
- E[work]=1+1+2(1/4)+1=7/2。E[starts]=1+1+1/4+1=13/4。
- E[cost]=7/2+13/4=27/4 JPY。単価不明をゼロに置き換えた値ではない。
- done_fixed の下限 1/4 は等号成立。Deadline=4 は fail の完了 5 で破れ、全体 violated/exit 1。
- Task は d,r,f,s の 4 site。合流値 d2 の Task は無い。枝へ展開した証拠模型だけ s1,s2 に分かれる。
- explain は Deadline の反例を verified とし、4 分に d2 を f から複写した merged を含む。
  send の start は Actor=Ledger、capability=send、binding=s、時刻=4。
- merged の形は2026-11-14に追加確認した。既存 steps の段に source=f を加え、kind=merged、binding=d2、time=4。
  actor/capability/executor は欄が存在して null。位置は yield 文全体の半開 span。
  M1 の先頭16行（LF込み）の byte 数を手で数えると
  25+36+39+27+36+33+52+27+94+97+4+104+116+17+22+22=751。
  17行目の yield 前は42 byte、yield f は7 byte なので start=751+42=793、end=800。
  syntax.rs の span は最後に消費した非セミコロン・非改行 token の end を採るため、閉じ波括弧や空白は含まない。

## M2: 終了する枝と二重の合流

- reject: 1/4、write+review=2、send は実行しない。
- fail: 1/4、write+review+fix+send=5。g←f の後に d2←g。
- pass: 1/2、write+review+send=3。g←d の後に d2←g。
- 総確率=1/4+1/4+1/2=1。
- E[elapsed]=2/4+5/4+3/2=(2+5+6)/4=13/4。
- E[send starts]=3/4、E[fix starts]=1/4、E[starts]=3、E[cost]=25/4 JPY。

## M3: 非決定と系譜（段階 1）

- fix の有無までの重みは M1 と同じ。audit は非決定であり 1/2 と置かない。
- pass→audit pass→send は 4、fail→fix→audit pass→send は 6。
- pass→audit fail は 3、fail→fix→audit fail は 5。
- done の時刻は [4,6]、held は [3,5]。各確率は [0,1]。
- write の祖先は Invoice。review r の subject=[Invoice]、新規 reads は無い。
- fix f は d を受け、reads Manual を足すので subject=[Invoice]、independent=[Manual]。
- audit a の subject は選択した枝で異なる。修正枝なら Invoice,Manual、非修正枝なら Invoice。
  前者の audit に新規 Manual は無く、後者には有る。和集合・積集合のどちらか一行を正解にしない。
- 段階 1 では a とその推移的依存の判定の lineage 行を出さず、対象名を含む
  lineage(merge_control) の unchecked を置く。r と f の行、O と E は保存する。
- F,U,W-AGY-0,W-AGY-2,W-LIN-2,W-RESP-2 は個別に merge_control で未導出。
  合流値 d2 を判断数へ足さない。段階 2 の経路別行・FI2 の二行保証は今回要求しない。

## M4: 修正後の潜在状態

- good: 4/5、review pass、audit pass、done=3。
- bad かつ review fail: (1/5)(3/4)=3/20、fix が bad=false を書くので audit pass、done=5。
- bad かつ review pass: (1/5)(1/4)=1/20、audit fail、flagged=3。
- done=4/5+3/20=16/20+3/20=19/20、flagged=1/20、総和=1。
- E[elapsed]=(4/5)3+(3/20)5+(1/20)3=(48+15+3)/20=66/20=33/10。
- E[fix starts]=3/20、E[starts]=63/20、E[cost]=129/20 JPY。
- quality の Flagged=end.flagged の期待件数と arrivals あたり率は 1/20。
  これは合流 binding を property から読む試験ではなく、許可された終端条件の品質集計。
- FI7 相当で fix の bad=false を捨てると、3/20 が flagged に移り、flagged=4/20=1/5。

## M5: 案件ごとの複写

- 到着は 0 と 10。最長 5 分なので互いの資源競合は無い。
- 各案件の確率は fixed=1/4、as_is=3/4、elapsed/work はそれぞれ 5 と 3。
- 案件 1 の completed_at は 5,3。案件 2 は 10+5=15,10+3=13。
- 同時確率は FF=1/16、FP=3/16、PF=3/16、PP=9/16。総和=16/16=1。
- 各案件 E[elapsed]=7/2、全体 E[work]=7、E[starts]=13/2、E[cost]=27/2 JPY。
- case_outcomes で案件を識別し、全体 terminals の時刻を案件時刻と取り違えない。

## 既存 DSL の証拠模型

`witnesses/M1.relca` から `M5.relca` は後段を枝の末尾へ手で複製したもの。
`models/` の新構文をパーサに通す前に、証拠模型で parser/lower と engine/reference を各々手値に照合する。
比較するのは判定・確率・終端・時刻・状態で、複製によって増える後段 site 数は同一でない。
M3 の証拠模型は a1 と a2 の異なる系譜を検査するが、新模型は段階 1 の未導出を検査する。
M1 の Deadline の反例は通常の deadline 到達前で止められず、5 分で破れを観測する。

join all の追加対照では、修正枝で fix 2分と追加の review 1分を並行に実行する。
経過は max(2,1)=2 のままなので完了は5分、workは追加の1分を足して6分になる。
非修正枝は経過・workとも3分。確率は修正1/4、非修正3/4のままである。

## 静的な壊した対と入口

B1〜B16 は ADR の表どおり、一つの欠陥を持つ入力。reason が定義された拒否は code と
message 中の reason の両方、severity=error、分類と exit を検査する。
B3/B5/B16 は既存 code だけが裁定されているため、新しい reason を発明しない。
B10/B11/B4 は既存の英語 reason をそのまま守る。
追加で定数・Actor 状態、repeat on_limit/公開 binding、recovery/応答別名、requires/property/metric/where、
contest/quantile の閉鎖を単独に検査する。合法な fork/join all、合流値を repeat の前で固定する対も置く。
compare/diff/mutate/sensitivity は merge_feature_combination、conform は U-LOG-1 と
合流を扱わない理由を検査する。単なる構文拒否や空ログ拒否で通さない。

## 故障校正の射程

新構文を実装する前には FI1/FI3〜FI9 の production の対応箇所が無い。
展開模型の単一編集と観測の単一破損で oracle の感度を校正し、実装済みの故障注入と区別して記録する。
FI1 は a の偽系譜行・unchecked 消失、FI3/4 は非修正枝の値を変更、FI5 は複写に待ち 1 分、
FI6 は案件 2 の複写を欠落させた観測、FI7 は修正書込み欠落、FI8 は reference 側だけ手値を破る対。
FI9 は B1 の正当な診断を消した観測を拒否する。FI2 は段階 2 の保証なので未実施とする。
旧入力の GREEN 試験は実装への一時故障でも RED を確認し、src の差を戻す。
