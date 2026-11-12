# ADR 0058 レビュー追従の手期待

期待の出所は ADR 0058 の決定 1・2・5・7 と付録、および Q3 レビュー裁定。
production の JSON を期待へ採取しない。既存 Q3 の Spec/expected は、入力から
選択・Actor 名・上限・状態を固定する独立 oracle として再利用する。

## M1: diff の子台帳の所有権

入力は既存 config-diff-parallel の同一模型。全宣言 Actor は Office 一つで、
from/to の Actor 表は完全一致する。AsIs は Serial、ToBeP は Parallel を選択する。
各子の requested.configuration と effective.configuration はその側の名前、
effective.process は Serial / Parallel。各子の参照は自分を基点とする。
両子と root の ledger は独立 Spec から組み直し、正常 wire の decode/roundtrip を先に要求する。

- 正常対: AsIs/Serial と ToBeP/Parallel がそれぞれ decode できる。
- 故障対 1: ledger object 二つだけを交換する。Actor 表では区別できないが decode error。
- 故障対 2: 両側それぞれの requested.configuration、effective.configuration の
  一欄だけを別の非空文字列へ変える。四対とも decode error。
- effective.process の書き換え二対は裁定により除外した。root の requested は process を
  持たず、正常対でも Serial / Parallel と異なるため、decode で照合する独立の証跡が無い。
  これは既知の限界であり、process の改ざんを検出したとは数えない。
- 台帳以外の Side.configuration や子 Report の証跡は変更しない。

### minor-A: 未 lower の自己申告と子 Report の証跡

同じ独立台帳の正常対から、片側の子台帳の actor_versions、requested.configuration、
effective.configuration の三欄だけを unavailable/not_lowered/entries=[]、null、
unavailable/not_lowered へ変える。Actor 表と execution.status は変更しない。
実行済み ran の両側二対と、lower 後・探索前の not_run の両側二対、計四対を拒否する。
後者の正常対は builtin.status も not_run、state_budget は
unavailable/execution_settings_unavailable とし、故障投入前に decode を要求する。
非 null の Actor 配列は探索前でも lower の証跡であり、空配列も同じ規則で扱う。
execution.status=ran も lower の証跡なので、台帳の not_lowered だけで照合を免れない。

真に未 lower の正常対では両子を下記 M2 の手組み拒否 Report に置き換える。
actors=null、execution.status=not_run、未知の要求/実効構成を維持し、
Side.configuration を要求欄へ補完せずに decode/roundtrip が成功することを要求する。
試験は minor_a_diff_rejects_false_unlowered_child_selection と
minor_a_diff_keeps_genuinely_unlowered_children_with_unknown_selection。

## M2: Actor 表の不在と既知空集合は異なる

手組みの parse/I/O 拒否 host は actors=null、Actor 版は
unavailable/not_lowered/entries=[]。正常 wire は decode できる。
actor_versions だけを known/reason=null にしたとき、空 entries も、
Invented 一 Actor を足した entries も decode error。
null を既知の空 Actor 集合や、捏造した版の根拠にしてはならない。

## minor-1: 前段拒否の state_budget

通常模型 serial-time は Office.short 3 分 + long 5 分で elapsed/work=8、
process=Check、bounds=1/2/8、探索一回、実渡し state_budget=1000000。
その対は lower の成功 IR に Error の U-SEM-0 を一つ入れ、verify を空にする。
既存 analyze_model の防御的拒否を使い、探索 closure は 0 回、性質/終端は空、
verdict=unsupported、execution/builtin.status=not_run。
lower 済みの Actor・process・bounds は保持するが、state_budget は
unavailable/value=null/reason=execution_settings_unavailable。
これは不明な数値の代用でなく、探索へ値を渡していないことを示す。

公開 DSL で verify だけを消すと lower 自体が Err になる。この経路では
actor_versions も unavailable となり、問題の finite_driver の known 書込みまで届かない。
そのため test-only の代替 lowerer を使う。判定を緩めたのではなく、
Error を持つ Ok IR を既存の私有注入境界へ渡す防御分岐の受入である。

## minor-2: 最小拒否の診断と最終 exit

verify の通常拒否は unsupported/4、共通拒否の引数なしは invalid/3。
producer.version 一欄を空にして発行 factory へ渡すと、両方に
U-LEDGER-0/error/span=null、message の先頭 version_unavailable が一回残る。
独立した invalid/unsupported と各 exit は維持する。
正常な factory/CLI には診断欄や stderr の変更を要求しない。

この二つは元から非 0 で seal 前後の exit が等しいため、それだけでは古い verdict の
使用を検出できない。別の明示的な人工対で factory 入力の predecessor verdict を
satisfied に固定する。同じ入力で正常 ledger なら satisfied/0、不正 ledger なら
unsupported/4。stdout の最終 verdict だけが変わって exit が 0 のままなら拒否する。
これは verify の通常動作を成功に変える要望ではなく、exit 式への故障校正である。

### 実行 seam

`support/ledger_refusal_probe.py` は現作業木の production src と manifests を
作業木内の一意な一時 workspace へコピーし、offline/locked で実 binary を作る。
原本は読取専用。コピーへの追加は、上記二つの入力注入と minor-1 の私有境界 adapter のみ。
seal/complete/validate、診断の格納・表示、Serialize、exit 式は変更しない。
正常 env のコピーと無変更 CLI の stdout/stderr/exit の完全一致を要求する。
代替 lowerer の正常対は探索一回と手値 8 を要求する。
原本全 Rust source の SHA-256 はコピー前後で一致させ、observations.json に保存する。
注入の anchor が変われば harness failure として止め、意味の RED と混同しない。
Python は -B、cargo は CARGO_NET_OFFLINE=true/CARGO_PROFILE_DEV_DEBUG=0、
TMPDIR と build 出力は作業木内。削除・ネットワーク・元 src の編集はしない。

## minor-3: 再 seal の冪等な診断

check と simulate と conform のそれぞれの正常結果で producer.version を空にし、
seal を一回呼ぶ。unsupported と U-LEDGER-0 一回を要求する。
同じ不正のまま再 seal しても同じコードは一回だけ。
正常 check の再 seal は JSON 全体不変。
独立した invalid/violated を持つ check の一回 seal では元 verdict を保持する。
simulate は sim-keys の seed=0/要求4/信頼係数3/4、conform は既存 Q3 正常6イベント。
これらの判定を snapshot 更新や新出力の採取で置き換えない。

## 適用範囲

ここでは decoder と発行境界を検査し、engine/reference の意味や全入力の同値性は主張しない。
一回限りの差分監査の検査範囲と鍵行列の分担は ADR 0058 に記録する。
手値の表や入力の存在だけで実装の検証が完了したとは数えない。
