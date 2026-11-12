# Selector diagnostic span: PRE-B5 control

The selector-span decision preserves the existing executable behavior,
not the planning text's `None`: every scenario selection error covers the full
parsed model, just as existing configuration selection errors do.

Control input: `crates/relca-cli/tests/support/scenario_legacy.relca` (426 source bytes,
model LegacyProbe, explicit Cut, two configurations, a simulation clause, no
scenario). With the archived PRE-B5 executable (recorded separately):

```sh
<baseline> check crates/relca-cli/tests/support/scenario_legacy.relca
<baseline> simulate crates/relca-cli/tests/support/scenario_legacy.relca
```

Both old outputs have E-CONF-0, `a configuration selector is required`,
span `{start:0,end:425}`, invalid/3, and no runtime bounds or properties.
The new no-scenario-declaration flag probes inherit this existing full-model
span rule; only the selector-specific diagnostic meaning changes. The committed
manifest lists exact old/new locations and bytes independently of new output.
425 is the end of this control's parsed model declaration, excluding its final
newline; the file has 426 bytes. It is not a universal constant for other models.

The same old Lower's configuration diagnostics supply the canonical Report
metadata/Cut failure shape. No source-derived span, model hash or input length
is taken from a post-B5 implementation run.
