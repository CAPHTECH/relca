# Independent hand calculation — ADR 0041

Unchecked start effects persist. Both Tasks increment started at0 so started2 even though only one finish commits. V only checks holder and never rolls back start. Removing start changes started2 to0 but not finish/terminal arithmetic.

Stage C: authored oracle, not observed implementation behavior.
