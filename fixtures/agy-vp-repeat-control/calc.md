# 反復の最終的な未導出理由

1回の判定はinspect+second=2分、両方failならcorrectを足して4分。
doneは1〜3回目のleaveで2、6、10分。3回すべてfailなら4*3+1=13分でescalated。
従ってdoneは到達可能でsatisfied。反復回数の上限は業務の上限で、clock40分は切断に当たらない。
F/Lの最終理由はloop_control。途中の静的経路列挙がpaths上限に当たっても、最終JSONに無い理由のW-AGY-5は出さない。
broken対ではleaveをagainへ変える。doneへの経路が無くなり、到達性はviolated。
