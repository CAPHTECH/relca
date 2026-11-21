"""Handwritten inputs and set verdicts for record registration.

No Relca execution happens here. Legacy rewrites use observed case snapshots;
they check the declared must/may sets, not the new presentation of records.
"""
import json
from pathlib import Path

DATA = Path(__file__).resolve().parent
ROOT = DATA.parents[3]
R0 = (DATA / 'r0.relca').read_text()
ID = 'contract Basis #1 evidence_basis'


def edit(source, old, new):
    assert source.count(old) == 1, (old, source.count(old))
    return source.replace(old, new)


def add(source, text):
    end = source.rfind('}')
    return source[:end] + '\n' + text + '\n' + source[end:]


def plain(source):
    return '\n'.join(line for line in source.splitlines()
                     if not line.strip().startswith('contract Basis {')) + '\n'


def deterministic(source):
    return source.replace('result nondet { pass, fail }', 'result nondet { pass }')


def simulated(source):
    return add(deterministic(source), 'simulation replications 4 seed 7 confidence 0.5')


def configured(source):
    begin = source.index('  process Main {')
    end = source.index('  bounds {', begin)
    other = source[begin:end].replace('process Main {', 'process Alternative {', 1)
    return add(source, other + '\nconfiguration A { cut S; process Main }\nconfiguration B { cut S; process Alternative }')


def legacy(source):
    """Manual value-free-record encoding by per-case observed Ack snapshots.

    Actor identity cannot be preserved by this encoding. The record is not a
    source in either input. Keep the otherwise empty Actor and Cut membership;
    swap only registration/read edges and add optional Ack cells.
    """
    source = source.replace('registers [Notes2, Notes]', 'writes [note2, note]')
    source = source.replace('reads [Notes2, Notes]', 'reads [note2, note]')
    declarations = []
    for record, cell in [('Notes2', 'note2'), ('Notes', 'note')]:
        if f'actor {record} {{' in source:
            declarations.append(f'{cell}: optional<Ack>')
            source = source.replace(f'registers [{record}]', f'writes {cell}')
            source = source.replace(f'reads [{record}]', f'reads [{cell}]')
    if 'observed case {' in source:
        return edit(source, 'observed case {', 'observed case { ' + '; '.join(declarations) + '; ')
    return add(source, 'observed case { ' + '; '.join(declarations) + ' }')


DOMINATING = edit(R0, 'reads [Invoice]; duration fixed 1min; result nondet { pass }',
                  'reads [Invoice]; duration fixed 1min; result nondet { pass, fail }')
CONDITIONAL = edit(DOMINATING, 'r = Clerk.register()', 'if e == pass { r = Clerk.register() } else {}')
AUDITOR = 'actor Auditor { executors [p1]; capability overwrite() -> Ack { reads [Invoice]; registers [Notes]; duration fixed 1min; result nondet { ok } } }'
OVERWRITE = add(edit(R0, 'Notes, Clerk,', 'Notes, Auditor, Clerk,'), AUDITOR)
COPY = edit(R0, 'actor Notes { }', 'actor Notes { }\n  actor Notes2 { }')
COPY = edit(COPY, 'capability enter()', 'capability copy() -> Ack { reads [Notes]; registers [Notes2]; duration fixed 1min; result nondet { ok } }\n    capability enter()')
COPY = edit(COPY, 'r = Clerk.register()', 'r = Clerk.register()\n    c = Clerk.copy()')
COPY = edit(COPY, 'capability review(e: V) -> V { reads [Notes]', 'capability review(e: V) -> V { reads [Notes2]')
COPY = edit(COPY, 'Know, Notes, Clerk', 'Know, Notes, Notes2, Clerk')
BLIND = edit(R0, 'reads [Know]; registers [Notes]', 'registers [Notes]')
LATENT = edit(BLIND, 'actor Know { source tacit "vendor knowledge of p1" }',
              'actor Know { source tacit "vendor knowledge of p1"; latent sure: bool = false }')
LATENT = edit(LATENT, 'registers [Notes]; duration fixed 1min; result nondet { ok }',
              'registers [Notes]; kernel_reads latent [Know.sure]; duration fixed 1min; outcomes kernel K')
LATENT = add(LATENT, 'kernel K given [Know.sure] { false -> { 1: ok }; true -> { 1: ok } }')
REQUIRES = add(edit(R0, 'type Ack = enum { ok }', 'type Ack = enum { ok, rejected }'),
               'observed case { eligible: optional<V> }')
REQUIRES = edit(REQUIRES, 'reads [Invoice]; duration', 'reads [Invoice]; writes eligible; duration')
REQUIRES = edit(REQUIRES, 'capability register() -> Ack {', 'capability register() -> Ack { requires eligible == pass else rejected;')
REQUIRES = edit(REQUIRES, 'reads [Invoice]; writes eligible; duration fixed 1min; result nondet { pass }',
                'reads [Invoice]; writes eligible; duration fixed 1min; result nondet { pass, fail }')
RELAY = edit(BLIND, 'register() -> Ack', 'register(k: V) -> Ack')
RELAY = edit(RELAY, 'r = Clerk.register()', 'k = Clerk.judge(e)\n    r = Clerk.register(k)')

# Explicit expectations; the constructor does not execute or infer verdicts.
CASES = {
    'a1_r0': (R0, 'satisfied', 4),
    'a1_registrar_blind': (BLIND, 'violated', 4),
    'a1_direct': (edit(R0, 'evidence_basis Reviewer.review', 'evidence_basis direct Reviewer.review'), 'violated', 4),
    'a2_dominating': (DOMINATING, 'satisfied', 4),
    'a2_conditional': (CONDITIONAL, 'unsupported', None),
    'a2_never': (edit(R0, '    r = Clerk.register()\n', ''), 'violated', 3),
    'a3_single_result': (R0, 'satisfied', 4),
    'a4_overwrite_before': (edit(OVERWRITE, 'r = Clerk.register()', 'o = Auditor.overwrite()\n    r = Clerk.register()'), 'satisfied', 5),
    'a4_overwrite_after': (edit(OVERWRITE, 'r = Clerk.register()', 'r = Clerk.register()\n    o = Auditor.overwrite()'), 'violated', 5),
    'a4_overwrite_maybe': (edit(edit(OVERWRITE, 'r = Clerk.register()', 'r = Clerk.register()\n    if e == pass { o = Auditor.overwrite() } else {}'),
                                'reads [Invoice]; duration fixed 1min; result nondet { pass }',
                                'reads [Invoice]; duration fixed 1min; result nondet { pass, fail }'), 'unsupported', None),
    'a5_copy_of_copy': (COPY, 'satisfied', 5),
    'f5_latent_not_observation': (LATENT, 'violated', 4),
    'f5_latent_with_direct_control': (edit(LATENT, 'registers [Notes]; kernel_reads', 'reads [Know]; registers [Notes]; kernel_reads'), 'satisfied', 4),
    'f8_requires': (REQUIRES, 'unsupported', None),
    'u4_relayed_argument': (RELAY, 'satisfied', 5),
    'u4_relayed_snapshot': (add(edit(BLIND, 'registers [Notes]', 'reads [knowledge]; registers [Notes]'),
                               'observed case { knowledge: optional<V> }').replace('reads [Know]; duration', 'reads [Know]; writes knowledge; duration').replace(
                                   'r = Clerk.register()', 'k = Clerk.judge(e)\n    r = Clerk.register()'), 'satisfied', 5),
}

MULTIPLE = edit(edit(R0, 'actor Notes { }', 'actor Notes { }\n  actor Notes2 { }'),
                'registers [Notes]', 'registers [Notes2, Notes]')
MULTIPLE = edit(MULTIPLE, 'reads [Notes]', 'reads [Notes2, Notes]')
MULTIPLE = edit(MULTIPLE, 'Know, Notes, Clerk', 'Know, Notes, Notes2, Clerk')
CASES['u2_multiple_targets'] = (MULTIPLE, 'satisfied', 4)
CONTROL_ONLY = edit(BLIND, '    r = Clerk.register()\n', '')
CONTROL_ONLY = edit(CONTROL_ONLY, 'a = Clerk.judge(e)', 'a = Clerk.judge(e)\n    if a == pass { r = Clerk.register() } else {}')
CONTROL_ONLY = edit(CONTROL_ONLY, 'reads [Know]; duration fixed 1min; result nondet { pass, fail }',
                    'reads [Know]; duration fixed 1min; result nondet { pass }')
CASES['u4_control_not_data'] = (CONTROL_ONLY, 'violated', 4)
SAME_SOURCE_OVERWRITE = edit(DOMINATING, 'r = Clerk.register()',
                             'r = Clerk.register()\n    if e == pass { r2 = Clerk.register() } else {}')
CASES['u7_same_source_conditional_overwrite'] = (SAME_SOURCE_OVERWRITE, 'satisfied', (4, 5))


def responsibility_source(same_person=False, conditional=False):
    source = plain(CONDITIONAL) if conditional else R0
    source = add(edit(source, 'reads [Invoice]', 'reads [Know]'),
                 f'responsible {"p1" if same_person else "q1"} for Reviewer.review')
    if same_person:
        source = edit(source, 'actor Reviewer { executors [q1]', 'actor Reviewer { executors [p1]')
    return source


def static_cases():
    for kind in ('document', 'record', 'tacit "knowledge"'):
        yield 'source_' + kind.split()[0], edit(R0, 'actor Notes { }', f'actor Notes {{ source {kind} }}'), 'E-CON-0', 'registration_target_is_source'
    for name, behavior in [('capacity', 'capacity 1'), ('executors', 'executors [p1]'),
                           ('capability', 'capability unused() -> Ack { duration fixed 0min; result nondet { ok } }'),
                           ('state', 'state flag: bool = false'), ('latent', 'latent flag: bool = false')]:
        yield name, edit(R0, 'actor Notes { }', f'actor Notes {{ {behavior} }}'), 'E-CON-0', 'registration_target_has_behavior'
    dependent = edit(edit(R0, 'type Ack = enum { ok }', 'type Ack = enum { ok, rejected }'),
                     'registers [Notes]; duration fixed 1min; result nondet { ok }',
                     'registers [Notes]; duration fixed 1min; result nondet { ok, rejected }')
    yield 'result_dependent', dependent, 'U-SEM-0', 'registration_result_dependent'
    yield 'basis_target_not_source', edit(R0, 'evidence_basis Reviewer.review [Know]', 'evidence_basis Reviewer.review [Notes]'), 'E-CON-0', 'evidence_basis_not_source'
    # Both effects and reads name an excluded target, never silently discard it.
    excluded = edit(edit(R0, 'Know, Notes, Clerk', 'Know, Clerk'),
                    'Reviewer, Ledger] }', 'Reviewer, Ledger]; exclude Notes reason "outside" loses "record access" }')
    yield 'cut_registration', edit(excluded, 'reads [Notes]', 'reads [Know]'), 'E-CUT-1', 'excluded'
    yield 'cut_read', excluded, 'E-CUT-1', 'excluded'
    yield 'shared_cases', add(edit(R0, 'cases 1', 'cases 2'), 'arrivals fixed [0min, 0min]'), 'U-SEM-0', 'registration_feature_combination'
    latent_guard = edit(R0, 'actor Know { source tacit "vendor knowledge of p1" }',
                        'actor Know { source tacit "vendor knowledge of p1"; latent sure: bool = false }')
    yield 'latent_condition', edit(latent_guard, 'if a == pass && b == pass', 'if Know.sure == true'), 'E-POL-LATENT', 'policy cannot read Actor latent state'


def combination_cases():
    # Remove Basis so each gate must be owned by registration itself.
    source = plain(R0)
    for role, call in [('registrar', 'r = Clerk.register()'), ('reader', 'b = Reviewer.review(e)')]:
        for name, suffix in [('response', 'response_loss none'),
                             ('retry', 'response_loss sequence [unknown, delivered] retry max_attempts 2 when receipt unknown backoff fixed 1min exhausted end returned')]:
            yield name + '_' + role, edit(source, call, call + ' ' + suffix), 'registration_feature_combination'
        degraded = edit(source, call, f'degrade Manual when e == pass {{ {call} }} else {{ end returned }}')
        if role == 'reader':
            degraded = edit(degraded, 'if a == pass && b == pass {} else { end returned }\n    x = Ledger.post(a, b)', '')
        yield 'fallback_' + role, degraded, 'registration_feature_combination'
        yield 'recovery_' + role, recovery_source(role), 'registration_feature_combination'
    start = add(edit(source, 'registers [Notes]', 'start { started = true }; registers [Notes]'),
                'observed case { started: bool = false }')
    yield 'start', start, 'registration_feature_combination'
    fork = edit(source, 'r = Clerk.register()\n    a = Clerk.judge(e)\n    b = Reviewer.review(e)',
                'a = Clerk.judge(e)\n    fork f { r = Clerk.register(); b = Reviewer.review(e) }\n    join all f')
    fork = edit(fork, 'live_tasks 1', 'live_tasks 2')
    yield 'fork_sibling', fork, 'read variable has a concurrent writer'
    for role in ('registrar', 'reader'):
        yield 'contest_' + role, contest_source(role), 'registration_feature_combination'


def recovery_source(role):
    return '''model Recovery version "0.2" {
 type Ack = enum { ok, duplicate }
 people [p1,q1]
 actor Know { source tacit "knowledge" }
 actor Notes { }
 actor Registrar { executors [p1]; capability store() -> Ack { reads [Know]; registers [Notes]; duration fixed 1min; result nondet { ok } } }
 actor Reader { executors [q1]; capability read() -> Ack { reads [Notes]; duration fixed 1min; result nondet { ok } } }
 actor Ledger { capacity 1; state keys: set<CaseId>
   capability post(key:CaseId) -> Ack { commits; duration fixed 1min; writes keys; idempotency key key
     atomic { if key in keys { result duplicate } else { keys += key; result ok } }
     cancel forbidden_after_start
   }
 }
 cut S { include [Know,Notes,Registrar,Reader,Ledger] }
 process Main { BEFORE x=Ledger.post(case.id) response_loss sequence [unknown]
   on receipt_unknown { EFFECT stop receipt_unknown }; end posted }
 bounds { cases 1; live_tasks 1; clock 10min }
 verify safety no_resource_overcommit
}'''.replace('BEFORE', 'r=Registrar.store();' if role == 'reader' else '').replace(
        'EFFECT', 'rr=Registrar.store();' if role == 'registrar' else 'b=Reader.read();')


def contest_source(role):
    return '''model ContestRecord version "0.2" {
 type V = enum { pass, fail }
 type Ack = enum { ok }
 people [p1,q1]
 actor Know { source tacit "knowledge" }
 actor Notes { }
 actor Clerk { executors [p1]; capability judge() -> V { duration fixed 1min; result nondet { fail } } }
 actor Registrar { executors [q1]; capability store() -> Ack { reads [Know]; registers [Notes]; duration fixed 1min; result nondet { ok } } }
 actor Reader { executors [q1]; capability read() -> Ack { reads [Notes]; duration fixed 1min; result nondet { ok } } }
 actor Vendor { control party; standing over [end.returned] basis "claim" }
 authority G { basis "decision"; grants p1 to Clerk.judge }
 cut S { include [Know,Notes,Clerk,Registrar,Reader,Vendor] }
 process Main { r=Registrar.store(); d=Clerk.judge(); if d == fail { end returned } else { end posted } }
 bounds { cases 1; live_tasks 1; clock 10min }
 verify safety no_resource_overcommit
 contract Appeal { contest Vendor on end.returned route ROUTE reversible_by [p1] respond_by 2min }
}'''.replace('ROUTE', 'Registrar.store' if role == 'registrar' else 'Reader.read')


def per_case():
    return add(edit(edit(R0, 'actor Notes { }', 'actor Notes { instance per_case }'), 'cases 1', 'cases 2'),
               'arrivals fixed [0min, 0min]')


EMPTY_LOG = '{"kind":"header","schema":"relca-log/v1","clock":"test-clock","time_unit":"min","window":{"from":0,"to":10},"cases":[],"actor_map":[],"executor_map":[]}\n'


def manifest(mode):
    return {'schema': 'relca-sensitivity/v1', 'scope': {'kind': 'enumerated_models', 'rationale': 'record command boundary'},
            'points': [{'id': 'record', 'model': 'model.relca', 'configuration': None, 'scenario': None}],
            'targets': [{'kind': 'probability_query', 'id': 'probability reached #1'}],
            'confidence': '1/2' if mode == 'simulation' else None}


def wlin(conditional=False, direct=False, blind=False):
    """Independent common-error pair: only B can add Know, and only must may suppress.

    In the conditional variant B has a may-only registration. Counting that
    record as a new explicit source would erase W-LIN-2 without uncertainty.
    """
    return '''model Correlation version "0.2" {
 type Entry = enum { item }
 type V = enum { pass, fail }
 type Ack = enum { ok }
 people [p1,q1]
 latent case { bad: bool = false }
 observed case { a, b: optional<V> }
 actor Know { source tacit "vendor knowledge" }
 actor Invoice { source document }
 actor Other { source record }
 actor Notes { }
 kernel K0 { * -> { 1: item with bad = false } }
 kernel KA given [bad] { false -> { 0.5: pass; 0.5: fail }; true -> { 0.5: pass; 0.5: fail } }
 kernel KB given [bad] { false -> { 1: pass }; true -> { 1: pass } }
 actor Producer { capacity 1; capability make() -> Entry { reads [Invoice]; duration fixed 1min; outcomes kernel K0; writes bad } }
 actor Registrar { executors [p1]; capability store() -> Ack { READ registers [Notes]; duration fixed 1min; result nondet { ok } } }
 actor A { capacity 1; capability check(e:Entry) -> V { reads [Other]; duration fixed 1min; outcomes kernel KA; kernel_reads latent [bad]; writes a } }
 actor B { executors [q1]; capability check(e:Entry) -> V { reads [TARGET]; duration fixed 1min; outcomes kernel KB; kernel_reads latent [bad]; writes b } }
 cut S { include [Know,Invoice,Other,Notes,Producer,Registrar,A,B] }
 process Main { e=Producer.make(); aV=A.check(e); REGISTER bV=B.check(e); end done }
 bounds { cases 1; live_tasks 1; clock 10min }
 verify safety no_resource_overcommit
}'''.replace('READ', '' if blind else 'reads [Know];').replace('TARGET', 'Know' if direct else 'Notes').replace(
        'REGISTER', 'if aV == pass { r=Registrar.store() } else {}' if conditional else 'r=Registrar.store();')


if __name__ == '__main__':
    print(json.dumps({name: {'source': src, 'legacy': legacy(src), 'expected': expected}
                      for name, (src, expected, _) in CASES.items()}))
