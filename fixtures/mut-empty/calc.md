# M-N: 候補なし

ADR 0020 本文・追記、ADR 0002・0004、設計 v0.2 §22.2。
end done のみ。reads も if も無いため四種は not_applicable、変異体0。
元の safety は satisfied。六種は unsupported のままなので全体4、run complete。
対では未知Actorのcall一つだけを追加する。全行と全siteの手期待は
core/tests/mutation.rs の hand_rows に固定し、CLIでも同じ独立oracleを使う。
rootが同じexitでも弱点の見落としと数えない。検出率は計算しない。
