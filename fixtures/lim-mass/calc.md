# lim-mass

ADR 0039; design v0.2 §22.3. Hand-derived before execution.

Eight equiprobable duration triples: 111->finishes123;113->125;131->145;133->147;311->345;313->347;331->367;333->369. Case3 admission=(2,2,2,2,3,3,3,3). Only333 exceeds clock8. Retain unknown1/8: probability[7/8,1], not1. Case3 C3..9, elapsed1..7, external_wait0..1.

P2 delay, P9 label and K mutations are not counted as single-input defect pairs.
