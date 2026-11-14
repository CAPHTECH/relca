# 最後の拒否を外す

正対からj10のelse終端だけを外した。doneはj1〜j9がpassならj10のどちらの値でも届く。
doneのj1〜j9はall_contexts、j10はread_only。rejectedは最終の拒否j9だけsome_contexts、j1〜j8はread_only。
j10はrejectedの経路で読まれない。Lは正対と同じ[1,1]/[1,10]/[0,1]。
doneは12分、rejectedは1+m(m=1..9)で2〜10分。全体satisfiedは拒否が保たれた証拠ではない。
