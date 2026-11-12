# M-G: 契約と拒否

ADR 0010・0009、ADR 0020追記。R1×2=2体、終端ガードG2×2=4体、計6。
元requires_eventとinvariantはsatisfied。po failは時刻2でreturned、
po pass/approval failは3でreturned、両passのみ4でdone。
G1は if po == pass { } else { end returned } 全体を空文列へ置換。
fail後にもapprove.startがあり契約#1 satisfied→violated、Hold detected。
G2も同様にif全体を空文列へ。approval failでもcommit、#2 satisfied→violated。
postへ両判定値を引数で残しており、対応Uはall_paths→none、拒否種も各detected。
ReaderのR体は独立源減少無し、責務無しで両種undetected。
割当以外のU/警告/時間変化も別配列へ残す。六種unsupportedで全体4、run complete。
