# A2: 無関係な監査を覆わない

A は note 1 + check 2 + post 1 = 4分。B は note 1 + check 1 + post 1 + log 1 = 4分。作業量も4分ずつ。
Clerk.note は両側に残るので Actor/資源 Clerk の行は無い。
置換に伴う行は actor Bot、relation Clerk.check/Bot.check、authority Clerk.check/Bot.check、resource Bot、visibility Doc、producer v の8行。
Audit の actor/relation/authority/resource の4行は置換では消えない。合計12行中8行を覆う（direct2/derived6）。
Doc はClerkからBot、vはClerk.checkからBot.check。監査はDocを読まない。
W-DIFF-1/2/3=4/0/4、satisfied / exit 0。壊した対では Audit.log に reads [Doc] を1つ追加する。
