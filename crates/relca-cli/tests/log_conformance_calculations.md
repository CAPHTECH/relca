# ADR 0057 ログ適合の手値と受入境界

状態: 受入先行。実装の出力から期待を採取しない。実行結果の RED / GREEN は作業記録に別記する。
根拠は設計 v0.1 §16・§18、v0.2 §20・§22.1 と ADR 0057 の決定1〜7。
fixtureの代わりに実CLIの専用integration targetに置く選択であり、既存fixture群や39行対応表を拡張したとはしない。

## 入力と期待の所在

- log_conformance.rs: 実CLIをローカルファイル入力で動かす。stdoutの全必須欄と三値・verdict・exitを照合し、その後で手値を照合する。
- support/log_conformance_inputs.rs: 全ての模型、合成JSONL、期待分類、診断、理由、証拠ID。各caseは入力だけから構成され、productionのparserやmonitorを呼ばない。
- support/log_conformance_wire.rs: 実出力のvalidatorと、手組みの完全なwire control。部分期待そのものを結果として通さない。root鍵の全欠落、型/exit/診断severity等の単一故障を拒否する。

JSONLは実行時に .tmp 以下へ書き出す。生の内容・期待を実装出力から書き換えず、試験はexpected更新機能を持たない。模型・ログの実行前後のSHA256を外部のsha256sumで検査し、入力更新副作用を拒否する。
relative TMPDIRはworkspace rootに対して絶対化する。実ログ、文書本文、実名は使わない。固定の仮名と分類、版IDだけである。

## 共通の手計算

模型Mの所要は全て1分。Clerk.enter / Approver.approveはpassまたはfail、Poster.postはcommittedを正常結果としcommitsを持つ。PersonIdはpとq、全Actorは両方を実行者に持つ。
通常窓は閉区間0..20000分、案件cの両端点はtrue。これは完全記録の証明ではない。入力・出力版は不透明な仮名であり、意味判断に使わない。
初回scopeはevent-contracts。模型のprocessはstatic loweringのために存在するが、process順序、容量、work-conservingは照合対象外。便宜上verify宣言を足してunsupportedを隠すことはしない。
既存lowerは要求無しの模型をU-SEM-0で拒否する。追加裁定に従い、取消/空ログには excludes Approver.approve.start, Poster.post.start、event境界には excludes Poster.post.start, Clerk.enter.start を明示する。取消では最初のstartを観測し後者を観測しないので適合、event境界でもpost.startを観測しClerk.startは無いので適合。これらに違反・未知を足して他の行の手値を変えない。空ログでは起動の証拠が無いためunknownのままである。要求を全て除いた模型は別の負対照wire_no_requestsに残す。
bounds cases2に必要なarrivals fixed [0min,0min]も全模型に明示する。これは観測ログの流入やprocessを再生して適合を証明する指定ではない。

通常の三Task:

| Task | start | finish | 担当 | 観測結果 |
|---|---|---|---|---|
| e / Clerk.enter | 0 | 1 | p | pass |
| a / Approver.approve | 1 | 2 | q | pass |
| z / Poster.post | 2 | 3 | p | committed |

同tickの先行は e.finish→a.start、a.finish→z.start を明示afterで保証する。時間差は各1。requires_event z.start after approve.finish(pass)は全許容順序で満たされる。通常のcommitはz.finishの1件。

## ADR行との対応と一故障の対

表のexitはADRの運用verdictに対応する。0=satisfied / conformant、1=violated / violating、2=inconclusive / unknown、3=invalid / unknown、4=unsupported / unknown。vacuous / exit5は使わない。
各変更版は一つのEditを親に適用する。Task全体の再割当・移動・記録喪失は一つの業務上の変更として、関連する境界・after・case宣言を一緒に整合させる。二つの独立故障を重ねない。独立違反と非対応を併置するL21は、既に違反する親へ非対応要求を一条だけ追加する。

| ADR行 / case | 親または基準 | 一変更と手値 | 期待 |
|---|---|---|---|
| L01 l01_serial_ok | 上記三Task | 差1/1/1、成功承認が必ず登録startに先行 | conformant / 0 |
| L02 l02_missing_start / V01-19 | L01 | e.startを一行だけ除く。eの所要を0や1と補完できない | W-LOG-0 missing_start / 2 |
| L03 l03_missing_finish | L01 | z.finishだけ除く。startは正常完了でもcommitでもない | W-LOG-0 missing_finish / 2 |
| L04 l04_time_precision | L01 | e.startの時刻だけ0..1。finish=1なので差は0または1 | W-LOG-1 time_precision / 2 |
| L05 l05_partial_start | L01 | from_case_start=false | W-LOG-2 partial_start / 2 |
| L05 l05_unspecified_start | L01 | from_case_start=null | W-LOG-2 scope_unspecified / 2 |
| L06 l06_partial_end | L01 | through_case_end=false | W-LOG-2 partial_end / 2 |
| L07 l07_missing_approval | L01 | 承認Task全体の記録を失う。dangling afterも同じ記録喪失の整合処理で除く | W-LOG-2 / 2。承認不実施の証拠にしない |
| L07 l07_failed_approval | L01 | a.finishの結果だけfail。成功承認は観測されないが、他の未記録承認が無いとは言えない | W-LOG-2 / 2、E-LOG-2は無し |
| L08 l08_separation_ok | L01 | separationを一条追加。p≠q | conformant / 0 |
| L08 l08_same_person | separation_ok | finishの担当だけpに変更。start=qと矛盾し、同一Taskの対応が不明 | W-LOG-0 task_identity_mismatch / 2 |
| L08 l08_distinct_person_control | 二Task、separationのみ | e=p、a=q | conformant / 0 |
| L08 l08_same_person_witness | distinct_person_control | 承認Task全体の担当だけpへ再割当。両境界がp | separation、証拠e.finish/a.finish / 1 |
| L09 l09_once_ok | post t1の0→1、once一条 | cの確実なcommit=1 | conformant / 0 |
| L09 l09_duplicate_commit | once_ok | t2の1→2を一Task追加。同じcでcommit=2 | at_most_once、二つのfinishが証拠 / 1 |
| L10 l10_distinct_cases | duplicate_commit | t2だけ案件dへ移す。c=1、d=1 | conformant / 0 |
| L10 l10_reused_task_id | distinct_cases | d内のtask_idだけt1へ改名。キーはcase_idとの組 | conformant / 0 |
| L11 l11_duplicate_finish | once_ok | 同じTaskのfinishを別event_idで再記録。二回実行とは言えない | W-LOG-0 duplicate_phase / 2 |
| L12 l12_excludes_violation | eの0→1、aの2→3 | 1<2なのでe.finishがa.startに必ず先行 | excludes、e.finish/a.start / 1 |
| L13 l13_ambiguous | eの0→1、aの1→2、excludes | 同時刻の二順序。e.finish先行なら違反、a.start先行なら適合 | W-LOG-1 ambiguous_order / 2 |
| L14 l14_forward | ambiguous | a.start.after=e.finishを一辺追加 | 全順序でexcludes違反 / 1 |
| L14 l14_reverse | ambiguous | e.finish.after=a.startを一辺追加 | 全順序でa.startが先、適合 / 0 |
| L15 l15_unmapped_actor | L01 | actor_mapのapprover対応一件だけを除く | W-LOG-3、unmapped_actor.no_actor_mappingに承認両イベント / 2 |
| L16 l16_unmapped_person | L01 | executor_mapのq対応一件だけを除く | W-LOG-3、no_executor_mappingに承認両イベント / 2 |
| L17 l17_cancel_ok | 単一の承認Task、cooperative | start0、request1、ack2。ackの遅れは1分。正常finishとcommitは0件 | conformant / 0 |
| L17 l17_request_only | cancel_ok | ackだけ除く。要求は完了ではない | W-LOG-0 missing_cancel_ack / 2 |
| L17 l17_missing_request | after無しの取消観測 | requestだけが欠けた観測。startとackからrequestを生成しない | W-LOG-0 missing_cancel_request / 2 |
| L18 l18_forbidden_ack | cancel_ok | 模型のcancelだけforbidden_after_startへ変更 | 成功ackが正の境界違反、cancel_boundary / 1 |
| L19 l19_bad_mapping | L01 | map宛先を存在しないGhostへ変更 | E-LOG-0 / 3。対応表の欠如と区別 |
| L20 l20_future_schema | L01 | relca-log/v2へ変更 | U-LOG-0 / 4 |
| L20 l20_unknown_flag | L01 | 未知CLI flagを一つ追加 | E-LOG-0 / 3 |
| L21 l21_response_unsupported | L01 | responseを一条追加 | U-LOG-1 / 4。要求を捨てて合格にしない |
| L21 l21_independent_violation | 同一PersonIdの正の違反 | responseを一条追加 | U-LOG-1行を残し独立のseparation違反を優先 / 1 |
| L22 l22_empty | header、cases空、event無し | 証拠0件。空虚な成功は不可 | W-LOG-2 no_evidence / 2 |
| L23 l23_duration_violation | once_ok | finish時刻だけ1→2。差2≠所要1 | duration、start/finishが証拠 / 1 |
| L23 l23_event_boundary | 完全な5000Taskと上記excludes | Task iのstart=2i、finish=2i+1。厳密な時刻順、10000eventで上限ちょうど。禁止対象のClerk.startは観測されない | conformant / 0、truncated=false |
| L23 l23_event_limit | event_boundary | 10001番目のeventを一つ追加 | W-LOG-4 event_limit / 2、truncated=true |
| L23 l23_line_boundary | onceの開始行 | input_versionのpaddingを手の算式で調整し、UTF-8で65536bytes。日本語1文字を含む | conformant / 0 |
| L23 l23_line_limit | line_boundary | paddingにASCII1byteだけ追加、65537bytes | W-LOG-4 line_limit / 2、truncated=true |

byte境界の算式: padding前の行長をB、置換する版の長さをVとすると、新しい版は65536−B+V bytes。日本語の1文字は3bytesなので、ASCII paddingはさらに3を引く。数値はJSONの実出力ではなく合成入力のシリアライズ長に対して計算する。
順序予算100000は固定出力を検査する。どの最適化実装でも必ずその探索数を使うと保証できる入力は今回設けていない。実際のorder_limit到達の校正は未実施として残す。event/lineの二つは入力境界で直接到達させる。

## 追加wire・ID・境界の手値

全てonce_okまたはL01の単一変更で、元のモデルは静的に有効。

| case | 変更 | 期待 |
|---|---|---|
| wire_missing_field | event.task_idだけ除く | E-LOG-0 / 3 |
| wire_unknown_field | 許可されないpayload欄を一つ追加 | E-LOG-0 / 3。秘密sentinelをstderrへ転載しない |
| wire_duplicate_key | 同一行にtask_idを二つ書く | E-LOG-0 / 3。serdeのlast-winsで受理しない |
| wire_duplicate_event_id | 二つのevent_idを同じ値にする | E-LOG-0 / 3 |
| wire_after_unknown | 存在しないafter先を一つ追加 | E-LOG-0 / 3 |
| wire_after_cycle | 自分自身へのafterを一つ追加 | E-LOG-0 / 3 |
| wire_after_time_contradiction | start0にfinish1のafterを追加 | 利用者証拠の不整合、E-LOG-1 / 3 |
| wire_reversed_lifecycle | start時刻だけ2へ変更、finishは1 | 入力afterの矛盾ではなく正のlifecycle違反 / 1 |
| wire_outcome_domain | committedを型域外の文字列へ変更 | result_domain / 1 |
| wire_result_type | resultをobjectへ変更 | E-LOG-0 / 3 |
| wire_window | window.toを0に変更、finishは1 | 窓の外なのでE-LOG-0 / 3 |
| wire_unknown_operation | finish.operationだけPoster.unknownへ変更 | W-LOG-3、unmapped_operation、unknown / 2 |
| wire_cut_undeclared | 模型のcutだけ除く | 適合0のままW-CUT-0、cut=undeclared、定型注記保持 |
| wire_null_version | 入力版一つをnullへ変更 | このscopeで版の制約は無い、適合0 |
| wire_equal_time_requires | L01のa.finish→z.startのafterだけ除く | 同時刻の前後が未確定、ambiguous_order / 2 |
| wire_future_scope | scopeをwhole-processに一値変更 | U-LOG-0 / 4。既存checkへfallbackしない |
| wire_missing_scope | 必須scopeだけ除く | E-LOG-0 / 3 |
| wire_future_unit | time_unitだけsへ変更 | U-LOG-0 / 4。分に丸めない |
| wire_no_requests | once_okの唯一のcontractだけ除く | 既存静的gateを保持しU-SEM-0 / 4 |
| wire_explicit_task_edge | once_okのfinish.afterにstartを一辺追加 | 同一Taskの明示因果は正常、適合0 |
| wire_source_cycle | explicit_task_edgeのstart.afterにfinishを一辺追加 | 入力の二頂点cycle、E-LOG-1 / 3 |
| wire_no_implicit_actor_match | map無しの承認Taskのsource Actor IDだけをApproverへ改名 | モデル名と同じでも暗黙対応しない、no_actor_mapping / 2 |
| wire_no_implicit_person_match | map無しの承認Taskのsource担当者IDだけをqへ改名 | PersonIdと同じでも暗黙対応しない、no_executor_mapping / 2 |
| wire_operation_actor_mismatch | Approverのmap宛先だけClerkへ変更 | 型として有効なmapでも操作の所有者と違うのでunknown、operation_actor_mismatch / 2 |

取消の追加対: l17_cancel_without_explicit_edge はcancel_okのafterだけ除いた正常対照で、同Taskの意味的先行は保たれる。l17_missing_requestはこの対照からrequest一行だけを除く。l17_finish_approval_controlは正常な承認0→1と登録3→4でrequires_eventが適合する対照。l17_ack_not_approvalは承認Taskだけをstart0/request1/ack2に置き換える一つの取消という変更で、正常finish(pass)が無いためunknown / 2。ackを成功承認へ変換する実装を拒否する。

live_partial_order_is_independent_of_jsonl_line_order はL13/L14の三入力のevent行を反転する。headerは先頭に残す。三値・exit・証拠event_id集合は同じで、元bytesのlog SHA256は異なる。JSONL行番号やhashを意味順序として使わない。
全てのevidenceは実際のevent_id・case_id・task_id・JSONL行番号へ逆照合する。正の違反証拠の期待IDは、他のlifecycle行の寄せ集めでなく、該当reasonのviolating行に存在することを要求する。L21では独立違反を優先した後もsummaryのunknown/violating件数が正であることを確認する。

## 検査自身のGREENと、REDの読み方

green_models_are_statically_valid_without_running_the_engine は全模型を既存syntax/lowerへ通す。wire_no_requestsだけは指定したU-SEM-0を要求し、他は全て静的受理を要求する。これはログ適合の合格ではなく、受入が誤った模型文法で落ちることを防ぐ検査。
green_hand_inputs_have_one_declared_edit_and_no_captured_expectations は親から宣言した一Editだけで変更版ができ、no-opでないことを確認する。
green_full_wire_control_and_single_fault_rejections は手組みwireを通し、必須欄欠落、partial result、exit不一致、時刻/診断/切断等の故障を一つずつ拒否する。productionと共通のvalidatorは使わない。
green_unknown_command_still_fails_closed は無関係な未知commandのinvalid3を保存する。conformが無いことを恒久的に要求する試験ではない。
green_small_order_and_interval_hand_oracles_are_independent は4イベントの全24順序を独立に列挙し、時刻制約後に2順序、L14の各afterでは1順序になることと、L04の差0/1を確認する。productionのDAG/monitorを呼ばない。

実CLIの試験は、global usageと旧invalidだけが返る場合にENTRYPOINT_MISSINGとして明示的に落ちる。これをfuture対応としてskipしたり、invalid3を期待するcaseのGREENに数えたりしない。実装後は同じ試験がfull wireと手値を実際に照合する。
故障注入済みと呼べるのは手組みwireの単一変更拒否だけ。production monitorへの故障注入、実ログ、現行snapshot接続、order_limitの実到達はこの受入作成段階では未実施。V01-19をcoveredに変更しない。
