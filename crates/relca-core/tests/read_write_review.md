# K5 取り込み前レビューの手計算 (実装を回す前に固定)

M1: two-if-sites の外側と内側は x の異なる reader。writer は A.w だけ。
元の二行を [outer, inner] とし、外側の then を残す bypass_guard では [inner]。
inner の byte span は保存されるので最後の行は不変、outer だけが消える。
比較のキーは同じ field が両側のいずれかで複数あるとき field と value の組。
outer の affects/field/value の三つの葉がそれぞれ文字列から null へ変わる。
これは位置による比較でも最後の value の置換でもない。
同じ私有比較入口には逆向きの追加、順の反転、別の重複fieldも与える。

major-2 (checkpoint承認済み):
field = concurrency.read_write_excluded
value = single_case=N; case_local=N (0を省かず、per_caseは含めない)。
affects = 一覧の対象外の静的な読取り/書込み対の種類と数。単一案件のfork兄弟もsingle_caseに含む。到達や同時実行を確定しない。潜在の読取りは数えない。候補の不在は競合なしの証明ではない。
同じ変数・reader・writerの集合の数で、分類はcase_local、single_caseの順に排他。
潜在は数えない。全0なら行なし。候補の後、既存固定末尾の前。

| 模型 | single_case | case_local | 候補 |
|---|---:|---:|---:|
| cases1 fork: A.r requires x / A.w finish x | 1 | 0 | 0 |
| 上のcases2の壊した対 | 0 | 0 | 1 |
| observed case xの逐次reader/writer (cases2) | 0 | 1 | 0 |
| per_case Doc.flagのifのみ | 0 | 0 | 0 |
| 共有A.xの候補+案件局所zのrequires/writer | 0 | 1 | 1 |
| readerを未書込みyに変えた模型 | 0 | 0 | 0 |
| case_localの同一requires内でxを二度読む | 0 | 1 | 0 |
| case_localのxを読む未使用sを足す (r,s × w) | 0 | 2 | 0 |
| さらに未使用tを足す (r,s,t × w) | 0 | 3 | 0 |
| 単一案件でrが共有x,yを読みwが両方を書く | 2 | 0 | 0 |
| 単一案件の共有xはr×w、局所zはr,s,t,u,v×w | 1 | 5 | 0 |

上の未使用Capabilityも静的三つ組に含む。この手の2/3/5を独立IRの値から採らない。
mixedの1対を品質property・metrics waiting・simulationへ運ぶ入力でも、開示は
候補の後の1行で、既存の固定末尾塊の直前となる。

per_caseの正の対は受理済み模型では作れない。Doc内CapabilityはU-SEM-0、
他ActorからDoc.flagへのwrites/効果は既存unsupported。この2形の実CLI拒否と、
読みのみの0対を固定する。到達しない分類の内部IR seamは作らない。

minor-5: set<CaseId> keys を requires の Contains と Or の両腕で重複して
読んでも reader A.r は1つ。A.w.atomicの条件もkeysを読み、そのatomic.writesで
keysへinsertする。候補は順に A.keys: A.r -> A.w と A.keys: A.w -> A.w の二行。
Orの右腕を別の未書込み集合otherへ変えてもこの二行は同じ。両腕をotherへ変えると
A.rの行が消え、A.wの自己対だけが残る。これは独立IR oracleからの採取ではない。

## 既存inlineの4 source (手計算)

いずれも共有Actorの候補は0、case_localは1、single_caseは0。構造上の対なので
範囲外書込みで停止するrange-stopのifにも到達性で刈らず1を残す。

| limits legacy control | 読み | 書き |
|---|---|---|
| requires-start-same-time | flag: A.run.requires | Writer.close.finish |
| requires-after-queue | flag: A.run.requires | A.close.finish |
| initial-product | route: process.Flow.if | Chooser.chooseの結果代入 |
| range-stop-with-queue | n: process.P.if | A.inc.finish |

4 sourceは同じPRE-FIX binaryで旧byte/旧indexを採取し、候補payloadの採取とは区別する。
limitsとC5が同じsourceを持つため8記録だが4 unique sourceである。
