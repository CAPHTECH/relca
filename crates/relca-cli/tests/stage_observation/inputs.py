"""Handwritten existing-DSL inputs for ADR 0074; no new syntax or adapter."""

S1 = '''model S1 version "0.2" {
  type V = enum { pass, fail }
  type Ack = enum { ok }
  people [c1, c2]
  actor Invoice { source document }
  actor Clerk { executors [c1]
    capability enter() -> V { reads [Invoice]; duration fixed 1min; result nondet { pass } }
    capability confirm(e:V) -> V { duration fixed 1min; result nondet { pass, fail } }
  }
  actor Ledger { capacity 1
    capability post(v:V) -> Ack { commits; duration fixed 1min; result nondet { ok } }
  }
  cut S { include [Invoice,Clerk,Ledger] }
  process Main {
    e = Clerk.enter()
    v = Clerk.confirm(e)
    if v == pass {} else { end returned }
    x = Ledger.post(v)
    end posted
  }
  bounds { cases 1; live_tasks 1; clock 10min }
  responsible c1 for Clerk.confirm
  verify reachable end.posted
}
'''


def edit(s, old, new):
    assert s.count(old) == 1, (old, s.count(old))
    return s.replace(old, new)


def add(s, text):
    i = s.rfind('}')
    return s[:i] + text + '\n' + s[i:]


S2 = edit(S1, '    capability confirm', '  }\n  actor Checker { executors [c1]\n    capability confirm')
S2 = S2.replace('Clerk.confirm', 'Checker.confirm').replace('include [Invoice,Clerk,Ledger]', 'include [Invoice,Clerk,Checker,Ledger]')
S3 = edit(S2, 'actor Clerk { executors [c1]', 'actor Clerk { executors [c2]')
S4 = add(S2, '  observed case { entry: V = fail }')
S4 = edit(S4, 'reads [Invoice]; duration', 'reads [Invoice]; writes entry; finish { entry = pass }; duration')
S4 = edit(S4, 'capability confirm(e:V) -> V {', 'capability confirm() -> V { reads [entry];')
S4 = edit(S4, 'v = Checker.confirm(e)', 'v = Checker.confirm()')
S5 = edit(S2, '    v = Checker.confirm(e)\n    if v == pass {} else { end returned }', '''    repeat rework at_most 2 {
      v = Checker.confirm(e)
      if v == pass { leave } else { again }
    } on_limit { end returned }''')
S5 = edit(S5, 'x = Ledger.post(v)', 'x = Ledger.post(e)')


def reread(s):
    if 'capability confirm()' in s:
        return edit(s, 'reads [entry]', 'reads [entry, Invoice]')
    return edit(s, 'capability confirm(e:V) -> V {', 'capability confirm(e:V) -> V { reads [Invoice];')


MODELS = {
    's1-artifact': S1, 's1-reread': reread(S1),
    's2-artifact': S2, 's2-reread': reread(S2),
    's3-other': S3, 's3-reread': reread(S3),
    's4-variable': S4, 's4-reread': reread(S4),
    's5-repeat': S5, 's5-reread': reread(S5),
}

MERGE = '''model Merge version "0.2" {
  type V = enum { pass, fail }
  people [c1]
  actor Invoice { source document }
  actor Clerk { executors [c1]
    capability enter() -> V { reads [Invoice]; duration fixed 1min; result nondet { pass } }
    capability fix(e:V) -> V { reads [Invoice]; duration fixed 1min; result nondet { pass } }
    capability confirm(e:V) -> V { duration fixed 1min; result nondet { pass, fail } }
  }
  cut C { include [Invoice,Clerk] }
  process Main {
    e = Clerk.enter()
    ready = if e == pass { f = Clerk.fix(e); yield f } else { yield e }
    v = Clerk.confirm(ready)
    if v == pass { end posted } else { end returned }
  }
  bounds { cases 1; live_tasks 1; clock 10min }
  responsible c1 for Clerk.confirm
  verify reachable end.posted
}
'''

# The declaration order Z,A and z,a is intentionally not alphabetical.
PER_SITE = '''model PerSite version "0.2" {
  type V = enum { pass, fail }
  people [z, a]
  actor Z { source document }
  actor A { source record }
  actor Intake { executors [z, a]
    capability first() -> V { reads [Z]; duration fixed 1min; result nondet { pass } }
    capability second() -> V { reads [A]; duration fixed 1min; result nondet { pass } }
  }
  actor Checker { executors [z, a]
    capability confirm(e:V) -> V { duration fixed 1min; result nondet { pass } }
  }
  cut C { include [Z,A,Intake,Checker] }
  process Main { zed = Intake.first(); alpha = Intake.second();
    one = Checker.confirm(zed); two = Checker.confirm(alpha); end posted }
  bounds { cases 1; live_tasks 1; clock 10min }
  responsible a for Checker.confirm
  verify reachable end.posted
}
'''

FALLBACK = '''model Fallback version "0.2" {
  type V = enum { pass, fail }
  people [c1]
  actor Doc { source document }
  actor Probe { executors [c1]; capability run() -> V { duration fixed 1min; result nondet { pass, fail } } }
  actor Worker { executors [c1]; capability read() -> V { reads [Doc]; duration fixed 1min; result nondet { pass } } }
  cut C { include [Doc,Probe,Worker] }
  process Main { p = Probe.run();
    degrade Manual when p == fail { f = Worker.read(); end done }
    else { n = Worker.read(); end done }
  }
  bounds { cases 1; live_tasks 1; clock 10min }
  verify reachable end.done
}
'''


def configured(s):
    start = s.index('  process Main {')
    end = s.index('  bounds {', start)
    second = s[start:end].replace('process Main', 'process Second', 1)
    return add(s, second + 'configuration A { cut S; process Main }\nconfiguration B { cut S; process Second }')


def deterministic(s):
    return s.replace('result nondet { pass, fail }', 'result nondet { pass }')


def simulated(s):
    return add(deterministic(s), 'simulation replications 4 seed 7 confidence 0.5')


def rule_source():
    s = add(S1, 'actor ApprovalRule { control policy; version "r1"; decides needed: bool = true }')
    s = edit(s, 'include [Invoice,Clerk,Ledger]', 'include [Invoice,Clerk,Ledger,ApprovalRule]')
    return edit(s, 'capability post(v:V) -> Ack {', 'capability post(v:V) -> Ack { requires ApprovalRule.needed == true else ok;')


def contest_source():
    s = edit(S1, 'capability enter() -> V {', 'capability recheck() -> V { reads [Invoice]; duration fixed 1min; result nondet { pass } }\n    capability enter() -> V {')
    s = add(s, '''actor Vendor { control party; standing over [end.returned] basis "claim" }
authority ReviewGrant { basis "test delegation"; grants c1 to Clerk.confirm; issued_by c2 }
contract Appeal { contest Vendor on end.returned route Clerk.recheck reversible_by [c1] respond_by 2min }''')
    return edit(s, 'include [Invoice,Clerk,Ledger]', 'include [Invoice,Clerk,Ledger,Vendor]')
