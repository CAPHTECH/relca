# 反復から抜ける経路の欠落

leaveをagainへ1語変更した。すべてのbodyがagainなので3回後は必ずon_limitへ進み、doneは到達不能でviolated。
各回の2判定は必須で6分、correctは0〜3回(0〜6分)、escalateは1分。escalatedは7〜13分。
F/Lはloop_controlのままで、実行の違反と静的観点の未導出を混同しない。W-AGY-5は無し。
