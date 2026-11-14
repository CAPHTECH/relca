"""Scope boundary controls, authored from declarations rather than saved output."""
import argparse
from pathlib import Path
import sys
import unittest

import cli as c
import inputs as h

CONTEST = '''model ContestRuntime version "0.2" {
  type T = enum { pass, fail }
  people [j,p]
  actor Judge { executors [j]; capability judge() -> T { duration fixed 1min; result nondet { fail } } }
  actor Route { executors [p]; capability respond() -> T { duration fixed 2min; result nondet { pass } } }
  actor Party { control party; standing over [end.returned] basis "claim" }
  permit j to Judge.judge
  permit p to Route.respond
  authority G { basis "decision"; grants j to Judge.judge }
  authority R { basis "route"; grants j to Route.respond }
  process Flow { d = Judge.judge(); if d == fail { end returned } else { end posted } }
  cut C { include [Judge,Route,Party] }
  configuration Old { cut C; process Flow }
  configuration New { cut C; process Flow }
  bounds { cases 1; live_tasks 2; clock 6min }
  verify safety no_resource_overcommit
  contract Controls { contest Party on end.returned route Route.respond reversible_by [j] respond_by 2min }
  contract RouteAuth applies_to [New] reason "route control belongs to New" { authority_basis Route.respond R }
}
'''


class Round1(unittest.TestCase):
    def test_recovery_handler_call(self):
        out, value = c.invoke(recovery_source('Screen.approve'), args=('--configuration', 'Old'))
        c.verdict(out, value, 'satisfied')
        c.scope(value, masks=True)
        c.diagnostic(value, 'W-CONF-2')

    def test_recovery_status_query(self):
        out, value = c.invoke(recovery_source('Psp.query_status'), args=('--configuration', 'Old'))
        c.verdict(out, value, 'satisfied')
        c.scope(value, masks=True)
        c.diagnostic(value, 'W-CONF-2')

    def test_merge_arms_are_calls_not_only_yield_aliases(self):
        body = 'p = Desk.pay(); merged = if p == ok { a = Screen.approve(); yield a } else { b = Screen.approve(); yield b }; end done'
        out, value = c.invoke(route_source(body), args=('--configuration', 'Old'))
        c.verdict(out, value, 'satisfied')
        c.scope(value, masks=True)
        c.diagnostic(value, 'W-CONF-2')

    def test_merge_evidence_scope_keeps_the_merge_control_boundary(self):
        source = h.evidence()
        source = h.edit(source, 'a = Screen.approve(); p = Desk.pay(); end done',
                        'p = Desk.pay(); merged = if p == ok { yield p } else { yield p }; a = Screen.approve(merged); end done')
        source = h.edit(source, 'capability approve() -> T', 'capability approve(input: T) -> T')
        source = h.scoped(source, 'Evidence')
        out, value = c.invoke(source, args=('--configuration', 'Old'))
        c.verdict(out, value, 'satisfied')
        c.scope(value, 'contract Evidence #1 evidence_basis', 'Evidence')
        out, value = c.invoke(source, args=('--configuration', 'New'))
        c.verdict(out, value, 'unsupported')
        c.diagnostic(value, 'U-SEM-0', 'merge_control')
        assert not any(r.get('verdict') == 'satisfied' for r in value['properties'] if r['id'] == 'contract Evidence #1 evidence_basis')
        # Keep the merge process identical, changing only which contract applies.
        same_process = h.edit(source, 'configuration Old { cut K; process AsIs }',
                              'configuration Old { cut K; process ToBe }')
        out, value = c.invoke(same_process, args=('--configuration', 'Old'))
        c.verdict(out, value, 'satisfied')
        c.scope(value, 'contract Evidence #1 evidence_basis', 'Evidence', masks=True)
        assert any('merge_control' in str(s) for s in value['agency']['unchecked'])
        out, value = c.invoke(same_process, args=('--configuration', 'New'))
        c.verdict(out, value, 'unsupported')
        c.diagnostic(value, 'U-SEM-0', 'merge_control')
    def test_contest_route_discloses_the_excluded_violation(self):
        # Judge ends at 1; a contest may run Route from 1 to 3 (deadline 3).
        # p executes Route, but the grant names j: applying RouteAuth violates it.
        out, value = c.invoke(CONTEST, args=('--configuration', 'Old'))
        c.verdict(out, value, 'satisfied')
        c.scope(value, 'contract RouteAuth #2 authority_basis', 'RouteAuth', reason='route control belongs to New', masks=True)
        c.diagnostic(value, 'W-CONF-2')
        unscoped = h.edit(CONTEST, ' applies_to [New] reason "route control belongs to New"', '')
        out, value = c.invoke(unscoped, args=('--configuration', 'Old'))
        c.verdict(out, value, 'violated')
        assert c.row(value, 'contract RouteAuth #2 authority_basis')['verdict'] == 'violated'

    def test_scope_change_can_have_its_own_intent(self):
        source = h.edit(h.scoped(), 'configuration Old { cut K; process AsIs }',
                        'configuration Old { cut K; process AsIs; intent { retire contract_scope Controls reason "control is absent" loses "approval requirement" } }')
        out, value = c.invoke(source, 'diff', ('--from', 'New', '--to', 'Old'))
        c.verdict(out, value, 'satisfied')
        row = next(r for r in value['differences'] if r['kind'] == 'contract_scope')
        assert row['change'] == 'removed' and row['reason'] == 'control is absent'
        assert not any(d['code'] == 'W-DIFF-1' and 'contract_scope Controls' in d['message'] for d in value['diagnostics'])

    def test_contract_scope_remains_a_legacy_actor_name_in_intent(self):
        for intent in ('retire contract_scope reason "old actor spelling" loses "retirement is only a claim"',
                       'maintain contract_scope by clerk reason "old actor spelling"'):
            source = h.BASE.replace('Desk', 'contract_scope')
            source = h.edit(source, 'configuration New { cut K; process ToBe }',
                            'configuration New { cut K; process ToBe; intent { ' + intent + ' } }')
            out, value = c.invoke(source, args=('--configuration', 'New'))
            c.verdict(out, value, 'satisfied')

    def test_scope_add_and_change_use_the_existing_intent_forms(self):
        for action, destination, origin, change in [('add', 'New', 'Old', 'added'), ('change', 'Old', 'New', 'removed')]:
            process = 'ToBe' if destination == 'New' else 'AsIs'
            source = h.edit(h.scoped(), 'configuration ' + destination + ' { cut K; process ' + process + ' }',
                            'configuration ' + destination + ' { cut K; process ' + process + '; intent { ' + action + ' contract_scope Controls reason "modeled scope change" } }')
            out, value = c.invoke(source, 'diff', ('--from', origin, '--to', destination))
            c.verdict(out, value, 'satisfied')
            row = next(r for r in value['differences'] if r['kind'] == 'contract_scope')
            assert row['change'] == change and row['reason'] == 'modeled scope change'
            assert not any(d['code'] == 'W-DIFF-1' and 'contract_scope Controls' in d['message'] for d in value['diagnostics'])

    def test_mutate_transports_scope_at_the_root(self):
        out, value = c.invoke(h.scoped(), 'mutate', ('--configuration', 'Old', '--kinds', 'authority_expired'))
        c.verdict(out, value, 'vacuous')
        assert value['contract_scope'] == value['original']['contract_scope']
        assert 'contract.scope.' + h.ID in value['unchecked']

    def test_cut_exclude_empty_reason_is_the_scope_precedent(self):
        source = h.add(h.scoped(), 'actor External { capacity 1 }')
        source = h.edit(source, 'include [Desk, Screen]', 'include [Desk, Screen]; exclude External reason "" loses "outside"')
        out, value = c.invoke(source, args=('--configuration', 'Old'))
        c.verdict(out, value, 'satisfied')
        source = h.edit(source, 'reason "' + h.REASON + '"', 'reason ""')
        out, value = c.invoke(source, args=('--configuration', 'Old'))
        c.verdict(out, value, 'satisfied')
        assert value['contract_scope'][0]['reason'] == ''


def route_source(body):
    source = h.edit(h.BASE, 'process AsIs { p = Desk.pay(); end done }', 'process AsIs { ' + body + ' }')
    source = source.replace('enum { ok }', 'enum { ok, bad }').replace('live_tasks 1', 'live_tasks 4').replace('clock 5min', 'clock 30min')
    return h.scoped(source)


def recovery_source(target):
    return '''model ScopeRecovery version "0.2" {
 type T = enum { ok }
 type Pay = enum { charged, duplicate }
 type Status = enum { committed_once, not_committed, still_unknown }
 people [clerk,boss]
 actor Psp { executors [clerk]; state keys: set<CaseId>
   capability charge(key: CaseId) -> Pay { idempotency key key; commits; writes keys; duration fixed 1min
     atomic { if key in keys { result duplicate } else { keys += key; result charged } }
     cancel forbidden_after_start }
   capability query_status(key: CaseId) -> Status { duration fixed 1min; receipt_query charge key key availability available; cancel forbidden_after_start }
 }
 actor Screen { executors [boss]; capability approve() -> T { duration fixed 1min; result nondet { ok } } }
 authority Mgr { basis "modeled"; grants clerk to Psp.query_status; grants boss to Screen.approve }
 process Flow { pay = Psp.charge(case.id) response_loss sequence [unknown] on receipt_unknown {
   status = Psp.query_status(case.id) for command pay
   if status == committed_once { a = Screen.approve(); resolve effect pay using status; end done } else { stop receipt_unknown }
 }; end done }
 cut K { include [Psp,Screen] }
 configuration Old { cut K; process Flow }
 configuration New { cut K; process Flow }
 bounds { cases 1; live_tasks 2; clock 20min }
 verify reachable end.done
 contract Controls applies_to [New] reason "''' + h.REASON + '''" { authority_basis ''' + target + ''' Mgr }
}
'''


PATHS = {
    'process': 'a = Screen.approve(); end done',
    'unreachable_if': 'p = Desk.pay(); if p == bad { a = Screen.approve() } else {}; end done',
    'after_end': 'end done; a = Screen.approve()',
    'fork_all': 'fork f { a = Screen.approve(); p = Desk.pay() } join all f; end done',
    'fork_any': 'fork f { a = Screen.approve(); p = Desk.pay() } join any f where result == ok tie by [a,p] remaining drain if none -> done; end done',
    'response': 'a = Screen.approve() response_loss none; end done',
    'retry': 'a = Screen.approve() response_loss none retry max_attempts 2 when result [bad] backoff fixed 0min exhausted end failed; end done',
    'repeat_body': 'repeat Review at_most 2 { a = Screen.approve(); leave } on_limit { end failed }; end done',
    'repeat_limit': 'repeat Review at_most 2 { p = Desk.pay(); again } on_limit { a = Screen.approve(); end done }; end done',
    'degradation_alternative': 'p = Desk.pay(); degrade Manual when p == bad { a = Screen.approve(); end done } else { end done }',
}


def path_test(body):
    def test(self):
        out, value = c.invoke(route_source(body), args=('--configuration', 'Old'))
        c.verdict(out, value, 'satisfied')
        c.scope(value, masks=True)
        c.diagnostic(value, 'W-CONF-2')
    return test


for name, body in PATHS.items():
    setattr(Round1, 'test_path_' + name, path_test(body))


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--binary', required=True)
    args, rest = parser.parse_known_args()
    c.BINARY = str(Path(args.binary).resolve())
    unittest.main(argv=[sys.argv[0], *rest], verbosity=2)
