# ADR 0041 read/write 候補の手計算 (実装前)

設計 v0.1 §6.3・§6.4、v0.2 §10・§22.3。単位は分。
新 fixture directory は増やさない。専用 core/CLI の inline 入力であり、既存 fixture 群・manifest は変えない。

## 核 R1 と壊した対 R2

2案件、到着0/0、A capacity2、x=false last_write_wins、y=false。
r は requires x==false else rejected、所要1、okだけを返す。
w は所要1、finish x=true、okだけを返す。各案件は r → w → done、FIFO case_id。
r は両案件とも0に開始、1に終了。w は両案件とも1に開始、2に終了。
各案件は done2、elapsed2、work2、truncated=false。
w の正常終了は同時刻の両順を展開する。後の w は stale な上書きなので no_lost_update A.x は violated。
候補は {(A.x,A.r,A.w)} の1行。候補追加は安全性を改善も悪化もさせない。
R2 は r の条件参照だけ y へ変更。W(y)=空、R(x)=空なので候補0行。
y はfalseのままなので、時間・終端・安全性はR1と同じ。

## 読取り種・集合・順序

R3: requires x==false && x==false は同一 reader の集合なので1行。
R4: atomic if x==false の読取りに置き換える。atomic の commit/reject はok/rejected、
commit枝だけ案件局所 touched=true を書く (共有の候補に影響しない)。
0に始まったrが1に終わる際、wはまだ書いていないのでrはok。時刻と安全性はR1と同じ。候補1行。
R5: process の if A.x==false は statement span.start の byte位置で reader を区別する。
両腕で同じ r → w → done を呼ぶため、分岐の結果で時間や安全性を変えない。
Capability r のrequiresはyを読む。候補はifの1行だけ。両腕の呼出しでwriterを二重計上しない。
R6: r と s がxを読む宣言を置くと、呼ばれないsも保守的な候補に入り2行。
宣言順r,s、writer wなので [A.x: A.r -> A.w, A.x: A.s -> A.w]。
R7: rのrequiresをyへ変え、w自身がrequires x==falseを持つと自己対(A.x,A.w,A.w)が1行。
R8: wのfinishをstartへ移してもwriter集合は同じなので1行。
R9: xとy両方を読むr、xとy両方を書くw、変数宣言順x,yで候補2行。
R10: 同じifを二つ置くとbyte位置が異なるので2行。非ASCIIの前置boundary文字列はUTF-8 byteで数える。

## 境界

cases1は候補0。共有xをcase-localにした入力も候補0 (同一案件のr→wだけ)。
per_caseはCapabilityを持たないため、文書Actorの観測値をprocess ifで読む既存受理形を使う。候補0。
capacity1や共通lockで直列化しても、共有変数の静的な対は残る (到達/同時生存を保証しない)。
評価専用contract/propertyにだけxが現れてもreaderなし。source Actorとkernel_reads_latentは候補なし。
通常条件から潜在を読む入力は静的invalid/E-POL-LATENT。新しい行で拒否を消さない。
無効構文はstatic拒否、候補を捏造しない。

## E9

verify の引数なし、存在するpath、存在しないpathの三つは全て同じ:
stdoutは unsupported 一欄と改行、exit4、stderrはADR0002追記の固定一行と改行。
check の存在しないpathは invalid/exit3のまま。verifyはcheckへfallbackしない。

## 互換

独立IR walkerで旧binaryの各Report位置について三つ組と順序を列挙し、旧末尾への追加位置だけを固定する。
新出力の対が固定表と一致しなければ、単に除去して旧hashへ比較するのではなく先に落とす。
許可無しjobは全byte、許可有りjobは専用行を厳密に除いたbyteを旧hashへ照合する。
旧のconcurrency説明文、全状態/性質/metrics/Agency、stderr、exit、source hash、ordered argvは保護する。
手計算/独立walkerは期待の根拠であり、まだ実装との一致や故障注入済みを主張しない。

## 意味 wire の単一故障校正

既存 aobs-capacity の source hash と旧末尾7行を固定し、手表の
Ledger.posted: Ledger.post -> Ledger.post をそのまま足すと受理される。
writer の post を past に1文字変えると、その source の手表とは一致しないので拒む。
別の未知field concurrency.unknown を足しても、許可された末尾全体とは一致しないので拒む。
いずれも安全性や時間を変えない偽の出力による照合器の校正で、実CLIの新行生成の検証ではない。
