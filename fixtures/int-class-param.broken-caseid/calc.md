# F18 single fault

Only parameter type changes to PersonId. No call, so no argument-count failure
may mask the unsupported parameter classification. Hand expectation U-SYN-0/4.

With CaseId support, the original CaseId
parameter becomes accepted, and declaring CaseId is no longer a legal alias.
The original source bytes and SHA are archived in caseset_compatibility.json.
PRE-A3 binary on this replacement emits E-NAME-0/3, undefined parameter type.
That is the explicitly requested PersonId classification repair, not an oracle
copied from the old executable. New expectation remains unsupported/4.
