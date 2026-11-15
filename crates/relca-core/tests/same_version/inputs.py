"""Hand-authored ADR 0076 inputs, verdicts and finite old-DSL witnesses.

This module never invokes Relca. Durations and identities are not snapshots.
"""
from pathlib import Path

DATA = Path(__file__).resolve().parent
ID = 'contract V #1 same_version'
CLAUSE = 'same_version Checker.check(d), Approver.approve(d)'

HEADER = '''model Versions version "0.2" {
  type Doc = enum { draft }
  type Verdict = enum { pass, fail }
  type Ack = enum { filed }
  people [w1,e1,b1,c1,a1]
  actor Request { source document }
  actor Writer { via model "gen@v1"; executors [w1]
    capability draft() -> Doc { reads [Request]; duration fixed 1min; result nondet { draft } } }
  actor Editor { executors [e1]
    capability fix(d:Doc) -> Doc { duration fixed 1min; result nondet { draft } } }
  actor Checker { executors [c1]
    capability check(d:Doc) -> Verdict { reads [Request]; duration fixed 1min; result nondet { pass, fail } } }
  actor Builder { executors [b1]
    capability assemble(d:Doc) -> Doc { duration fixed 1min; result nondet { draft } } }
  actor Approver { executors [a1]
    capability approve(d:Doc, k:Verdict) -> Ack { commits; duration fixed 1min; result nondet { filed } } }
  responsible a1 for Approver.approve
  responsible c1 for Checker.check
  cut Scope { include [Request,Writer,Editor,Checker,Builder,Approver] }
  bounds { cases 1; live_tasks 2; clock 10min }
  verify reachable end.done
'''


def add(source, text):
    i = source.rfind('}')
    return source[:i] + '\n' + text + '\n' + source[i:]


def edit(source, old, new):
    assert source.count(old) == 1, (old, source.count(old))
    return source.replace(old, new)


def model(body):
    return HEADER + '  process Main {\n' + body + '\n  }\n}\n'


def contract(source, clause=CLAUSE):
    return add(source, 'contract V { ' + clause + ' }')


def plain(source):
    return '\n'.join(s for s in source.splitlines() if not s.startswith('contract V {')) + '\n'


# (source, contract verdict, terminal elapsed/work ranges, witness target)
# The fifth positive is the second positive: sequential check, then approve.
CASES = {}
for name, arg, expected in [('a1_fixed', 'f', 'satisfied'), ('a1_unfixed', 'd', 'violated')]:
    s = model(f'''    d = Writer.draft()
    f = Editor.fix(d)
    k = Checker.check(f)
    if k == pass {{}} else {{ end rejected }}
    a = Builder.assemble({arg})
    p = Approver.approve(a,k)
    end done''')
    CASES[name] = (contract(s, 'same_version Checker.check(d), Builder.assemble(d)'), expected,
                   {'done': (5,5,5,5), 'rejected': (3,3,3,3)}, 'Builder.assemble')

REUSE = model('''    d = Writer.draft()
    k = Checker.check(d)
    if k == pass {} else { end rejected }
    p = Approver.approve(d,k)
    end done''')
CASES['a2_reuse'] = (contract(REUSE), 'satisfied', {'done': (3,3,3,3), 'rejected': (2,2,2,2)}, 'Approver.approve')
REUSED = edit(REUSE, 'd = Writer.draft()', 'd = Writer.draft(); d_dup = Writer.draft()')
REUSED = edit(REUSED, 'Approver.approve(d,k)', 'Approver.approve(d_dup,k)')
CASES['a2_reused'] = (contract(REUSED), 'violated', {'done': (4,4,4,4), 'rejected': (3,3,3,3)}, 'Approver.approve')
for name, recheck, expected in [('a3_recheck', True, 'satisfied'), ('a3_no_recheck', False, 'violated')]:
    inside = 'f = Editor.fix(d); '
    if recheck:
        inside += 'k2 = Checker.check(f); if k2 == pass {} else { end rejected }; '
    s = model('''    d = Writer.draft()
    k = Checker.check(d)
    ready = if k == fail { ''' + inside + '''yield f } else { yield d }
    p = Approver.approve(ready,k)
    end done''')
    times = {'done': (3,5,3,5), 'rejected': (4,4,4,4)} if recheck else {'done': (3,4,3,4)}
    CASES[name] = (contract(s), expected, times, 'Approver.approve')
for name, arg, expected in [('a4_latest_f', 'f', 'satisfied'), ('a4_latest_d', 'd', 'violated')]:
    s = model(f'''    d = Writer.draft()
    k = Checker.check(d)
    f = Editor.fix(d)
    k2 = Checker.check(f)
    if k2 == pass {{}} else {{ end rejected }}
    p = Approver.approve({arg},k2)
    end done''')
    CASES[name] = (contract(s), expected, {'done': (5,5,5,5), 'rejected': (4,4,4,4)}, 'Approver.approve')
FORK = model('''    d = Writer.draft()
    fork g { k = Checker.check(d); p = Approver.approve(d) }
    join all g
    end done''').replace('approve(d:Doc, k:Verdict)', 'approve(d:Doc)')
CASES['a5_parallel'] = (contract(FORK), 'violated', {'done': (2,2,3,3)}, 'Approver.approve')
SLOW_B = edit(FORK, 'capability approve(d:Doc) -> Ack { commits; duration fixed 1min;',
              'capability approve(d:Doc) -> Ack { commits; duration fixed 2min;')
CASES['a5_slow_B'] = (contract(SLOW_B), 'violated', {'done': (3,3,4,4)}, 'Approver.approve')

NO_CHECK = edit(REUSE, 'k = Checker.check(d)\n    if k == pass {} else { end rejected }', '')
NO_CHECK = edit(NO_CHECK, 'Approver.approve(d,k)', 'Approver.approve(d)')
NO_CHECK = edit(NO_CHECK, 'approve(d:Doc, k:Verdict)', 'approve(d:Doc)')
CASES['no_check'] = (contract(NO_CHECK), 'violated', {'done': (2,2,2,2)}, 'Approver.approve')
UNUSED = edit(REUSE, 'p = Approver.approve(d,k)', '')
CASES['unused_B'] = (contract(UNUSED), 'vacuous', {'done': (2,2,2,2), 'rejected': (2,2,2,2)}, 'Approver.approve')
REJECT_A = add(REUSE, 'observed case { enabled: bool = false }')
REJECT_A = edit(REJECT_A, 'capability check(d:Doc) -> Verdict {',
                'capability check(d:Doc) -> Verdict { requires enabled == true else pass;')
CASES['f7_rejected_A'] = (contract(REJECT_A), 'violated', {'done': (2,2,2,2)}, 'Approver.approve')
REJECT_B = add(REUSE, 'observed case { enabled: bool = false }')
REJECT_B = edit(REJECT_B, 'capability approve(d:Doc, k:Verdict) -> Ack {',
                'capability approve(d:Doc, k:Verdict) -> Ack { requires enabled == true else filed;')
CASES['rejected_B'] = (contract(REJECT_B), 'vacuous', {'done': (2,2,2,2), 'rejected': (2,2,2,2)}, 'Approver.approve')

# A failed check is still a normal completion. Return spelling is not semantics.
FAIL_CHECK = edit(REUSE, 'result nondet { pass, fail }', 'result nondet { fail }')
FAIL_CHECK = edit(FAIL_CHECK, 'if k == pass {} else { end rejected }', '')
CASES['failed_result_A'] = (contract(FAIL_CHECK), 'satisfied', {'done': (3,3,3,3)}, 'Approver.approve')

REFUSED_PRODUCER = add(REUSE, 'observed case { enabled: bool = false }')
REFUSED_PRODUCER = edit(REFUSED_PRODUCER, 'capability draft() -> Doc {',
    'capability draft() -> Doc { requires enabled == true else draft;')
CASES['refused_producer'] = (contract(REFUSED_PRODUCER), 'satisfied', {'done': (2,2,2,2), 'rejected': (1,1,1,1)}, 'Approver.approve')

# Nested aliases must retain d/f rather than acquiring a merge-slot identity.
NESTED = CASES['a3_recheck'][0].replace('yield f } else { yield d }',
    'inner = if k2 == pass { yield f } else { yield d }; yield inner } else { yield d }')
CASES['nested_merge'] = (NESTED, 'satisfied', CASES['a3_recheck'][2], 'Approver.approve')


def witness(name):
    """Finite old-DSL monitor. It is not a same_version implementation.

    A finite monitor encodes the *hand-computed* identity relation. Only the
    bad branch of the no-recheck merge writes no, with the existing fix finish.
    Normal-completion activation is supplied by the existing invariant.
    """
    source, expected, _, target = CASES[name]
    source = plain(source)
    initial = expected != 'violated' or name == 'a3_no_recheck'
    source = add(source, 'type VersionTruth = enum { yes, no }')
    field = 'version_ok: VersionTruth = ' + ('yes' if initial else 'no')
    if 'observed case {' in source:
        source = source.replace('observed case {', 'observed case { ' + field + '; ', 1)
    else:
        source = add(source, 'observed case { ' + field + ' }')
    if name == 'a3_no_recheck':
        source = source.replace('capability fix(d:Doc) -> Doc {',
            'capability fix(d:Doc) -> Doc { writes version_ok; finish { version_ok = no };')
    return add(source, f'contract Witness {{ invariant {target}.finish -> version_ok == yes }}')


def deterministic(source):
    return source.replace('result nondet { pass, fail }', 'result nondet { pass }')


def configured(source):
    start = source.index('  process Main {')
    end = source.index('\n  }', start) + len('\n  }')
    other = source[start:end].replace('process Main {', 'process Alternative {')
    return add(source, other + '\nconfiguration A { cut Scope; process Main }\nconfiguration B { cut Scope; process Alternative }')


def combinations():
    yield 'repeat', contract(edit(REUSE, 'd = Writer.draft()',
        'repeat attempt at_most 1 { d = Writer.draft(); leave } on_limit { end limited }')), 'loop_feature_combination'
    any_join = edit(FORK, 'join all g', 'join any g where result == pass tie by [k,p] remaining cancel_then_drain if none -> done')
    # Both fork branches need one common result type for join-any selection.
    any_join = any_join.replace('approve(d:Doc) -> Ack', 'approve(d:Doc) -> Verdict').replace('result nondet { filed }', 'result nondet { pass }')
    any_join = any_join.replace('duration fixed 1min; result nondet', 'cancel immediate; duration fixed 1min; result nondet')
    yield 'join_any', contract(any_join), 'same_version_feature_combination'
    for name, suffix in [
        ('response_loss', 'response_loss none'),
        ('resend', 'response_loss sequence [unknown,delivered] retry max_attempts 2 when receipt unknown backoff fixed 1min exhausted end returned'),
        ('result_retry', 'response_loss none retry max_attempts 2 when result [fail] backoff fixed 1min exhausted end returned'),
    ]:
        s = edit(REUSE, 'k = Checker.check(d)', 'k = Checker.check(d) ' + suffix)
        yield name, contract(s), 'same_version_feature_combination'
    recovery = add(REUSE, '''type Receipt = enum { posted, duplicate }
    actor Ledger { capacity 1; state keys: set<CaseId>
      capability post(key:CaseId) -> Receipt { commits; duration fixed 1min; writes keys; idempotency key key
        atomic { if key in keys { result duplicate } else { keys += key; result posted } }
        cancel forbidden_after_start } }''')
    recovery = recovery.replace('include [Request,', 'include [Ledger,Request,')
    recovery = edit(recovery, 'p = Approver.approve(d,k)',
        'x = Ledger.post(case.id) response_loss sequence [unknown] on receipt_unknown { stop receipt_unknown }')
    yield 'recovery', contract(recovery), 'same_version_feature_combination'
    s = edit(REUSE, 'p = Approver.approve(d,k)', 'degrade Manual when k == pass { p = Approver.approve(d,k) } else { end rejected }')
    yield 'degrade', contract(s), 'same_version_feature_combination'


def static_cases():
    for name, clause, code, reason in [
        ('unknown_actor', 'same_version Missing.check(d), Approver.approve(d)', 'E-NAME-0', 'undefined Actor.Capability'),
        ('unknown_capability', 'same_version Checker.missing(d), Approver.approve(d)', 'E-NAME-0', 'undefined Actor.Capability'),
        ('unknown_first_parameter', 'same_version Checker.check(missing), Approver.approve(d)', 'E-NAME-0', 'undefined parameter'),
        ('unknown_second_parameter', 'same_version Checker.check(d), Approver.approve(missing)', 'E-NAME-0', 'undefined parameter'),
        ('type_mismatch', 'same_version Checker.check(d), Approver.approve(k)', 'E-TYPE-0', 'same_version_type_mismatch'),
        ('same_target', 'same_version Checker.check(d), Checker.check(d)', 'E-CON-0', 'same_version_same_target'),
    ]:
        yield name, contract(REUSE, clause), code, reason
    alias = add(REUSE, 'type OtherDoc = enum { draft }')
    alias = edit(alias, 'capability assemble(d:Doc)', 'capability assemble(d:OtherDoc)')
    yield 'nominal_types', contract(alias, 'same_version Checker.check(d), Builder.assemble(d)'), 'E-TYPE-0', 'same_version_type_mismatch'


def contest_source():
    source = add(REUSE, '''actor Party { control party; standing over [end.rejected] basis "claim" }
      actor Route { executors [b1]
        capability respond(d:Doc) -> Ack { duration fixed 1min; result nondet { filed } } }
      contract RouteControl { contest Party on end.rejected route Route.respond reversible_by [c1] respond_by 1min }''')
    source = source.replace('include [Request,', 'include [Party,Route,Request,')
    return contract(source, 'same_version Checker.check(d), Route.respond(d)')


EMPTY_LOG = '{"kind":"header","schema":"relca-log/v1","clock":"test-clock","time_unit":"min","window":{"from":0,"to":10},"cases":[],"actor_map":[],"executor_map":[]}\n'


def manifest(mode):
    return {'schema': 'relca-sensitivity/v1',
            'scope': {'kind': 'enumerated_models', 'rationale': 'version command gate'},
            'points': [{'id': 'version', 'model': 'model.relca', 'configuration': None, 'scenario': None}],
            'targets': [{'kind': 'probability_query', 'id': 'probability reached #1'}],
            'confidence': '1/2' if mode == 'simulation' else None}


if __name__ == '__main__':
    import json
    print(json.dumps({name: {'source': row[0], 'expected': row[1], 'witness': witness(name)}
                      for name, row in CASES.items()}))
