# F1 fault: latent difficulty in business policy

Single edit: difficult_seen == true becomes Invoice.difficulty == difficult.
ADR 0012 prohibits every process-if latent read statically, including a fixed
default and a false comparison. Thus E-POL-LATENT/error, invalid, exit3.
No execution paths, times, variable snapshots or clause results are reported:
properties=[], terminals=[], agency=null and all four bounds fields=null.
The declarations would specify false/routine, but must not be used to bypass
the prohibition. W-CUT-0 does not change invalid precedence.
