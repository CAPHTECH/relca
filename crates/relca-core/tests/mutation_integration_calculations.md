# Mutation と他機能の組合せの手計算

閉じた mut2 79 入力・61 手編集・旧数値は変更しない。交差入力は試験内に置く。
根拠は ADR 0046「意味」「互換」と ADR 0038〜0045。手計算と、末尾の変更前 binary による照合記録を分ける。

| 接点 | 一つの編集と手計算 |
|---|---|
| T13d | Entry を Verdict に置換した共有 flag writer。version_check の有無だけを変更。共有書込みが既に除外理由なので両側 summary NA。done 1+1+1+1=4、rejected=3、work 同値。version_check 単独の除外は私有 IR で書込みを消して別に検める。公開の検出力と混同しない |
| T13h | X の bounds を Day に移すだけ。到着0・approve終了1>expiry0なので1 mutant detected。scenario の宣言/選択は original と mutant で保持。選択無しは E-CONF-0/invalid、6生成種は blocked、試行0。horizon=1 は inclusive、clock=10と別で切断ではない |
| T20 | S の先頭に snap writer1分を挿入。Extract は snap を読む。Sources(e)={Invoice}、aはsubject Invoice/independent空、group判定数3→4、3行ともheld。done5→6/rejected4→5、work同値。Writerのreadsだけを消した故障はSources(e)=空、候補は残りno_assigned_check |
| T20複数writer | Earlier(Archive)1分をさらに先行。C5は最後のwriterだけでなく全先行writerを読む。Sources(e)={Archive,Invoice}、group3→4。done6→7/rejected5→6。record全体readsも同じ。欄readsは別の静的拒否 |
| A4 | Entryをinline enum欄を持つrecordへ変更し、Approver引数を同じe二回にする。手編集はe_dup挿入とaの二引数だけを置換。e.confidenceのguardはeのまま。done4→5/rejected3→4、3held。旧guard演算子のaffected_bindingsが欄条件eを挙げないことはADR0046 U17どおり変更しない |
| D10/B6 | Sの判定を常にpassに限定。Timely=done within4、分母arrivals。元done4→Rate1、複製done5→Rate0。qualityは両Reportで運ぶがC6の閉じた11欄other_changesには含めない。私有Report対でqualityだけ変えても比較結果・assigned・otherは不変。C6単独では種7はquality_denominator_unavailable。B7取り込み後は字句終端rejected/doneの2候補を構造比較しdetected、report:null、other空。通常のsummaryの品質差の扱いは変えない |
| A6 | 固定Rule.required=trueがifを制御。RuleChangeのchange grantだけならexpiry候補0。通常authority_basisも名指せばRuleChange1箇所、change grant/not_by/basisは編集で不変。期限0の編集は既存のrestricted rule-change Authority拒否でinvalid_mutant/static_unsupportedとなり、detectedにはしない(下の訂正)。Ruleは値を変異させず、Judgement::Ruleをprocess judgementへ混同しない。品質宣言時だけquality aborted/quality.ruleをoriginalと全mutantに運ぶ |
| A6動的改定 | process内amendは従来のunsupported。original gateで6種blocked、種5rule_version_unavailableは変えない。configurationの静的amendモデルはselector無しmutateがinvalid、規則の値を勝手に選ばない |
| B4/B5 | Heldは成功/失敗へ丸めず到着順にdrain。overflowは業務の結末、horizonは業務観測、clockは計算切断。各対の元と独立手編集を両backendで手計算へ照合する |
| C4 | 効果contestのAuthority失効は既存のrestricted Authority拒否を保持しstatic_unsupported。contest_pathsの効果target・reversalの旧行を運ぶ。dynamic失効を生成したとは呼ばない |

T20の欄readsはwhole-recordの読取りと区別する。既存の
`agency_second_half.rs:463` の `reads_record_whole_value_adds_writer_parent`
の受入は `reads [entry.ok]` を U-SYN-0 とする。正例はwhole-record
readsとfield guard、負例はfield readsの件数・message・spanを固定する。

追加の手計算: Held S は全判定pass、到着0/1、同時案件1なので元の終了4/8、
elapsed4/7・work4/4。独立手編集の終了5/10、elapsed5/9・work5/5。
horizon5では第2案件cut_off、clock5では計算切断・inconclusive。
overflow対は到着0/0、wait_externalだけをend spillへ替え、第2案件は時刻0の
業務overflowでありtruncatedでない。end spillは共通のconcurrent項目に置く。
scenario内のon_overflow endはADR0042 U10どおり項目spanのU-SEM-0で拒否する。
C4 sov-basicにauthority_basisだけを足すと
元のreversalは適合するがexpiry0はrestricted Authorityの静的拒否を保つ。
LWWの同時2書込みは実行自体は許されるがno_lost_updateはviolated、
version_checkの同時2試行は一つconflictとなり更新の範囲を保つ。
remove_readはどちらの共有更新にも触れず、元の性質の行と結末は不変。

fresh-decides: Rule の decides e_dup もActor局所の
使用済み名。対照ではe_dup、欄を1個足した側ではe_dup2。値自体は変異しない。

rule-expiry の検算: 初稿のRule手計算は
approve終了3>expiry0だけを見てdetectedとしたが、その前に静的拒否される。
BASE d17f9e1の手入力でexit4、U-SEM-0 restricted rule-change Authority is unsupported。
`rule-restricted-expires-unused.broken-s14expires1/expected.json` も未使用を含む拒否を固定し、
lower.rs::authority_declarationsはrule_grantsがありrestrictedなら拒否する。
underに名指すAuthorityという文書の限定とcodeの差を区別する。
ただし既存の境界レビュー裁定(旧318行)は既にU7をchange-grant全部へ掛け、
新基点b6951fbの追記も同じ訂正を明示する。本文よりこの裁定を優先する。
意味を広げず、mutant1件と全AST保持、invalid_mutant/static_unsupported、
assigned空、run inconclusiveを要求する裁定を採る。

T20のstart/atomic writerも同じ1分でsnapを作るのでdone5、group3→4、subject Invoice。
atomicはflag=false固定でsnap=entry側だけを実行し、通常成功と拒否を混ぜない。
初稿のatomic入力だけはobserved caseブロックを2個にしてしまい、既存の
duplicate case state blockで止まった。snapとflagを同じブロックへまとめ、
時間・根拠集合・期待結果は変えずに再実行する。この初稿REDを製品の欠陥に数えない。

W-LIN-2交差: snapのWriterがlatent badもSeed kernelで書き、2consumerの
KA/KBがbadをgivenに読む。祖先producerはC5の観測辺でつながるWriterのw。
Clerkだけが新しいPurchaseOrderを読むので警告はApproverの1件。Extractの
複製後も両方の祖先は同じwなので1件のまま。検出には割り当てず、wrapperの
unchecked W-LIN-2を保つ。元done5、複製done6、時間を確率に置き換えない。

品質とhorizonの交差: 全passのSにDay到着0、horizon4、clock10。
元done4はinclusiveな端で完了しTimely within4のRate1。複製は所要5となり
horizon4でcut_off、reachable end.doneはinconclusiveだがbounds.truncated=false。
Timelyの期限4は既に閉じたmissなのでRate0、未確定の成功へ丸めない。
horizon6の対照では元done4/複製done5がどちらも完了しRate1/0。
horizon-summary: ReportがinconclusiveならMutation行もinconclusiveであり、
静的に3heldを導けてもdetectedに数えない。detectedのReportをsatisfied/violated/
vacuousに限る既存wireは緩めない。heldの運搬は任意とし受入で強制しない。
horizon-reason: 既存語assigned_check_unavailableを使い、新語を足さず、
truncatedとは呼ばない。heldはdetected専用という照合器の制限も緩めない。

## R2 held の入力別の手計算 (2026-10-29、変更前 check の実行前に固定)

統合時の裁定に従い、`held()` の判定数を入力ごとに渡す。
実物の呼出しは11か所、freshの対照ループを展開すると12入力(依頼の9か所ではない)。
数えるのはprocessの判定の呼出しであり、実行回数、Actor数、Ruleの項の数ではない。

Sの呼出しは `e = Extract.make()`、`p = Clerk.match_po(e)`、
`a = Approver.approve(e)`、`z = Ledger.post(p,a)`。
効果zの祖先/guardにあるもののうち、実引数または観測readsの先行writer辺を持つ
呼出しが判定となる(既存C5の `agency.rs` の候補条件)。pとaは各1個。
Invoiceへの直接readsだけを持つeは引数も観測辺も無く、判定には入らない。
一方、snapを読むeは先行writer辺を持つので判定が1個増える。
snapのwとfirstは引数も観測readsも無いため判定には入らず、z自身は効果である。
複製したe_dupも同じ条件で数える。重複実引数は呼出しを増やさず、
recordの欄guardもeに実引数/観測辺を追加しない。Ruleの固定値もprocess判定ではない。

以下の群はそれぞれ1群だけ。pのindependent_sourcesはPurchaseOrder、aは空。
eを数える側でもeのindependent_sourcesは空なので群の独立源の和はPurchaseOrderのまま。
subject_sourcesとindependent_sourcesの割当て2行は厳密一致、判定数の行は非減少のheld。
「3 held」は3判定の意味ではなく、割当て3行のdirectionである。

| held呼出しの入力 / 試験名 | subject_sources | beforeの呼出しを手で列挙 | afterの呼出しを手で列挙 | before→after |
|---|---|---|---|---|
| observed(true,false,false) / t20_observed_read_sources_follow_writer_not_direct_reads_only | Invoice | e・p・a | e・e_dup・p・a | 3→4 |
| observed(true,true,false) / t20_all_lexical_preceding_writers_are_included | Archive,Invoice | e・p・a | e・e_dup・p・a | 3→4 |
| observed(true,true,true)+e.confidence guard / t20_whole_record_observed_read_keeps_sources_and_field_guard | Archive,Invoice | e・p・a | e・e_dup・p・a | 3→4 |
| record() / a4_record_repeated_arguments_and_guard_are_not_silently_rewritten | Invoice | p・a | p・a | 2→2 |
| quality() / d10_quality_delta_is_carried_without_becoming_an_assigned_or_other_check | Invoice | p・a | p・a | 2→2 |
| rule(false,false) / a6_change_only_authority_is_not_an_expiry_site_and_rule_values_are_preserved | Invoice | p・a | p・a | 2→2 |
| rule(false,false), occupied=false / a6_fresh_names_include_decides_fields_and_keep_the_no_collision_control | Invoice | p・a | p・a | 2→2 |
| rule(false,false)+decides e_dup, occupied=true / 同試験の占有側 | Invoice | p・a | p・a | 2→2 |
| held_summary(None,20), Day選択 / held_selected_summary_reuses_same_scenario_without_converting_held_to_success | Invoice | p・a | p・a | 2→2 |
| observed(true,false,false)+start writer / t20_start_writer_is_not_missing_from_summary_source_ancestry | Invoice | e・p・a | e・e_dup・p・a | 3→4 |
| observed(true,false,false)+atomic writer / t20_atomic_writer_is_not_missing_from_summary_source_ancestry | Invoice | e・p・a | e・e_dup・p・a | 3→4 |
| observed_wlin2() / t20_wlin2_of_the_observed_writer_is_carried_but_never_assigned | Invoice | e・p・a | e・e_dup・p・a | 3→4 |

record/quality/Rule/Heldでは複製e_dupも引数・観測辺を持たないため、判定数は増えない。
Rule占有側の複製名は裁定によりe_dup2だが、これも群に入らず2→2。
Heldの到着2件は同じprocessの実行であり、静的な群の判定数を2倍にしない。
観測readsの6入力は複製が群に入るので3→4であり、88c4117の観測祖先を持つ
e・p・aからe・e_dup・p・aへの前例と同じである。

### 変更前 binary の照合記録 (beforeのみ、各入力1回)

手の表を固定した後、`pre-c6-relca-d17f9e1` の check を元入力だけに各1回実行。
binaryのSHA256は `c46e7049ced2be488e30467b205e4a8679c4acc9344226b0e75238584820c79c`。
上記support関数と試験内の置換を使って入力を実体化した。コマンド形は
変更前binaryでのcheck、
held_dayだけ末尾に `--scenario Day`。いずれもexit0。
以下は手計算の期待の転記ではなく、元checkの観測と事前の手計算との照合結果である。

| 入力名 (上表の順) | 観測lineageのcall | 観測lineage_groups.judgements | 手のbeforeとの一致 |
|---|---|---|---|
| observed | e,p,a | 3 | 一致 |
| multiple | e,p,a | 3 | 一致 |
| whole_record | e,p,a | 3 | 一致 |
| record | p,a | 2 | 一致 |
| quality | p,a | 2 | 一致 |
| rule | p,a | 2 | 一致 |
| fresh_control | p,a | 2 | 一致 |
| fresh_occupied | p,a | 2 | 一致 |
| held_day | p,a | 2 | 一致 |
| start | e,p,a | 3 | 一致 |
| atomic | e,p,a | 3 | 一致 |
| wlin2 | e,p,a | 3 | 一致 |

全12入力とも1群だけで、subject_sourcesの集合も手の表と一致した。
multiple/whole_recordの元Reportの配列順はInvoice,Archive、mutationの割当てでは
既存の正規化順Archive,Invoiceを要求する。群の数はこの並びに依存しない。
初回の一時照合器はstderr空と配列順まで誤って要求したため不一致としたが、
判定数/callは12件とも最初から一致していた。既存の警告とActor宣言順を切り分け、
同じ保存済みstdoutを再照合した(binary再実行0回)。判定数の期待は変更していない。
この12入力のbeforeの観測は上表、変更前binaryのSHA256は本節の本文に保存した。
この表は当時の照合結果の記録であり、現在のソースで同じbinaryを再現する手順ではない。
受入試験はこの観測値を実行時に読み込まない。

### 今の実装で確認した範囲と残り

`TMPDIR=.tmp CARGO_NET_OFFLINE=true cargo test -p relca-core --test mutation_integration --quiet`
を1763717のsrcに対して1回実行し、既存29試験は19成功/10失敗だった。
失敗はR1のscenario選択panicが3本、R2のundetected対detectedが6本、
R3のe_dup対e_dup2が1本。判定数のbefore/afterのassertで落ちたものは0本。
R2の6本はheldの入口のresultで止まり、3→4の割当ての意味はまだ確認していない [U]。
R1のHeld行とR3の占有側も後段のassert未到達 [U]。
直接readsだけのrecord/quality/Ruleとfresh非占有側の2→2は通過した。
これは当時の一回の実行観測であり、現在の全試験の結果ではない。

追加のhelper校正は、手で構成した3→4と2→2の行を受け、before/afterのどちらかを
0へ壊すと必ずassertに失敗することを要求する。これは照合器の校正であり、
Mutationがその行を生成した証拠ではない。変更前binaryで確認したのは元入力の
beforeだけであり、観測readsのafterとMutationの行全体はR1/R2/R3修正後の
再実行へ渡す [U]。全体GREENや実装の故障注入済みとは呼ばない。

helper校正の単独実行は1成功/0失敗、29 filtered out、exit0を1回観測した。
最初の単独実行コマンドはcargoとtest harnessの `--exact` の区切りを誤り、
実行前の引数エラーで停止した。`-- --exact` へ直した上記成功だけを試験実行と数える。
当時のログ名は `held-helper-test.log` (同archive)。
`./scripts/check.sh` はfmtとclippyを通過してworkspace testへ進んだが、
120秒のtool timeoutで停止したため完走・全体合格は未確認 [U]。
当時のログ名は `held-check.log` (同archive)。節参照は別に `python3 -B scripts/check-refs.py` を
1回実行してexit0。src・閉じたfixture・互換manifestは変更していない。

## C6 取り込み後レビューの追加受入 (19123f21、実行前の期待)

### root constraint: 行の比較に入れる Report の正の集合

変異体のverdictが satisfied / violated / vacuous のいずれか、かつ
bounds.truncated = false のときだけ detected / undetected を許す。
種0・1・2・4・8・9で同じ前提。種3・5・6・7は未対応のまま候補0。
先に静的invalid→invalid_mutant/static_invalid、unsupportedでagency無し→
invalid_mutant/static_unsupported、truncated=null→inconclusive/runtime_unsupported、
truncated=true→inconclusive/truncated。これらの優先順位を変えない。
残るunsupportedかつagencyあり・truncated=falseは裁定に従い
inconclusive/runtime_unsupported。inconclusiveかつtruncated=falseは
業務打切りとしてinconclusive/assigned_check_unavailable。
私有Report対は6verdict×3truncated×agency有無を各比較器へ渡す。
正の対照は完全な同じReportをsatisfied/violated/vacuousにした3対。
これは防御の入口の試験であり、全偽造対が公開DSLで到達可能とは主張しない。

境界レビューのmodel Hはfailしか返さない照合を持つ。
Extract 0..1、Clerk 1..2、p=failで元はrejected at2、work2、satisfied。
ifのkeep_thenだけを残すとLedgerは2..4、horizon3でcut_off、Reportは
inconclusive、truncated=false。種2と8のbody/2:keep_thenはどちらも
inconclusive/assigned_check_unavailable、run.completion=inconclusive、
run.reason=mutant_incomplete。horizon5の対照ではdone at4、work4、
同じ2行はundetected/no_assigned_check。外側は4未対応種のためunsupported/exit4。
元と手編集のOutcomeを両backendへ渡し、全Mutation出力は既存の
validate_mutation_wireで照合する。wireを緩めない。

### 候補のsourcesとstartだけのwriter

既存observed(true,true,false)はEarlierのArchive、WriterのInvoiceの順で
snapへ書く。summary_sites自身のe→a候補のsourcesは厳密に
{Archive,Invoice}。単writer observed(true,false,false)は{Invoice}。
observed(false,false,false)は空でもe→a候補を残す。Reportのheldではなく
実際の候補内の集合を直接照合し、last-writer-onlyの故障を区別する。

startだけの入力はWriter.makeのwrites snapを消し、start { snap = entry }を
残す。lower後にsnapがstart_writesにあり、finish_writesにもkernelの
宣言・overrideにも無いことを先にassertする。公開lowerに成功すれば、
元done5/rejected4、手編集done6/rejected5、work同値、e→aのsourcesは
{Invoice}、Mutationのsubject_sourcesもInvoice、group3→4の3held。
もし既存の公開lowerで拒否されるなら、その実際の制約を明記して私有IRへ
分離する。公開の検出力が確認できたと先取りしない。

校正は単独で3件: written_sourcesの累積をsetの上書きへ替える、
written_variablesからstartだけを除く、comparison_failureのR5腕だけを除く。
最後の故障は公開quality_horizon_mutant_is_inconclusive_not_a_static_detection
を実行する。各回srcを復元してgit diffのsrcが空であることを確かめる。
結果と19123f21上のRED/GREENは後段の実行記録へ分けて追記する。

### 追加受入の実行記録 (本番srcは19123f21のまま)

start-onlyの公開DSLはlowerに成功した。snapはstart_writesだけにあり、
通常finish・atomic finish・kernel宣言・overrideには無いという前提を通った。
両backendの元done5/rejected4と手編集done6/rejected5、3→4 held、
候補sourcesの厳密集合を実際に照合して成功した。私有IRへのfallbackは不要だった。

| 対象target | 固有の成功 / 失敗 | 失敗の観測 |
|---|---|---|
| core mutation_integration | 31 / 1 (固有32本。共有wire65本を含むtarget全体は96 / 1) | Hのkind2 keep_thenがundetected、期待inconclusiveで失敗。Horizon5の完了対照と元/手編集の両backendは成功 |
| CLI mutation_integration | 8 / 1 (固有9本。共有wire65本を含む全体は73 / 1) | 実binaryのHorizon3出力をvalidate_mutation_wireがincomplete/invalid report classified as comparisonで拒否。Horizon5は通過 |
| core --lib mutation::default_private | 7 / 7 (14本。追加9本のうち2成功 / 7失敗) | 旧5種のhorizon/Unsupported入口とsummaryのUnsupported入口の6行列、run completionの1本 |

各行列は36対、合計216対を実際の比較器へ渡した。旧5種はtruncated=falseの
Inconclusive/Unsupportedをundetectedへ分類し、summaryはagency付きUnsupportedを
detectedへ分類した。静的失敗、truncated=null/true、正の3verdict対照の期待には
不一致を報告しなかった。私有run試験は実際に全6種の生成を観測し、
UnsupportedのSとInconclusive/UnsupportedのXでcomplete/nullを返すことを記録した。
合計9 REDは入口の根が共通であり、9個の独立の製品欠陥とは数えない。
受入の合格を得るためにsrcを直していない。初回観測後にwireの呼出しを
assertより前へ置き、cut_offとrun.reasonのassertを足し、run試験を全種の
観測まで継続する形へ強めた後も、同じ件数と理由でREDだった。
ログは作業時の実行記録へ分けた。core、CLI、
`c6fix-final-private.log`。共有wireの同じ65本を新しい網羅数へ重複加算しない。

### 単一故障の校正の観測

| 故障 (本番srcへ一時的に1件ずつ) | 対象の実受入 | 故障時 / 復元後 |
|---|---|---|
| written_sourcesの累積extendを各writerのsetで上書き | c6fix_summary_candidate_sources_are_the_exact_all_writer_union | exit101、実候補Invoiceだけと期待Archive+Invoiceで失敗 / exit0、1成功 |
| written_variablesからstart_writesだけを外す | t20_start_writer_is_not_missing_from_summary_source_ancestry | exit101、公開lower/Outcome/手時間を通過後undetected対detectedで失敗 / exit0、1成功 |
| 同じstart故障のまま別の私有入口 | c6fix_start_only_writer_contributes_to_the_actual_summary_candidate | exit101、実候補空集合と期待Invoiceで失敗 / exit0、1成功 |
| comparison_failureからInconclusiveを排除するR5腕だけを外す | 公開quality_horizon_mutant_is_inconclusive_not_a_static_detection | exit101、Rate1/0・cut_off・Report inconclusiveを通過後detected対inconclusiveで失敗 / exit0、1成功 |

3故障とも復元の直後にgit diffのcore/CLI srcが空へ戻ったことを確かめた。
保存した3つの復元diffはすべて0byte (SHA256
`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`)。
この3件の校正を
全故障・全scheduleの保証へ一般化しない。

### 旧固定出力の独立列挙 (新出力diffは不使用)

変更前binaryをsha256sumで上記`c46e7049...20c79c`へ照合してから、
固定manifestのmutate2220jobを各1回実行した。ordered argv・source SHA256・
stdout/stderr SHA256・exitは各jobの旧固定値に厳密一致した。
元manifestは4273job、SHA256
`dbd8c4fb345f052ccff8e3683b4207e5ed875c8dbffc2c30ea6bfc74bfe28d32`。
1540個のReport付き変異行を走査し、そのうちdetected/undetectedの1471行は
すべて正のverdict集合かつtruncated=false。その他はstatic_invalid50、
assigned_check_unavailable17、truncated2。root constraintにより移行を
許すべき旧job/位置はともに0 (`[]`)。manifest JSON、既存12許可位置、
Rust/Python verifierは変更しない。非mutate2053jobは分類行が無いため
この再実行の対象外。新binaryの全jobの互換成功はここから主張しない。
この記述は、すべての交差を検査したという主張ではない。
12入力のbeforeの古い観測表とSHA256は上の本文を保持し、今回再採取していない。
