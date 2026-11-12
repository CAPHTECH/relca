# Agency の導出と計算の上限: 手計算

`support/agency_limit_inputs.rs` が入力を組み立てる。
`agency_derivation_limit.rs` と CLI の同名試験が、構造と実行結果を分けて照合する。
以下の意味期待値は入力から手で決めたものであり、実行した binary の出力から採取していない。
末尾の全出力保存だけが例外である。

## 共通の意味

値型 V は pass / fail、Ack は done。Doc は document の根拠源。
Intake.start は Doc を読み、引数なしで pass を返す生産者 s。
Judge.check は V の引数を一つ取り、pass / fail の両方を返しうる。
Ledger.post は commits を持ち、done を返す効果 p。
すべて所要時間 1 分、容量 1、案件 1、live_tasks 2。
Cut は全 Actor を明示的に含める。判定の潜在値参照はない。

Judge の結果の確率は通常指定しない。check は両方を探索する。
simulate の入力に限り、宣言した Coin kernel の各行を
pass=1/2、fail=1/2 とする。共有の潜在 coin は false に固定され、
どちらの kernel 行も同じ分布である。未指定の非決定を一様乱数で埋めない。
simulation は 4 回、seed 1、confidence 0.95。標本の件数や比率を
網羅探索の性質に読み替えず、実行完了と Agency の未導出だけを照合する。
標本で反例がない safety は basis=sample の inconclusive であり、root も inconclusive / exit2 とする(受入訂正の裁定)。

## 入力と手値

| 名前 | 入力を作る関数 | 意味期待値 |
|---|---|---|
| agy-limit-control | gate(13) | j1..j13 の13判定がすべて all_paths、bypasses=0 |
| agy-limit-work | work() = joined(11, Or) | 性質2件とも satisfied、root satisfied、exit 0。Agency は not_derived / derivation_limit、W-AGY-4 は1件 |
| agy-limit-paths | requires(13) | 性質2件とも satisfied、root satisfied、exit 0。paths 上限4096、最初の超過請求4097、場所は g13 |
| agy-limit-paths-control | requires(12) | 上限ちょうど4096。未導出扱いにならない |
| agy-limit-violation | violation(true) | safety / reachable は satisfied、contract Once #1 at_most_once は violated、root violated、exit 1 |
| agy-limit-violation.broken-no-contract | violation(false) | contract だけを除く。性質2件とも satisfied、root satisfied、exit 0 |
| agy-limit-c1 | gate(14), gate(40), gate(200), joined(20, And) | 導出成功、すべての j は all_paths、bypasses=0 |
| agy-limit-any | joined(3, Or), joined(10, Or) | conditional、bypasses は3と511 |
| agy-limit-cont | continuing(3) | none、bypasses=4 |
| agy-limit-chain | chain(986) | j0..j986 の987判定。none、bypasses=1、対象の根拠源は Doc のみ、独立な根拠源は空 |
| agy-limit-configs | configs() | Small は2個の gate、Big は work()。Small の Agency は導出済み、Big だけ未導出。両側の性質は satisfied |
| agy-limit-degradation | degradation() | reachable done の証拠はあるが clock 切断もある。性質は satisfied、root inconclusive、exit 2 |
| agy-limit-contest | contest() | Standing と2分の異議応答を持つ。性質4件とも satisfied、root satisfied、exit 0 |

### 直列 gate と And

gate(k) は各 ji の fail 側を end rejected で終える。
p へ至る経路では全 ji=pass だけが許される。判定 ji 以外の
k−1 個の値をすべて pass に固定した文脈は一つだけ。
ji=fail に変えると p に至れず、ji=pass なら至れる。
従って各 ji は all_paths、bypasses=0。Guard が読む ji はすべて
引数 s を持つので、判定候補数と lineage 行数はいずれも k。
真の枝の And も同じ連言である。

異なる束縛を一つずつ読む連言は成分ごとに閉じる。
14、40、200 および And20 は導出の成功を要求するが、巨大な
全状態探索の完了までは要求しない。構造の照合には parser と lower、
public agency::derive を使う。

### 一つの Or

Or は k 個の束縛を同じ条件で結ぶため、一つの成分として扱う。
判定 ji 以外の k−1 個がすべて fail の文脈は一つだけで、そこでだけ
ji を fail にすると止まる。残る 2^(k−1)−1 個の文脈では
別の判定が pass なので、ji のどちらの値でも p へ進める。
従って k=3 は conditional / 3、k=10 は conditional / 511。
Or を連言のように分けると all_paths / 0 になり、この手値が検出する。

work の k=11 は、一つの Or の成分を全列挙する計算の上限の対照である。
上限を越える k の選択には既存の再現報告と、現行で Agency の段まで届いて
止まることの実測を使う。satisfied という期待はその実測から作らない。
少なくとも一つ pass の組があるので done は到達可能。
同時に動く task は一つで、容量1を超えない。
最長経過は s1 + 判定11 + p1 = 13分 < clock92分。
ゆえに性質2件とも satisfied、bounds は {cases:1, live_tasks:2, clock:92, truncated:false}。
Agency の未導出は、この性質の結果を変えない。W-AGY-4 は warning 一つであり、
U-SEM-0 error を併記して root を unsupported に戻してはならない。
work には via model 宣言がないため via_model_paths は従来どおり null とし、主の導出の失敗で独立した出力を変えない(受入訂正の裁定)。

### 両枝が続く if

continuing(3) の各 if はどちらの結果でも後続へ至る。
判定自身以外の二つの値は全4組とも実行可能。
どの文脈でも止められないので none / 4。
この小さい入力の意味の保存だけを要求し、大きな入力の最適化は要求しない。

### requires の経路の上限

Gate の requires は permit==pass、初期 permit は pass で書き換えない。
実行時は全 gate が受理され、s1 + g13 + p1 = 15分 < clock96分。
done は到達し、直列なので資源も超過しない。
構造の経路展開は受理と拒否を両方残す。
初期経路1と、各 gate が作る追加経路の数を足すと
1 + (1 + 2 + ... + 2^(k−1)) = 2^k。
k=12 では4096であり、上限を超えない。
k=13 の必要量は8192だが、単位1で請求するため最初の失敗 requested は4097。
その時点は g13 の展開中。8192 を requested にしてはいけない。
診断はこの呼出しの span を指す。上限を4096より大きく変えて通す修正はしない。

### 違反と契約を外した対

violation は Or の通過後に p と p2 の二つの Ledger.post を直列に呼ぶ。
全 pass の値の組は存在し、同じ case.id について commit が2回起こる。
at_most_once の許す1回を超えるので契約は violated。
done も到達し、所要時間は14分 < clock92分、task も一つずつなので
他の2性質は satisfied、切断なし。契約だけを除いた対は satisfied になる。
Agency が未導出でも違反を隠さず、explain はこの2回の commit の反例を再生する。
開始と11判定の計12分に各効果1分を加え、case1のpのcommitは13分、p2は14分。traceはactor=Ledger / capability=postの別欄なので、この2段のcommitイベントを構造で照合する(受入訂正の裁定)。

### chain の行数

chain(k) は s = Intake.start()、j0 = Judge.check(s)、続いて
各 i=1..k の ji = Judge.check(j(i−1))、最後に p = Ledger.post(jk)。
従って k=986 の判定呼出しは 1+986=987個であり、986個ではない。
s は引数を持たない生産者なので判定から除く。
p は効果であって自身の祖先ではないため、この p の判定としては数えない。
全 j が p の引数の祖先にあり、それぞれ対象を持つため全987件を lineage に残す。
根拠源集合は s が {Doc}、各 ji も帰納的に {Doc}。
Judge は reads を持たないため independent_sources=[]。
同じ対象の group は一つ、judgements=987、actors=1。
条件のない経路で guard support は空。全 ji の文脈は空の組一つで、
単値 pass は p へ進むため、すべて none / bypasses=1。
この987件は生成手順を手で数えた訂正であり、binary の観測値ではない。

### 縮退の切断を成功に変えない

degradation は Or11 の後、p を実行し、j11==fail のとき Slow.run(3分)、
それ以外では Fast.run(1分)を呼び、両方 end done。
s + 判定11 + p = 13分。Fast の done は14分で clock15分以内。
j11=fail でも他の判定が pass なら Or を通り、Slow の done は16分で範囲外。
従って done の到達証拠と truncated=true が同時に成立する。
done 14分の証拠は既存の per_case 出力契約に従い case_outcomes の terminals で照合し、root の terminals へ重複させない(受入訂正の裁定)。
reachable の satisfied を root の satisfied と取り違えず、縮退の whole-run の
切断規則を掛けて inconclusive にする。単に性質も inconclusive になる入力では、
この規則の欠落を検出できないため、その期待には変えない。

### Standing と contest

Judge の実行者は j、Route.respond の実行者は r。許可をそれぞれ宣言し、
Grant は j に Judge.check の権限を与える。Party は control party とし、
end.rejected への Standing を持つ。
Or11 の全 fail の組では rejected が13分より前に生じ、
そこから Route.respond が2分で完了する。respond_by=2min の境界と一致し、
clock92分以内であり、reversible_by j は元の判断の権限に含まれる。
contest 宣言は静的な contest と実行時の contest_response の二つの性質を持つ。
よってこの二つはともに satisfied。done は別の値の組で到達可能、資源超過もない。
合計は contest 2件 + reachable 1件 + safety 1件 = 4件。
このモデルでの上限超過は、静的な責務・異議の照合が未導出同士を誤って
不一致としないことも検査する。

## 上限ごとの小さい注入と不正な IR

`support/agency_limit_private.rs` は agency の子モジュールとして使う。
gate(2) の model と他の既定上限を固定し、depth / paths / contexts / cells / work の
一つだけを変えて、成功する最小値 k を二分探索する。
k では既定の全結果と一致、k−1 では U-SEM-0 / error を要求する。
失敗の文言は counter と k−1、requested >= k、非空の段、process Flow、
非空のソース span を持つ。contexts の段は refusal contexts、効果は p / Ledger.post、
span は効果の呼出しと一致する。各 counter を work と取り違える故障は4件で落ちる。
請求点の実装変更に耐えるよう、偶然の work 件数そのものは固定しない。

public agency::derive の入力 IR も壊す。範囲外の BindingId / CapabilityId、
自己循環、束縛の重複をそれぞれ独立に作る。すべて U-SEM-0 / error のままで、
not_derived として通過させない。重複の文言は invalid static IR for Agency: で始まり、
duplicate という理由を含み、limit exceeded と混同しない。

## 故障対照

| 対象 | 故障 | 検出する期待 |
|---|---|---|
| counter の名称 | すべて work と表示 | paths / contexts / cells / depth の注入 |
| 縮退の切断 | Agency が未導出のときゲートを外す | 性質 satisfied、root inconclusive |
| 未導出の表現 | 空の導出済み Agency を入れる | status / reason / W-AGY-4 / unchecked、および mutation の unsupported |
| 不正な IR | 重複・循環・範囲外も継続 | derive が U-SEM-0 / error を返す |
| contest の照合 | 未導出同士を不一致とする | contest を含む4性質の satisfied |
| Or の過剰分解 | Or を And と同様に分解 | conditional / 3 と conditional / 511 |
| 契約を除いた入力 | contract Once のみを除去 | violated / exit1 と satisfied / exit0 の区別 |

構造の小さい故障対照では、gate の else を継続に変える、Or を And に変える、
継続の else を終了に変える、の単一入力差を同じ oracle に渡し、拒否後に元の入力を
再度受理する。実際の実装への一時故障と、その復元・実行の証拠は試験結果と区別する。

## 他のコマンド

explain は違反の反例を再生し、simulate は宣言済みの確率 kernel を実行する。
diff の Small / Big の両向き、compare の Small,Big では性質の結果を出し続ける一方、
Agency の差は not_derived とし、未導出を空の既知値と比較しない。
mutate の evidence_missing / summary_reused / refusal_path_cut は、元の Agency が
未導出なら、そこに割り当てた agency.lineage / agency.refusal の検査について
unsupported / agency_not_derived とする。捕捉・見逃し・assigned_changes に数えない。
そのため root の mutation run は exit4 であり、元の check の satisfied / exit0 と区別する。

## 全出力の保存という別の期待

CLI の `agency_limit_preservation.json` は、小さい gate2 / gate3 / And3 / Or3 / Or10 /
continuing3 / chain3 および configs の Small について、**現行の stdout・stderr・exit を
そのまま保存した差分照合用の期待**である。意味の手計算から作ったものではない。
分解と祖先集合の表現変更が、これらの入力の全出力を変えないという契約を照合するため、
ここだけ現行 binary の採取を認める。JSON の部分抽出や並べ替え、値の除外をせず、
stdout / stderr の文字列と exit を完全一致で比較する。手値の oracle の代わりにはしない。
