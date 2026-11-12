# F19 single fault

Only A.use(a) → A.use(case.other). Unsupported/4, not invalid. No runtime results.

With CaseId support, case.id is accepted
as an argument but is E-TYPE-0 when passed to this enum parameter. Preserve the
old unsupported-member purpose with case.other instead. PRE-A3 returns
E-SYN-0/3 (expected closing parenthesis at the dot), while the approved A3
member rule repairs this to U-SYN-0/4, unsupported syntax: case.other argument,
with the whole case.other reference as its span. The exact classification
transition is a closed migration, not a general syntax-error exemption.
Original source bytes/SHA are archived in caseset_compatibility.json.
The new enum-argument mismatch has its own aset-static-enum-argument pair.
