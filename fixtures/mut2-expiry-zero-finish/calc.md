# mut2-expiry-zero-finish — ADR 0046

Round 2: the0 expiry candidates and absence of other supported sites imply total/attempts=0. Unsupported retains precedence (exit4, reason null), not success; the original check stays satisfied.

明示の期限30を0へ変えても、所要時間・到着時刻・終了時刻はすべて0。
期限0は時刻0を含むため失効は観測できない。ADR 0068 では変異を生成せず、
not_applicable / no_time_elapse、候補数0とする。最初から明示の期限0なら編集が無く、
従来どおり no_matching_site。どちらの元模型も時刻0では satisfied。

Expected values were derived before running the implementation. Time/work unit: minute.
Mutation is not a coverage or real-world quality score. Rule-version mutation and the undeclared quality denominator remain unsupported here.
The exact new-row oracle is mutation_default_oracle.json in core tests/support.
The zero-time Authority validity is also checked directly in mutation_expiry_time.rs.
