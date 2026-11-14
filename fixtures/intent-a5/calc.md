# A5: 3案と相手別の排他選択

A=2+1=3分、B=1+1=2分、C=3+1=4分。逐次なので経過と作業量は等しい。
登録したjobは C→B。C側のRobot.checkからBot.checkへ差し替える10行を覆う（A1のClerkをRobotに読み替える）。
Permissionは{boss}から{op}、Docの読み手は{Robot}から{Bot}、vの生産者はRobot.checkからBot.check。
intent_blockはC/B。reason=migration、loses=supportであり、既定のvisibility理由defaultは混ぜない。
W-DIFF-1/2/3=0/0/0、satisfied / exit 0。A→Bも10行、A→Cと逆向きには宣言を流用しない（固定受入でも検査）。
故障対はC→Bのreplace元だけをClerk.checkに変える。
