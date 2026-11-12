# ADR 0061 承認負担 — 合成観測の独立手計算

2026-11-02。裁定U1〜U8に基づく先行受入。実装の出力を期待へ写していない。
模型/JSONL/sidecarはsupport/approval_inputs.rs、受入はapproval_burden.rs、独立wire検査はsupport/approval_wire.rs。
入力は試験実行時にソース木内 .tmp/q8-* へ三つの通常ファイルとして具現化する。
期待は同supportのNumbers/Expectedの手定数。入力builderは期待値を集計して作らない。

## 観測範囲と基準

既存scopeはevent-contracts。新scopeはapproval-burden。
全てのID・時刻・版・結果は合成。実運用の追認率、ログの完全性/真正性、原本参照の実在は未確認。
profileはP.proposeのyesを承認提案、A.decideのapproved/returnedを決定、approvedを承認として明示する。
pendingは型域内だが決定でない。R.checkは2分、R.scanは3分。
proposal/approval/r1は別案件でも同じTask IDを使い、案件キーの混同を検出する。
同じPersonIdへのsource別名も区別しない。Actor名や人/AIラベルを意味の根拠にしない。

各案件のproposalは[0,1)、c1/c2のapprovalは[10,11)、c3は[12,13)。
提案outputと承認inputはそれぞれc1-v/c2-v/c3-vで一致。
再調査はc1:p[2,4)、c2:明示[]、c3:p[5,7)とq[7,10)。
同じpの別episode区間を重ねない。初稿のc3:p[2,4)では跨episode配分未対応になるため、採用文書でc3区間を訂正した。
時間2/0/5の手値は変更していない。片方の時刻を実装出力に合わせた変更ではない。

## A01〜A18と固定した手値

N=承認提案への承認数、D=対応が確定した承認提案と決定の組数。
U=率の未知episode、I=提案noなどの既知対象外、C0=承認未観測案件数。
K/TU=時間の既知/未知episode。率と時間は独立の分母。

| 行・試験名 | 手で固定した計算・結果 |
|---|---|
| A01 a01_baseline | yes→approved/returned/approvedでN2,D3、率2/3。時間2+0+(2+3)=7、K3、平均7/3、exit0 |
| a01_approved_control | c2 returned→approvedだけ。N3,D3、率1、時間7・平均7/3のまま、exit0 |
| A02 a02_ineligible | c4 no→returned、[]=0を追加。N2,D3,I1、時間7,K4、平均7/4、exit0 |
| a02_eligible_control | c4 no→yesだけ。N2,D4、率1/2、時間7/4のまま、exit0 |
| A03 a03_unobserved_case | headerにc5のみ追加。N2,D3のまま、C0=1、架空episodeは足さずU0、時間7/3、両coverage partial、exit2 |
| A04 a04_null | c1再調査eventを除いた同じ合成ログでリストnull。率2/3。c1時間は不明、既知0+5=5、K2,TU1、平均5/2、exit2 |
| a04_empty_control | 同じログのnull→[]だけ。率2/3、既知0+0+5=5、K3,TU0、平均5/3、c1 declared_empty_observation、exit0。実際に再調査しなかった証明ではない |
| A05 a05_missing_start / a05_missing_finish | c1再調査の一境界だけ欠損。未知after参照も除去しJSONL不正を混ぜない。率2/3、時間5/2、K2,TU1、exit2 |
| A06 a06_precision | c1再調査startの真時刻範囲[2,3]。2分へ一端補完せず時間null、集約は5/2、exit2 |
| A07 a07_same_tick | c1再調査finish4、承認[4,5)、after無し。両順序が可能、時間は未知、集約5/2、exit2 |
| a07_after_control | 再調査finish→承認startをafterへ一辺追加。時間2が確定、total7/mean7/3、exit0 |
| A08 a08_same_person_union | 単一c1、p[2,4)とp[3,6)の和集合[2,6)、4人・分。単純和2+3=5ではない。N1,D1、平均4、exit0 |
| a08_two_person_control | scanの担当だけqへ。p2+q3=5人・分、率1のまま、exit0 |
| a08_alias_control | scanのsource別名をalias-p→PersonId pへ明示対応。和集合は4のまま、exit0 |
| A09 a09_missing_approval_start / a09_cancel_ack | 単一c1。承認開始欠落、または正常finishをrequest/ackへ置換。ackはresult=null、承認決定ではない。N0,D0,U1、率null、K0,TU1、時間null、exit2 |
| A10 a10_two_decisions | 単一c1、同じ正常提案に対しapproved/returnedの二承認。N1,D2、率1/2。再調査2と[]=0、時間total2,K2,mean1、exit0 |
| a10_duplicate_finish | c1のfinish同phaseを別event_idで複製。二回決定に数えずc1未知。既知N1,D2,U1、率1/2、時間5/2、exit2 |
| a10_case_key_control | 基準と同じ。別caseの同じTask IDを結合しない、N2,D3、時間7/3、exit0 |
| A11 a11_missing_link | c1のsidecar episodeだけ除去。観測approvalを候補から消さずU1、N1,D2、率1/2。c1時間未知、既知5/2。r1は未割当一覧へ、exit2 |
| A12 a12_version_mismatch / a12_missing_version | c1承認inputを別版/nullへ。別提案から直近を自動選択しない。N1,D2,U1、時間5/2、exit2 |
| A13 a13_zero_denominator | 全提案no。N0,D0,I3、率null/zero_denominator。時間は7/3が残る、exit2。0/0を0や1にしない |
| A14 a14_reverse / a14_rename | 行とspec episode順の反転、または操作/Actor/対応表/profileの一貫した改名。N2,D3、時間7/3、exit0。hash/line/位置/ラベルは不変対象でない |
| A15 a15_unmapped / a15_cut_outside | c1承認Actorの対応なし/除外Actor対応。既知N1,D2,U1、時間5/2。未知一覧に両eventを保持、exit2 |
| a15_partial_start / a15_partial_end / a15_endpoint_null | c1端点false/null。局所数値N2,D3・時間7/3を消さないがscope未知・coverage partial、exit2 |
| a15_header_only | 三案件headerだけ、episode=[]。C0=3、episode未知数U0、D0、率null、K0,TU0、時間null、exit2 |
| a15_orphan_recheck | c1のr1が観測されているのにリンクは[]。率2/3、時間はc1未知で5/2。r1の二eventを未割当一覧へ残す、exit2 |
| A16 duplicate_key / unknown_field / missing_field | spec重複キー/未知欄/必須欄欠落は入力不正。E-LOG-0、数値null、checks/cases空、exit3 |
| duplicate_episode / double_assignment | 同episodeの重複/同一再調査Taskを二承認へ配分。E-LOG-0、数値null、exit3 |
| hash_mismatch | log対象hashのみ0にする（正常な他の対は機械更新する）。E-LOG-0、数値null、exit3 |
| future_schema | 未知schemaに将来版の未知構造を付ける。v1の欄不足より未知版を先に判定、U-LOG-0、exit4 |
| missing_task | c1再調査は未観測Taskへ明示参照。不存在を設定不正・0分にせずunknown、率2/3・時間5/2、exit2 |
| multiple_profiles | 有効profile二つの配列を明示。黙って先頭を選ばずU-LOG-1、数値null、exit4 |
| bad_result / bad_operation | profileの有限結果/操作名がモデルに無い。E-LOG-0、数値null、exit3 |
| A17 spec_boundary / spec_limit | JSON末尾空白だけで1048576/1048577 bytes。一つ目は基準値exit0、二つ目はtruncated/exit2で集約値null |
| episode_boundary / episode_limit | 有効な一意episodeを10000/10001件。byte上限に先に当たらないことを別GREEN試験が確認。10000は未観測Taskによりunknownだがepisode_limit無し、10001は切断で集約null |
| event_boundary / event_limit | headerは増やさず通常eventを10000/10001件。10000は未割当R候補unknown、10001は未読切断で集約null。headerのbyte上限によるinvalidを混ぜない |
| line_boundary / line_limit | 最初のevent行を65536/65537 bytesにJSON空白だけで調整。前者は基準exit0、後者はline_limit/exit2、集約null |
| order_limit | c1へ11個のproposal[0,1)を追加。同tick開始の相互順は未定で、既存excludesの全順序探索が実行全体予算へ達する。未知/exit2、truncated。独立数値の一律撤回は要求しない |
| A18 independent_violation | c1承認をpにしてseparation違反。同時にc1再調査null。独立違反exit1を保ち、時間既知部分5/2と欠損理由を残す |
| q1_unsupported | 選択processの提案にresponse_loss none。Q1のログ履歴は初回scope外、U-LOG-1、exit4。成功へfallbackしない |
| defer | CutでR.checkを保留。profileの時間指標全体をnot_configured/nullにし、時間K0,TU3。率2/3を保つがexit2 |
| cross_episode_overlap | c3のp[5,7)をp[2,4)へずらす。c1のp区間と別episodeで重なる。配分の根拠がなく時間集約null/unsupported、率2/3、exit4 |

## 既存scopeの手値・GREEN対照

正常基準は9Task。各Taskのpermission/lifecycle/duration/resultで4check、計36。
案件ごとのexcludesが3、合計39 conformant check。案件は3 conformant、violating/unknownは0。
positive evidenceは18event。モデルprocessは実行しないので、3件を超えるheader案件を勝手に捏造したり減らしたりしない。
既存coreとCLIの全Value一致も検査するが、上記の手値と独立wire検査を代替しない。
新flagを旧scopeへ渡す場合は既存argument/invalid/exit3。旧scopeにapproval_burden欄を加えない。
正常0、同人分離違反1、missing_start2、模型構文不正3、Q1履歴未対応4の五exitを別に固定する。
ledgerは既存の出力である。試験の失敗をledger欠落と同一視しない。

## 五つの壊した対と検査の独立性

1. 再調査欠損を追認へ加算: A04 nullでもN2/D3を要求。N3に変える実装を落とす。
2. 同人の重複区間の二重加算: A08は4、担当だけ別人なら5。同人で5を返す実装を落とす。
3. 未知案件を分母へ投入: A03はD3/C0=1、D4にする実装を落とす。
4. null/[]混同: A04は同じログに対してexit2/0、時間平均5/2と5/3を区別する。
5. 複数profileを一つ選ぶ: A16は同じprofile二つでもunsupported/4。先頭を選んでexit0にする実装を落とす。

これらのproduction故障注入と、次のwire負対照は別の検証である。
別系統のwire負対照は手組みcontrolに一箇所ずつN/D、総時間、平均分母、episode分類、必須/未知鍵の故障を入れて拒否を確認する。
未読spec切断は全体hashが不明なのでspec.sha256=nullを許す。非切断でのnullは拒否する独立対も置く。
これは未読部分を検証済みとしない契約による限定であり、既知入力のhash検査を緩めるものではない。
live試験は必ず実CLIへ接続し、schema/scope/approval_burden、数値、診断、stderr、exit、evidenceと入力非書換えを検査する。
最初に新schemaが無いことで落ちたとき、後段の数値/証拠検査を実行済みと報告しない。

## 内部検証の義務

- 新core入口を接続後、同じ手値をcoreへも照合し、engine/reference/simulation/probability非呼出しをspyで確かめる。
- A07の許容順序は再調査finish4と承認start4の二通り。afterを付けた側だけfinish先行一通り。
  小さい独立列挙器を下記試験で用い、行順や任意の一順序をoracleにしない。
- production故障は一度に一つ、compile成功後の意味不一致を観測し、復元する。受入期待を変更しない。
- 本受入作成の完了はREDの実行とGREEN対照まで。実装検証・故障校正・実ログ・独立レビューの完了ではない。
