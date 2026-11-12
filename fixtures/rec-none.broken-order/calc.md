# rec-none.broken-order

T12: both sides begin1min, extract1min, approve2min. Correct approve finishes t4 with entry high/low. Swapping only extract/approve makes approve finish t3 with entry none, so both field equality leaves are false. Later extraction at4 does not erase past violation. Total done4/work4 on both.

Expected values are hand derivations, not captured implementation output. Units: minutes. New acceptance stops RED before implementation.
