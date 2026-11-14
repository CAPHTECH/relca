"""Hand-authored ADR 0071 inputs and explicit old-DSL projections.

Projections select a configuration and retain only applicable contracts. They
validate the executable substrate, not the new scope implementation itself.
"""
import json
from pathlib import Path

DATA = Path(__file__).resolve().parent
ROOT = DATA.parents[3]
BASE = (DATA / 'm1.relca').read_text()
CLAUSE = 'contract Controls { authority_basis Screen.approve Mgr }'
REASON = 'As-Is has no approval screen'
ID = 'contract Controls #1 authority_basis'
BASELINE = 'contract Baseline { excludes Desk.pay.finish, Screen.approve.start }'


def edit(source, old, new):
    assert source.count(old) == 1, (old, source.count(old))
    return source.replace(old, new)


def add(source, text):
    i = source.rfind('}')
    return source[:i] + '\n  ' + text + '\n' + source[i:]


def scoped(source=BASE, name='Controls', plans='New', reason=REASON):
    return edit(source, f'contract {name} {{',
                f'contract {name} applies_to [{plans}] reason "{reason}" {{')


def without_control(source=BASE):
    return edit(source, CLAUSE, '')


def deterministic(source):
    return add(source.replace('result nondet { ok }', 'outcomes kernel Certain'),
               'kernel Certain { * -> { 1: ok } }')


def simulation(source):
    return add(deterministic(source), 'simulation replications 4 seed 7 confidence 0.5')


def both_call(source=BASE):
    return edit(source, 'process AsIs { p = Desk.pay(); end done }',
                'process AsIs { a = Screen.approve(); p = Desk.pay(); end done }')


def bad_grant(source=BASE):
    return edit(source, 'grants boss to Screen.approve', 'grants clerk to Screen.approve')


def dead_branch(source=BASE):
    source = edit(source, 'type T = enum { ok }', 'type T = enum { ok, bad }')
    return edit(source, 'a = Screen.approve(); p = Desk.pay(); end done',
                'p = Desk.pay(); if p == bad { a = Screen.approve() } else {}; end done')


def unused(source=BASE):
    return edit(source, 'process ToBe { a = Screen.approve(); p = Desk.pay(); end done }',
                'process ToBe { p = Desk.pay(); end done }')


def only_new(source):
    source = edit(source, '  configuration Old { cut K; process AsIs }\n', '')
    return edit(source, '  process AsIs { p = Desk.pay(); end done }\n', '')


def ordinal_source():
    return add(scoped(), 'contract Later { permitted Desk.pay }')


def evidence(source=BASE, read=True):
    source = without_control(source)
    source = add(source, 'actor Document { source document }')
    source = edit(source, 'include [Desk, Screen]', 'include [Desk, Screen, Document]')
    if read:
        source = edit(source, 'capability approve() -> T {', 'capability approve() -> T { reads [Document];')
    return add(source, 'contract Evidence { evidence_basis Screen.approve [Document] }')


def clause_cases():
    """(kind, body, New verdict, Old masks_call); all bodies are old DSL."""
    return [
        ('authority_basis', 'authority_basis Screen.approve Mgr', 'satisfied', False),
        ('permitted', 'permitted Screen.approve', 'satisfied', False),
        ('at_most_once', 'at_most_once Desk.pay.commit per case.id', 'satisfied', True),
        ('requires_event', 'requires_event Desk.pay.start after Screen.approve.finish', 'satisfied', True),
        ('separation', 'separation Screen.approve.finish, Desk.pay.finish', 'satisfied', True),
        ('excludes', 'excludes Screen.approve.finish, Desk.pay.start', 'violated', True),
        ('invariant', 'invariant Screen.approve.finish -> flag == true', 'violated', False),
        ('invariant', 'invariant flag == true', 'violated', True),
        ('response', 'response case.received -> one_of(end.done) by 1min after case.received otherwise unresolved', 'violated', True),
    ]


def clause_model(body):
    source = add(edit(BASE, CLAUSE, f'contract Controls {{ {body} }}'),
                 'observed case { flag: bool = false }')
    if body.startswith('at_most_once'):
        source = edit(source, 'capability pay() -> T {', 'capability pay() -> T { commits;')
    return source


WAIVE = '''model WaiveScope version "0.2" {
  type T = enum { ok }
  people [p,q]
  actor Officer { executors [p]; capability release() -> T { duration fixed 1min; result nondet { ok } } }
  actor B { capacity 1; capability run() -> T { duration fixed 1min; result nondet { ok } } }
  actor Wait { capacity 1; capability run() -> T { duration fixed 4min; result nondet { ok } } }
  authority G { basis "exception"; grants p to Officer.release; issued_by q }
  process Flow { o = Officer.release(); w = Wait.run(); end done }
  cut K { include [Officer,B,Wait] }
  configuration Old { cut K; process Flow }
  configuration New { cut K; process Flow }
  bounds { cases 1; live_tasks 1; clock 20min }
  verify reachable end.done
  contract Delivery { response delivery: case.received -> one_of(B.run.finish) by 2min after case.received otherwise unresolved }
  contract Exception { waive Delivery.delivery on Officer.release.finish reason "exception" authorized_by G }
}'''

CONTEST = '''model ContestScope version "0.2" {
  type T = enum { pass, fail }
  people [j,p]
  actor Judge { executors [j]; capability judge() -> T { duration fixed 1min; result nondet { fail } } }
  actor Route { executors [p]; capability respond() -> T { duration fixed 2min; result nondet { pass } } }
  actor Party { control party; standing over [end.returned] basis "claim" }
  authority G { basis "decision"; grants j to Judge.judge }
  process Flow { d = Judge.judge(); if d == fail { end returned } else { end posted } }
  cut K { include [Judge,Route,Party] }
  configuration Old { cut K; process Flow }
  configuration New { cut K; process Flow }
  bounds { cases 1; live_tasks 2; clock 10min }
  verify safety no_resource_overcommit
  contract Controls { contest Party on end.returned route Route.respond reversible_by [j] respond_by 2min }
}'''


def static_cases():
    yield 'unknown', scoped(plans='Missing'), 'E-NAME-0', 'contract_scope_unknown_configuration'
    yield 'empty', scoped(plans=''), 'E-CON-0', 'contract_scope_empty'
    yield 'duplicate', scoped(plans='New,New'), 'E-CON-0', 'contract_scope_duplicate'
    yield 'reason-missing', edit(scoped(), f' reason "{REASON}"', ''), 'E-SYN-0', 'contract_scope_reason_missing'
    yield 'anonymous', edit(scoped(), 'contract Controls applies_to', 'contract applies_to'), 'U-SYN-0', 'contract_scope_anonymous'
    no_config = edit(edit(edit(BASE, '  configuration Old { cut K; process AsIs }\n', ''),
                               '  configuration New { cut K; process ToBe }\n', ''),
                          '  process ToBe { a = Screen.approve(); p = Desk.pay(); end done }\n', '')
    yield 'no-configuration', scoped(no_config), 'E-CONF-0', 'contract_scope_no_configuration'
    yield 'contest', scoped(CONTEST), 'U-SEM-0', 'contract_scope_contest'
    yield 'waive-not-subset', scoped(WAIVE, 'Delivery'), 'E-CON-0', 'contract_scope_waive_not_subset'
    yield 'waive-disjoint', scoped(scoped(WAIVE, 'Delivery'), 'Exception', 'Old'), 'E-CON-0', 'contract_scope_waive_not_subset'


def probability_model(source):
    return add(source, 'verify probability reached end.done >= 1')


def evaluation_model(source):
    source = add(source, 'observed case { paid: bool = false }')
    source = edit(source, 'capability pay() -> T {',
                  'capability pay() -> T { writes paid; finish { paid = true };')
    return add(source, '''arrivals fixed [0min]
  property case.Paid = reached(paid == true)
  metric cohort.PaidPerArrival = expected(count(case.Paid)) / arrivals
  compare [Old,New] cut K hard { cohort.PaidPerArrival >= 1 }''')


def manifest(mode='exhaustive', configurations=('Old', 'Old'), models=None):
    return {'schema': 'relca-sensitivity/v1',
            'scope': {'kind': 'enumerated_models', 'rationale': 'hand scope contract comparison'},
            'points': [{'id': f'p{i}', 'model': (models or ['model.relca'] * len(configurations))[i],
                        'configuration': c, 'scenario': None} for i, c in enumerate(configurations)],
            'targets': [{'kind': 'probability_query', 'id': 'probability reached #1'}],
            'confidence': '1/2' if mode == 'simulation' else None}


def log(duplicate=False):
    header = {'kind': 'header', 'schema': 'relca-log/v1', 'clock': 'scope-clock', 'time_unit': 'min',
              'window': {'from': 0, 'to': 5},
              'cases': [{'case_id': 'c', 'from_case_start': True, 'through_case_end': True}],
              'actor_map': [{'source_id': 'desk', 'actor': 'Desk'}, {'source_id': 'screen', 'actor': 'Screen'}],
              'executor_map': [{'source_id': 'clerk', 'person': 'clerk'}, {'source_id': 'boss', 'person': 'boss'}]}
    rows = [header]
    for n in range(2 if duplicate else 1):
        for lifecycle, at in [('start', 2 * n), ('finish', 2 * n + 1)]:
            rows.append({'kind': 'event', 'event_id': f't{n}-{lifecycle}', 'case_id': 'c',
                         'task_id': f't{n}', 'operation': 'Desk.pay', 'lifecycle': lifecycle,
                         'cancel_phase': None, 'time': {'earliest': at, 'latest': at}, 'after': [],
                         'actor_id': 'desk', 'executor_id': 'clerk', 'input_version': 'v1',
                         'output_version': 'v2' if lifecycle == 'finish' else None,
                         'result': 'ok' if lifecycle == 'finish' else None})
    return ''.join(json.dumps(r) + '\n' for r in rows)


def conform_model():
    source = edit(BASE, CLAUSE, 'contract Controls { at_most_once Desk.pay.commit per case.id }')
    return add(edit(edit(source, 'capability pay() -> T {', 'capability pay() -> T { commits;'),
                    '  verify reachable end.done\n', ''), BASELINE)


def conform_evidence_model():
    return add(edit(evidence(), '  verify reachable end.done\n', ''), BASELINE)


def zero_requests():
    return edit(scoped(), '  verify reachable end.done\n', '')


def substrates():
    """Explicit old-language meanings, emitted for direct Rust parse/lower."""
    values = {'a1-old-excluded': (without_control(), 'Old'),
              'a1b-old-calls-excluded': (both_call(without_control()), 'Old'),
              'a2-new-applied': (BASE, 'New'), 'a2b-new-bad-grant': (bad_grant(), 'New'),
              'a3-dead-branch': (dead_branch(), 'New'), 'not-called': (unused(), 'New'),
              'a3-single-configuration': (only_new(dead_branch()), 'New'),
              'ordinal': (add(without_control(), 'contract Later { permitted Desk.pay }'), 'Old'),
              'waive': (WAIVE, 'New'), 'contest': (CONTEST, 'New'),
              'contest-probability': (add(CONTEST, 'verify probability reached end.returned >= 1'), 'Old'),
              'contest-simulation': (add(CONTEST, 'simulation replications 4 seed 7 confidence 0.5'), 'Old'),
              'evidence': (evidence(), 'New'), 'evidence-missing': (evidence(read=False), 'New'),
              'evidence-excluded': (without_control(), 'Old'),
              'conform': (conform_model(), 'New'),
              'conform-monitor-excluded': (edit(conform_model(), 'contract Controls { at_most_once Desk.pay.commit per case.id }', ''), 'Old'),
              'conform-evidence-excluded': (edit(conform_evidence_model(), 'contract Evidence { evidence_basis Screen.approve [Document] }', ''), 'Old'),
              'evaluation': (evaluation_model(without_control()), 'Old'),
              'simulation': (simulation(without_control()), 'Old'),
              'sensitivity': (probability_model(without_control()), 'Old')}
    for i, (_, body, _, _) in enumerate(clause_cases()):
        values[f'clause-{i}'] = (clause_model(body), 'New')
    return {name: {'source': s, 'configuration': c} for name, (s, c) in values.items()}


if __name__ == '__main__':
    print(json.dumps(substrates()))
