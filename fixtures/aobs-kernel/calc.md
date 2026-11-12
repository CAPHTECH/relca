# F23 aobs-kernel — hand oracle

ADR 0032; plan section 3. Values fixed before implementation, minutes.

K exists in both, no missing-kernel confound. Actor state cannot enter kernel_reads/given/with. Independent kernel result may still write self enum state using ordinary writes.

Single input transformation: 'outcomes kernel K' → 'outcomes kernel K; kernel_reads latent [A.flag]'.

Expected verdict: satisfied; command: check.
RED acceptance is not implementation verification.
