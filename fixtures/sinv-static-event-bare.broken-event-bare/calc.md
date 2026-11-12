# sinv-static-event-bare.broken-event-bare — independent hand oracle

ADR 0037; design v0.2 §11.1; design v0.2 §12.4; design v0.2 §22.3. All times/work are minutes.

Control uses only declared state and has Fix1, end/work1. Single replacement is rejected statically with exactly U-SYN-0; no runtime terminal, no probability, bounds.truncated=null. Both expression sides and nested groups are tested separately.

Single edit: '-> ok == true' → '-> ok'
Fault/control oracle: {"verdict": "unsupported", "diagnostics": [{"code": "U-SYN-0"}, {"code": "W-CUT-0"}], "bounds": {"truncated": null}, "properties": [], "terminals": []}

## Existing parser-error Cut behavior (observed once before C3)

Parsing fails before the Cut AST is available, so the existing report adds W-CUT-0 even though the source contains a Cut. Lowering errors retain only their error code. This is an old behavior observation, not a capture of the new invariant expectation.

Pre-C3 binary SHA256: ad80ad35702f9ec2e6b464dad8eb32fe59da7c7c0c1bba050fc046f3f94ed045. Exit: 3. Input:

```relca
model ParseCutProbe version "0.2" {
  type R = enum { ok }
  observed case { ok: bool = true }
  actor Fix { capacity 1; capability fix() -> R { duration fixed 1min; result nondet { ok } } }
  process Flow { r = Fix.fix(); end done }
  cut C { include [Fix] }
  bounds { cases 1; live_tasks 1; clock 4min }
  contract Controls { invariant Fix.fix.start -> ok == }
}
```

stdout (verbatim):

```json
{"actors":null,"cut_context":{"notices":["Cut 外の存在(未知の被影響者、一度だけの拒否、登録されていない当事者)が作用しないとは主張しない","この結果を運用の範囲決定に使う場合は採用経路を別に追うこと"],"reentry":[],"horizon":[]},"agency":null,"verdict":"invalid","mode":"exhaustive","unit":"min","model":{"name":null,"sha256":"359ca85c1d9b017ed5d1b715ec478791e520c8e1bfdfca70aadf0dbfa8b58b91"},"cut":"undeclared","assumptions":[],"diagnostics":[{"code":"E-SYN-0","severity":"error","message":"expected identifier","span":{"start":362,"end":363}},{"code":"W-CUT-0","severity":"warning","message":"Cut is undeclared; exclusions are unknown","span":null}],"bounds":{"cases":null,"live_tasks":null,"clock":null,"truncated":null},"properties":[],"terminals":[]}
```

stderr (verbatim):

```text
E-SYN-0: expected identifier
W-CUT-0: Cut is undeclared; exclusions are unknown
```


Expected data was frozen before implementation. A failing pre-feature run is not semantic verification.
