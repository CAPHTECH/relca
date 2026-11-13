# mut2-expiry.broken-zero-duration — ADR 0046

Round 2: total/attempts=0 retains unsupported/exit4 (reason null). This is the zero-time not_applicable example, not an undetected-expiry counterexample; see mut3-expiry-window.broken-late-arrival for that separate role.

所要時間と到着時刻がともに0なので、終了時刻も0。期限0は時刻0を含み、
0 ≤ 0 で授権は有効なまま。時間経過による失効を観測できない。
ADR 0068 では authority_expired を not_applicable / no_time_elapse とし、候補数は0。
この名前は元の「検出漏れ」の対の来歴で、現在の役割は0分なら適用できない例である。
reads・process guard・2消費者を持つproducer・commit・cohortはいずれも無いので、
他の対応済み種も0候補。規則版と未宣言の品質分母は列挙不可(null)で加算しない。
従って全種の candidates_total も0となる。
授権自体の期限の意味は変えない。1分の正例では 1 > 0 となり引き続き検出する。

Expected values were derived before running the implementation. Time/work unit: minute.
Mutation is not a coverage or real-world quality score. Rule-version mutation and the undeclared quality denominator remain unsupported here.
The exact new-row oracle is mutation_default_oracle.json in core tests/support.
The zero-time Authority validity is also checked directly in mutation_expiry_time.rs.
