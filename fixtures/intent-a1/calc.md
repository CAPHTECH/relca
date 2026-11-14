# A1: 能力1つの差し替え

手計算を期待の正本とする。実行結果を転記していない。
A は check 2分 + post 1分 = 3分、B は1+1=2分。逐次なので作業量も3/2分。
活性集合は {Clerk.check,Desk.post} と {Bot.check,Desk.post}。
Actor の追加/削除2、能力の追加/削除2、Permission の追加/削除2、実行者資源の追加/削除2、Doc の読み手1、v の生産者1、合計10行。
Permission は {clerk}/{op}、Doc の読み手は {Clerk}/{Bot}、生産者は Clerk.check/Bot.check。
差し替え後の集合と値が B に一致し、全10行を覆う。能力2行が direct、他8行が derived。
W-DIFF-1/2/3 は0/0/0、satisfied / exit 0。各行の reason/loses はnull、宣言にだけauto/eyeを置く。
壊した対は loses だけを削除し、覆いを保ったまま W-DIFF-2=1、W-DIFF-3=8 になる。
全行の覆い・警告数・時間は intent_revision と独立した snapshot hand oracle でも照合する。
