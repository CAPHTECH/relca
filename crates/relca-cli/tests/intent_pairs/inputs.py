"""Hand-authored ADR 0073 inputs; no binary output is used to construct them."""
import json
import os
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[4]
REPLACE = 'replace Clerk.check with Bot.check reason "自動化" loses "人の目視"'


def model(default='', pairs='', partial=False, audit_reads=False, three=False):
    note = 'capability note() -> R { duration fixed 1min; result nondet { ok } }' if partial else ''
    audit = ('actor Audit { executors [boss]; capability log() -> R { '
             + ('reads [Doc]; ' if audit_reads else '')
             + 'duration fixed 1min; result nondet { ok } } }') if partial else ''
    robot = ('actor Robot { executors [boss]; capability check() -> R { reads [Doc]; '
             'duration fixed 1min; result nondet { ok } } }') if three else ''
    return '''model Swap version "0.2" {
  type R = enum { ok }
  people [clerk, op, boss]
  actor Doc { source document }
  actor Clerk { executors [clerk]
    capability check() -> R { reads [Doc]; duration fixed 2min; result nondet { ok } }
    NOTE
  }
  actor Bot { executors [op]
    capability check() -> R { reads [Doc]; duration fixed 1min; result nondet { ok } } }
  actor Desk { executors [clerk]
    capability post(x:R) -> R { duration fixed 1min; result nondet { ok } } }
  AUDIT
  ROBOT
  permit clerk to Clerk.check
  permit op to Bot.check
  process PA { PREFIX v = Clerk.check(); p = Desk.post(v); end done }
  process PB { PREFIX v = Bot.check(); p = Desk.post(v); SUFFIX end done }
  THIRD
  cut K { include [Doc, Clerk, Bot, DeskEXTRA] }
  configuration A { cut K; process PA }
  configuration B { cut K; process PBDEFAULT }
  CONFIG_C
  PAIRS
  bounds { cases 1; live_tasks 1; clock 10min }
  verify reachable end.done
}
'''.replace('NOTE', note).replace('AUDIT', audit).replace('ROBOT', robot).replace(
        'PREFIX', 'n = Clerk.note();' if partial else '').replace(
        'SUFFIX', 'a = Audit.log();' if partial else '').replace(
        'THIRD', 'process PC { v = Robot.check(); p = Desk.post(v); end done }' if three else '').replace(
        'EXTRA', (', Audit' if partial else '') + (', Robot' if three else '')).replace(
        'DEFAULT', '; intent { ' + default + ' }' if default else '').replace(
        'CONFIG_C', 'configuration C { cut K; process PC }' if three else '').replace('PAIRS', pairs)


def pair(body, from_='A', to='B'):
    return f'intent from {from_} to {to} {{ {body} }}'


def claims(old='Clerk', reason='自動化', loses=True, partial=False):
    loss = ' loses "人の目視"' if loses else ''
    lines = []
    if not partial:
        lines += [f'add Bot reason "{reason}"', f'retire {old} reason "{reason}"{loss}']
    lines += [f'add Bot.check reason "{reason}"', f'retire {old}.check reason "{reason}"{loss}',
              f'change authority Bot.check reason "{reason}"',
              f'change authority {old}.check reason "{reason}"',
              f'change resource Bot reason "{reason}"']
    if not partial:
        lines += [f'change resource {old} reason "{reason}"']
    lines += [f'change visibility Doc reason "{reason}"', f'change relation v reason "{reason}"']
    return '\n'.join(lines)


def extra(source, declarations):
    assert source.endswith('}\n')
    return source[:-2] + declarations + '\n}\n'


def deterministic(source):
    return extra(source.replace('result nondet { ok }', 'outcomes kernel Certain'),
                 'kernel Certain { * -> { 1: ok } }')


def command_model(command, default='', pairs=''):
    source = model(default, pairs, three=True)
    if command == 'simulate':
        source = extra(deterministic(source), 'simulation replications 4 seed 7 confidence 0.5')
    if command == 'sensitivity':
        source = deterministic(source).replace('verify reachable end.done', 'verify probability reached end.done >= 1')
    if command == 'conform':
        source = source.replace('verify reachable end.done', 'contract Once { at_most_once Desk.post.commit per case.id }')
        source = source.replace('capability post(x:R) -> R {', 'capability post(x:R) -> R { commits;')
    return source


def manifest():
    return {'schema': 'relca-sensitivity/v1',
            'scope': {'kind': 'enumerated_models', 'rationale': 'two identical hand points'},
            'points': [{'id': f'p{i}', 'model': 'model.relca', 'configuration': 'B', 'scenario': None}
                       for i in range(2)],
            'targets': [{'kind': 'probability_query', 'id': 'probability reached #1'}],
            'confidence': None}


def log():
    header = {'kind': 'header', 'schema': 'relca-log/v1', 'clock': 'intent-clock', 'time_unit': 'min',
              'window': {'from': 0, 'to': 3},
              'cases': [{'case_id': 'c', 'from_case_start': True, 'through_case_end': True}],
              'actor_map': [{'source_id': 'desk', 'actor': 'Desk'}],
              'executor_map': [{'source_id': 'clerk', 'person': 'clerk'}]}
    events = [header]
    for lifecycle, at in [('start', 1), ('finish', 2)]:
        events.append({'kind': 'event', 'event_id': lifecycle, 'case_id': 'c', 'task_id': 'post',
                       'operation': 'Desk.post', 'lifecycle': lifecycle, 'cancel_phase': None,
                       'time': {'earliest': at, 'latest': at}, 'after': [], 'actor_id': 'desk',
                       'executor_id': 'clerk', 'input_version': 'v1',
                       'output_version': 'v2' if lifecycle == 'finish' else None,
                       'result': 'ok' if lifecycle == 'finish' else None})
    return ''.join(json.dumps(e) + '\n' for e in events)


def unrelated(kind, default=''):
    source = model(default).replace('verify reachable end.done', 'verify safety no_resource_overcommit')
    if kind == 'contract_scope':
        return extra(source, 'contract Ctl applies_to [B] reason "new plan" { permitted Desk.post }')
    if kind == 'meaning':
        return source.replace('p = Desk.post(v);  end done', 'p = Desk.post(v); end other')
    if kind == 'time':
        return source.replace('v = Bot.check(); p = Desk.post(v);',
                              'v = Bot.check(); fork f { p = Desk.post(v) }; join all f;')
    if kind == 'contest':
        source = source.replace('p = Desk.post(v);  end done', 'p = Desk.post(v); end other')
        source = source.replace('include [Doc, Clerk, Bot, Desk]',
                                'include [Doc, Clerk, Bot, Desk, Vendor, Review]')
        return extra(source, '''actor Vendor { control party; standing over [end.other] basis "claim" }
actor Review { executors [clerk]; capability review() -> R { duration fixed 1min; result nondet { ok } } }
contract Appeal { contest Vendor on end.other route Review.review reversible_by [clerk] respond_by 5min }''')
    if kind == 'rule':
        source = source.replace('include [Doc, Clerk, Bot, Desk]', 'include [Doc, Clerk, Bot, Desk, Policy]')
        source = source.replace('process PB' + ('; intent' if default else ' }'),
                                'process PB; amend Policy from "r1" to "r2" by boss under ChangePolicy { required = false }'
                                + ('; intent' if default else ' }'))
        return extra(source, '''actor Policy { control policy; version "r1"; decides required: bool = true }
authority ChangePolicy { basis "amend"; grants boss to change Policy }''')
    if kind == 'qualification':
        return source.replace('capability check() -> R { reads [Doc]; duration fixed 2min',
                              'capability check() -> R { qualified_executors [clerk]; reads [Doc]; duration fixed 2min').replace(
            'capability check() -> R { reads [Doc]; duration fixed 1min',
            'capability check() -> R { qualified_executors [op]; reads [Doc]; duration fixed 1min')
    if kind == 'performance':
        source = source.replace('duration fixed 2min; result nondet { ok }', 'duration fixed 2min; outcomes performance Old')
        source = source.replace('reads [Doc]; duration fixed 1min; result nondet { ok }',
                                'reads [Doc]; duration fixed 1min; outcomes performance New')
        return extra(source, '''kernel Certain { * -> { 1: ok } }
performance Old basis "assumed" { load none; executor clerk { kernel Certain } }
performance New basis "assumed" { load none; executor op { kernel Certain } }
arrivals fixed [0min]
policy observable fifo tie_by case_id then call_order assign first_idle
metrics { window 10min; currency "JPY" }''')
    if kind == 'degradation':
        return re.sub(r'p = Desk\.post\(v\);\s+end done',
                      'degrade Manual when v == ok { end done } else { end done }', source)
    raise ValueError(kind)


def merged(default=''):
    return model(default, three=True).replace('v = Clerk.check();', 'z = Robot.check(); v = Clerk.check();')


def scoped_authority(default='', prime=False):
    source = extra(model(default), '''authority Grant { basis "approval"; grants op to Bot.check; issued_by boss }
contract Scoped applies_to [B] reason "new approval" { authority_basis Bot.check Grant }''')
    return source.replace('v = Clerk.check()', 'v = Bot.check()') if prime else source


def substrates():
    """Manual old-DSL expansions, including counterfactual from-prime inputs.

    Expansions establish geometry/hand counts, not replace's new annotation rule.
    Pair selection is represented by putting exactly the selected old items in B.
    """
    values = {}
    for name, partial, reads in [('a1', False, False), ('a2', True, False), ('a3', True, True)]:
        source = model(partial=partial, audit_reads=reads)
        values[name] = source
        values[name + '-claims'] = model(claims(partial=partial), partial=partial, audit_reads=reads)
        values[name + '-prime'] = source.replace('v = Clerk.check()', 'v = Bot.check()')
    values['a5-ab'] = model(claims(), three=True)
    values['a5-cb'] = model(claims('Robot', '移行'), three=True)
    values['a5-none'] = model(three=True)
    values['merged'] = merged()
    values['merged-prime'] = merged().replace('z = Robot.check();', 'z = Bot.check();').replace('v = Clerk.check();', 'v = Bot.check();')
    values['selected-preserve'] = extra(model('preserve Selected', three=True),
                                       'contract Default { permitted Desk.post }\ncontract Selected { permitted Desk.post }')
    values['destination-binding'] = model('change relation fresh reason "new binding"').replace(
        'v = Bot.check(); p = Desk.post(v)', 'fresh = Bot.check(); p = Desk.post(fresh)')
    for command in ['check', 'explain', 'simulate', 'mutate', 'sensitivity', 'conform', 'verify']:
        values['entry-' + command] = command_model(command, 'change visibility Doc reason "既定"')
    values['scoped-authority'] = scoped_authority()
    values['scoped-authority-prime'] = scoped_authority(prime=True)
    for kind in ['contract_scope', 'meaning', 'time', 'contest', 'rule', 'qualification', 'performance', 'degradation']:
        values['unrelated-' + kind] = unrelated(kind)
    return values


if __name__ == '__main__':
    values = ({'pairs': model(pairs=pair(claims())), 'replace': model(REPLACE)}
              if '--new-wire-inputs' in sys.argv else substrates())
    if os.environ.get('RELCA_INTENT_SUBSTRATE_FAULT'):
        values = {name: source.replace('duration fixed 2min', 'duration fixed -1min')
                  for name, source in values.items()}
    print(json.dumps(values, ensure_ascii=False))
