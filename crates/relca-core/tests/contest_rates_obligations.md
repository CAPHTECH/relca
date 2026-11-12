# Contest rates の内部検査

公開受入はcontest_rates.rs、CLI同名target、support/q9_inputs.rsとq9_oracle.rs。
手値はcontest_rates_calculations.md。これらはproductionソースを編集しない受入である。

## 公開APIだけで閉じない境界

1. **確率actionと正支持**: 起票を二つの非決定actionに分ける故障を入れ、Cとp1のsimulationが拒否して落ちること。
   pを落として単なる非決定へ戻す故障、uses_probabilityを接続しない故障も別に落とす。
2. **両入口の不正IR**: 新rates IRのp=0/範囲外、空basis、route/target/site不整合を一欄ずつ改竄し、engineと参照が実行前に拒否すること。
   本受入は新IRの型名/欄が未存在なので、コンパイルを壊す仮の型参照を置いていない。未実行のまま成功と数えない。
3. **集約器直接投入**: 実際の集約器へ16反復の固定履歴(不起票12、短3、長1)を与える。target16/filed4、時間mean5/2、sample4。
   子Sampleをnot_configuredへ撤回する故障、未解決反復だけを捨てる故障を落とす。oracleの手組みtableの受理は本体接続の代用でない。
4. **合成site**: 先頭に普通のcontract clauseがあってordinalとcontest添字が異なる入力を使う。
   engineのtask_site/note_taskと参照のorigin/sample_durationsをそれぞれ普通のbinding添字へ戻すと、反復別の試験が落ちること。
5. **End後の閉鎖**: 業務Endだけでrunを止める故障、cancel_ack/requires拒否を通常応答へ変換する故障、期限の等号を遅延へ変換する故障を別々に落とす。
6. **待ちと費用**: A9/A10でserviceとqueueを混ぜる故障、route費用を終端costへ加える故障、同じ人を案件ごとに複製する故障を落とす。
7. **Cut**: 根拠があるとopaque保留を解除する故障、保持したfiled/targetからrateを復活させる故障、親理由だけ残して子Sampleへ数値を漏らす故障を落とす。
8. **旧モデル**: rates無しの既存起票をp=1/2で埋める故障を落とす。未到達clauseの存在だけでsimulateを事前拒否する故障も別に落とす。
9. **未対応交差**: 効果rates、Q1 response/retry、condition wait、Q7 duration given/per_run相関は理由つき拒否を個別に固定する。
   Q7/Q5統合前の未知構文による拒否を、Q9の意図した意味拒否の完了証拠にしない。
10. **Q5/Q7統合**: 共有実行関数はQ5後、report/ledger/uncertaintyはQ7後。A12の既存同時finish非決定をpriorityで勝手に解かない。
    Q7の三区分全体をここで作り直さず、根拠申告と局所版のリンクを接続する。

受入で不足が分かったら、期待を実装都合で変えるのでなく根拠・裁定を確認し、別担当へ返す。
新しい試験や故障校正の結果は、本体のcommitと実行条件を添えて記録する。
