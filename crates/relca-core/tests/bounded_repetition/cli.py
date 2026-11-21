"""Real CLI acceptance, ADR 0066 stage 1. No expected failures are suppressed.

python3 -B cli.py --binary target/debug/relca [--expanded] [TestClass.test_name]
--expanded uses independent OLD DSL witnesses only for behavioral calibration;
new grammar and refusal tests always use the raw input, never an adapter.
"""
import argparse
from copy import deepcopy
from fractions import Fraction as F
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

import acceptance as hand
import expanded

DATA = Path(__file__).resolve().parent
ROOT = DATA.parents[3]
BINARY = ''
EXPANDED = False


def model(name):
    return expanded.source(name) if EXPANDED else (DATA / 'models' / (name + '.relca')).read_text()


def replace(source, before, after):
    assert source.count(before) == 1, (before, source.count(before))
    return source.replace(before, after)


def append(source, text):
    i = source.rfind('}')
    return source[:i] + '\n' + text + '\n' + source[i:]


def invoke(source, command='check', args=(), files=None):
    # All generated files stay in this worktree, even if TMPDIR points elsewhere.
    with tempfile.TemporaryDirectory(prefix='bounded-cli-', dir=ROOT / '.tmp') as d:
        root = Path(d)
        path = root / 'model.relca'
        path.write_text(source)
        for name, content in (files or {}).items():
            (root / name).write_text(content)
        tail = [str(root / x[1:]) if x.startswith('@') else x for x in args]
        out = subprocess.run([BINARY, command, str(path), *tail], capture_output=True, text=True, cwd=ROOT)
        assert out.stdout.count('\n') == 1, (command, out.returncode, out.stdout, out.stderr)
        return out, json.loads(out.stdout)


def verdict(out, value, expected):
    code = {'satisfied': 0, 'violated': 1, 'inconclusive': 2, 'invalid': 3, 'unsupported': 4}[expected]
    assert out.returncode == code, (expected, out.returncode, value.get('diagnostics'), out.stderr)
    assert value['verdict'] == expected, value


def refusal(source, code, reason='', command='check', args=(), files=None):
    out, value = invoke(source, command, args, files)
    verdict(out, value, 'invalid' if code.startswith('E-') else 'unsupported')
    matches = [d for d in value['diagnostics'] if d['code'] == code]
    assert matches, value['diagnostics']
    assert any(reason in d['message'] for d in matches), (reason, matches)
    assert code in out.stderr, out.stderr
    return value


class Behavior(unittest.TestCase):
    def test_hand_l1_l2_l3_l4_p1(self):
        for name in ('L1', 'L2', 'L3', 'L4', 'P1', 'L1-FFF'):
            with self.subTest(name=name):
                out, value = invoke(model(name))
                verdict(out, value, 'violated' if name == 'L1-FFF' else 'satisfied')
                hand.check_report(name, value)
                if name in hand.TERMINALS:
                    assert {t['end'] for t in value['terminals']} == set(hand.TERMINALS[name])

    def test_fixed_poll_paths(self):
        for name in ('P1-r1', 'P1-r2', 'P1-r3', 'P1-never'):
            with self.subTest(name=name):
                out, value = invoke(model(name))
                verdict(out, value, 'satisfied')
                hand.check_metrics(name, value['metrics'])

    def test_explain_first_start_and_replay(self):
        out, value = invoke(model('L1-FFF'), 'explain')
        verdict(out, value, 'violated')
        hand.check_explain(value)
        for trace in value['explanation']['counterexamples']:
            events = [e for s in trace['steps'] for e in s['events'] if e.get('kind') == 'start' and e.get('actor') == 'Rev']
            assert all(e['capability'] == 'review' for e in events), events

    def test_lineage_is_not_three_independent_sources(self):
        out, value = invoke(model('lineage'))
        verdict(out, value, 'satisfied')
        hand.check_lineage(value)

    def test_clock_is_not_on_limit(self):
        out, value = invoke(model('L2-clock'))
        verdict(out, value, 'inconclusive')
        hand.check_clock(value)
        assert {t['end'] for t in value['terminals']} == {'done'}
        assert value['terminals'][0]['elapsed'] == {'min': 1, 'max': 1}

    def test_poll_clock_is_not_business_limit(self):
        source = replace(model('P1'), 'clock 200min', 'clock 40min')
        # Query eventual shipped, not the already settled prefix time points.
        source = source[:source.index('verify probability')] + 'verify probability reached end.shipped >= 1\n}'
        out, value = invoke(source)
        verdict(out, value, 'inconclusive')
        assert value['bounds']['truncated'] is True
        p = next(p['probability'] for p in value['properties'] if p.get('probability'))
        assert F(p['min']) == F(5, 8) and F(p['max']) == 1, p
        assert not any(t['end'] == 'gave_up' for t in value['terminals'])

    def test_clock_keeps_known_done_mass(self):
        source = append(model('L2-clock'), 'verify probability reached end.done >= 0.9')
        out, value = invoke(source)
        verdict(out, value, 'inconclusive')
        values = [p['probability'] for p in value['properties'] if p.get('probability')]
        p = values[-1]
        assert F(p['min']) == F(9, 10) and F(p['max']) == 1, p

    def test_semantic_fault_pairs(self):
        # Same four single edits as carry-tests/faults.py, same assertions.
        for name in ('L2', 'L3', 'L4', 'P1'):
            with self.subTest(name=name):
                source = model(name)
                if name == 'L2':
                    source = replace(source, 'actor Clerk', 'kernel Reset { * -> { 0.1: pass with bad = true; 0.9: pass with bad = false } }\nactor Clerk')
                    source = replace(source, 'duration fixed 2min; result nondet { pass }', 'duration fixed 2min; outcomes kernel Reset; writes bad')
                if name == 'L3':
                    source = replace(source, '0.75: pass with bad = false; 0.25: pass with bad = true', '1: pass with bad = true')
                if name == 'L4':
                    # Existing DSL witness includes successor writes; replace
                    # the probabilities only, not those state transitions.
                    source = source.replace('0.2: fail', '0.5: fail').replace('0.8: pass', '0.5: pass').replace('0.1: fail', '0.5: fail').replace('0.9: pass', '0.5: pass')
                if name == 'P1':
                    source = replace(source, 'end gave_up', 'wait 30min; end gave_up')
                out, value = invoke(source)
                verdict(out, value, 'violated')
                values = [p['probability'] for p in value['properties'] if p.get('probability') is not None]
                index = 0 if name == 'L2' else -1
                wrong = {'L2': F(1, 1000), 'L3': F(1, 10), 'L4': F(1, 8), 'P1': F(0)}[name]
                assert F(values[index]['min']) == F(values[index]['max']) == wrong
                if name == 'P1':
                    t = next(t for t in value['terminals'] if t['end'] == 'gave_up')
                    assert t['elapsed'] == {'min': 93, 'max': 93}
                with self.assertRaises(AssertionError):
                    hand.probabilities(values, hand.PROBABILITIES[name])


BASE = '''model Grammar version "0.2" {
type V = enum { pass, fail }
actor A { capacity 1; capability c() -> V { duration fixed 1min; result nondet { pass } } }
process P { repeat rework at_most 3 { r = A.c(); again } on_limit { end limit } end done }
cut C { include [A] }
bounds { cases 1; live_tasks 2; clock 20min }
verify safety no_resource_overcommit
}'''


class Grammar(unittest.TestCase):
    def test_required_limit_and_exit(self):
        for before, after in [('at_most 3 ', ''), ('on_limit { end limit } ', '')]:
            with self.subTest(before=before):
                refusal(replace(BASE, before, after), 'E-SYN-0')

    def test_fallthrough(self):
        refusal(replace(BASE, 'r = A.c(); again', 'r = A.c()'), 'E-CON-0', 'loop_fallthrough')

    def test_one_branch_fallthrough(self):
        refusal(replace(BASE, 'r = A.c(); again', 'r = A.c(); if r == pass { leave } else { wait 1min }'), 'E-CON-0', 'loop_fallthrough')

    def test_zero_time_iteration(self):
        for body in ('wait 0min; again', 'again', 'if rework.round == 2 { wait 0min; again } else { wait 1min; again }'):
            with self.subTest(body=body):
                refusal(replace(BASE, 'r = A.c(); again', body), 'E-CON-0', 'zero_time_iteration')

    def test_zero_duration_requires_refusal(self):
        source = replace(BASE, 'pass, fail', 'pass, fail, denied')
        source = append(source, 'observed case { flag: bool = false }')
        source = replace(source, 'duration fixed 1min', 'requires flag == true else denied; duration fixed 1min')
        refusal(source, 'E-CON-0', 'zero_time_iteration')

    def test_limit_zero(self):
        refusal(replace(BASE, 'at_most 3', 'at_most 0'), 'E-CON-0')

    def test_limit_thirty_three(self):
        refusal(replace(BASE, 'at_most 3', 'at_most 33'), 'U-SEM-0', 'loop_expansion_limit')

    def test_limit_one_counts_first_body(self):
        out, value = invoke(replace(BASE, 'at_most 3', 'at_most 1'))
        verdict(out, value, 'satisfied')
        assert value['terminals'] == [{'end': 'limit', 'elapsed': {'min': 1, 'max': 1}, 'work': {'min': 1, 'max': 1}, 'result_at': None}]

    def test_round_condition(self):
        source = replace(BASE, 'r = A.c(); again', 'r = A.c(); if rework.round == 2 { leave } else { again }')
        source = append(source, 'verify reachable end.done')
        out, value = invoke(source)
        verdict(out, value, 'satisfied')
        assert [(t['end'], t['elapsed']) for t in value['terminals']] == [('done', {'min': 2, 'max': 2})]

    def test_round_given_rows_are_exhaustive(self):
        source = (DATA / 'models/L4.relca').read_text()
        refusal(replace(source, '; 3 -> { 0.1: fail; 0.9: pass }', ''), 'E-PROB-0')

    def test_poll_given_rows_are_exhaustive(self):
        source = (DATA / 'models/P1.relca').read_text()
        refusal(replace(source, '; never, 3 -> { 1: not_yet }', ''), 'E-PROB-0')

    def test_latent_is_not_a_policy_input(self):
        for name, before, after in [('L2', 'if r == pass', 'if bad == true'), ('P1', 'if c == arrived', 'if ready == r1')]:
            with self.subTest(name=name):
                refusal(replace((DATA / 'models' / (name + '.relca')).read_text(), before, after), 'E-POL-LATENT')

    def test_name_collision(self):
        for name in ('A', 'r', 'done'):
            with self.subTest(name=name):
                refusal(replace(BASE, 'repeat rework', 'repeat ' + name), 'E-NAME-0')

    def test_limit_block_cannot_again(self):
        refusal(replace(BASE, 'on_limit { end limit }', 'on_limit { again }'), 'E-CON-0', 'loop_again_in_limit')

    def test_binding_latest_after_leave(self):
        source = replace(BASE, 'r = A.c(); again', 'r = A.c(); if rework.round == 2 { leave } else { again }')
        source = replace(source, 'end done }', 'if r == pass { end done } else { end wrong } }')
        source = replace(source, 'result nondet { pass }', 'outcomes kernel K')
        source = append(source, 'kernel K given [rework.round] { 1 -> { 1: fail }; 2 -> { 1: pass }; 3 -> { 1: fail } }')
        out, value = invoke(append(source, 'verify reachable end.done'))
        verdict(out, value, 'satisfied')
        assert value['terminals'][0]['elapsed'] == {'min': 2, 'max': 2}

    def test_binding_not_definite_on_all_leave_paths(self):
        source = replace(BASE, 'r = A.c(); again', 'if rework.round == 1 { leave } else { r = A.c(); leave }')
        source = replace(source, 'end done }', 'if r == pass { end done } else { end wrong } }')
        refusal(source, 'U-SEM-0', 'result is not definitely available (not yet defined or possibly cancelled)')

    def test_forbidden_body_combinations(self):
        bodies = {
            'nested': 'repeat inner at_most 2 { x = A.c(); again } on_limit { leave }; again',
            'response_loss': 'r = A.c() response_loss sequence [delivered]; again',
            'retry': 'r = A.c() response_loss none retry max_attempts 2 when result [fail] backoff fixed 1min exhausted end limit; again',
            'receipt_handler': 'r = A.c() response_loss sequence [unknown] on receipt_unknown { stop receipt_unknown }; again',
            'degrade': 'r = A.c(); degrade Manual when r == pass { wait 1min; again } else { wait 1min; leave }',
        }
        for name, body in bodies.items():
            with self.subTest(name=name):
                refusal(replace(BASE, 'r = A.c(); again', body), 'U-SEM-0', 'loop_feature_combination')

    def test_repeat_inside_recovery_is_closed(self):
        source = replace(BASE,
                         'repeat rework at_most 3 { r = A.c(); again } on_limit { end limit }',
                         'v = A.c() response_loss sequence [unknown] on receipt_unknown { repeat rework at_most 3 { r = A.c(); again } on_limit { end limit } }')
        refusal(source, 'U-SEM-0', 'loop_feature_combination')

    def test_limit_block_must_close_all_paths(self):
        refusal(replace(BASE, 'on_limit { end limit }', 'on_limit { x = A.c() }'), 'E-CON-0', 'loop_fallthrough')

    def test_limit_leave_returns_to_suffix(self):
        source = replace(BASE, 'on_limit { end limit }', 'on_limit { leave }')
        out, value = invoke(source)
        verdict(out, value, 'satisfied')
        assert [(t['end'], t['elapsed'], t['work']) for t in value['terminals']] == [('done', {'min': 3, 'max': 3}, {'min': 3, 'max': 3})]

    def test_end_terminates_case_without_limit_or_suffix(self):
        source = replace(BASE, 'r = A.c(); again', 'r = A.c(); end early')
        out, value = invoke(source)
        verdict(out, value, 'satisfied')
        assert [(t['end'], t['elapsed']) for t in value['terminals']] == [('early', {'min': 1, 'max': 1})]

    def test_round_unread_public_observations(self):
        # Accepted replacement for the rejected private equality/hash experiment.
        # A tautological read materializes an observable round slot, but must
        # not change the otherwise round-unobserving model's behavior.
        source = (DATA / 'models/L1-32.relca').read_text()
        other = replace(source, 'r = Rev.review()', 'if rework.round >= 1 { wait 0min } else { wait 0min }; r = Rev.review()')
        aout, a = invoke(source)
        bout, b = invoke(other)
        verdict(aout, a, 'satisfied')
        verdict(bout, b, 'satisfied')
        for key in ('terminals', 'properties', 'bounds'):
            assert a[key] == b[key], key
        assert a['metrics'] == b['metrics']


class Simulation(unittest.TestCase):
    def test_deterministic_poll_keeps_sample_safety_inconclusive(self):
        source = append(model('P1-r3'), 'simulation replications 4 seed 7 confidence 0.5')
        out, value = invoke(source, 'simulate')
        verdict(out, value, 'inconclusive')
        safety = next(p for p in value['properties'] if p['id'] == 'safety no_resource_overcommit')
        assert safety['verdict'] == 'inconclusive'
        case = value['case_outcomes'][0]
        assert case['unfinished_count'] == 0
        assert case['terminals'] == [{'kind': 'end', 'end': 'shipped', 'count': 4}]

    def test_unresolved_nondeterminism_stays_unsupported(self):
        refusal(append(model('L1'), 'simulation replications 4 seed 7 confidence 0.5'), 'U-SEM-0', command='simulate')


class Compare(unittest.TestCase):
    def compare(self, plans, source=None, expected='satisfied', mode='exhaustive'):
        out, value = invoke(source or model('C1'), 'compare', ['--plans', plans, '--mode', mode])
        verdict(out, value, expected)
        return value

    def test_round_rows_and_unrolled_equivalence(self):
        value = self.compare('Loop3,Loop2')
        tables = value['tables']
        pair = tables['terminals']
        for end, p, times in [('done', '9/10', (1, 1)), ('escalated', '1/10', (9, 6))]:
            row, = (r for r in pair if r['end'] == end)
            for cell, t in zip(row['values'], times, strict=True):
                self.assertEqual(cell['status'], 'reported')
                self.assertEqual(cell['value']['probability'], {'min': p, 'max': p})
                self.assertEqual(cell['value']['completed_at'], {'min': t, 'max': t})
        prop, = tables['properties']
        self.assertEqual(prop['id'], 'probability reached #1')
        for cell in prop['values']:
            self.assertEqual(cell['verdict'], 'satisfied')
            self.assertEqual(cell['probability'], {'min': '1/10', 'max': '1/10'})

        rows = {(r['key']['scope'], r['key']['subject'], r['key']['metric']): r['values']
                for r in tables['metrics']}
        work = rows['overall', None, 'work']
        for cell, max_work in zip(work, (9, 6), strict=True):
            self.assertEqual(cell['status'], 'reported')
            self.assertEqual((F(cell['value']['min']), F(cell['value']['max'])), (1, max_work))
        for site in ('r', 'f'):
            for k in (1, 2, 3):
                cells = rows['task', f'loop:{site}:round{k}', 'starts']
                for plan, cell in enumerate(cells):
                    if plan == 1 and k == 3:
                        self.assertEqual((cell['status'], cell['value']), ('not_applicable', None))
                    else:
                        self.assertEqual(cell['status'], 'reported')
                        self.assertEqual((F(cell['value']['min']), F(cell['value']['max'])),
                                         (int(site == 'r' and k == 1), 1))

        self.assertIn('repetition_tables', value['checked'])
        self.assertEqual([(r['case_id'], r['loop']) for r in tables['repetition']],
                         [(1, 'rework2'), (1, 'rework3')])
        for row, n in zip(tables['repetition'], (2, 3), strict=True):
            for plan, cell in zip((3, 2), row['values'], strict=True):
                self.assertEqual(cell['configuration'], f'Loop{plan}')
                if plan != n:
                    self.assertEqual((cell['status'], cell['value']), ('not_applicable', None))
                    continue
                self.assertEqual(cell['status'], 'reported')
                self.assertEqual(cell['value'], {
                    'rounds': {'min': 1, 'max': n}, 'on_limit': {'min': 0, 'max': 1},
                    'on_limit_probability': {'min': '1/10', 'max': '1/10'},
                    'samples': None, 'excluded_samples': None, 'observed_on_limit': None,
                    'distribution': [dict(rounds=k, probability={'min': p, 'max': p}, observed_count=None)
                                     for k in range(n + 1)
                                     for p in ['9/10' if k == 1 else '1/10' if k == n else '0']],
                })

        hand_tables = self.compare('Hand3,Hand2')['tables']
        for rows in hand_tables.values():
            for row in rows:
                for cell in row['values']:
                    cell['configuration'] = cell['configuration'].replace('Hand', 'Loop')
                if row.get('key', {}).get('scope') == 'task':
                    site, k = row['key']['subject']
                    row['key']['subject'] = f'loop:{site}:round{k}'
        self.assertEqual({k: v for k, v in tables.items() if k != 'repetition'}, hand_tables)

    def test_mixed_pair_does_not_match_tasks(self):
        for plans in ('Loop3,Hand3', 'Hand3,Loop3'):
            with self.subTest(plans=plans):
                tables = self.compare(plans)['tables']
                for row in tables['terminals'] + tables['properties'] + [
                    r for r in tables['metrics'] if r['key']['scope'] == 'overall'
                ]:
                    left, right = [{k: v for k, v in c.items() if k != 'configuration'} for c in row['values']]
                    self.assertEqual(left, right)
                for subject, plan in [('loop:r:round1', 'Loop3'), ('r1', 'Hand3')]:
                    row, = (r for r in tables['metrics'] if r['key']['scope'] == 'task'
                            and r['key']['metric'] == 'starts' and r['key']['subject'] == subject)
                    for c in row['values']:
                        self.assertEqual(c['status'], 'reported' if c['configuration'] == plan else 'not_applicable')
                row, = tables['repetition']
                self.assertEqual((row['case_id'], row['loop']), (1, 'rework3'))
                for c in row['values']:
                    self.assertEqual(c['status'], 'reported' if c['configuration'] == 'Loop3' else 'not_applicable')

    def test_repeat_with_evidence_basis_keeps_the_contract_rows(self):
        source = append(Diff.paired_lineage(), 'contract Basis { evidence_basis Ledger.post [Invoice] }')
        out, report = invoke(source, args=['--configuration', 'Loop'])
        verdict(out, report, 'satisfied')
        clause = 'contract Basis #1 evidence_basis'
        row, = (p for p in report['properties'] if p['id'] == clause)
        self.assertEqual(row['verdict'], 'satisfied')
        compared = self.compare('Loop,Hand', source, 'satisfied')
        row, = (p for p in compared['tables']['properties'] if p['id'] == clause)
        self.assertEqual([side['verdict'] for side in row['values']], ['satisfied', 'satisfied'])
        out, value = invoke(source, 'diff', ['--from', 'Loop', '--to', 'Hand'])
        verdict(out, value, 'satisfied')
        row, = (p for p in value['properties'] if p['id'] == clause)
        self.assertEqual((row['from'], row['to']), ('satisfied', 'satisfied'))

    def test_cut_does_not_count_as_on_limit(self):
        source = replace(model('C1'), 'clock 30min', 'clock 7min')
        # The uncut done mass leaves at most 1/10 for escalated. A 1/20
        # bound cannot be settled using that upper bound alone.
        source = replace(source, 'end.escalated <= 0.2', 'end.escalated <= 0.05')
        value = self.compare('Loop3,Hand3', source, 'inconclusive')
        row, = value['tables']['repetition']
        self.assertEqual([(c['status'], c['value']) for c in row['values']],
                         [('not_reported', None), ('not_applicable', None)])
        self.assertNotIn('repetition_tables', value['checked'])
        self.assertIn('repetition_tables', value['unchecked'])
        self.assertEqual(row['values'][0]['reason'], '/plans/0/report/repetition/unchecked')

    def test_simulation_transports_samples_and_names_limit_tasks(self):
        source = replace(model('C1'), 'true: 0.1, false: 0.9', 'true: 1, false: 0')
        source = replace(source, 'on_limit { end escalated }\n    end done\n  }\n  process Looped2',
                         'on_limit { last = Clerk.fix(); end escalated }\n    end done\n  }\n  process Looped2')
        source = append(replace(source, '<= 0.2', '<= 1'), 'simulation replications 4 seed 7 confidence 0.5')
        value = self.compare('Loop3,Loop2', source, mode='simulation')
        row = value['tables']['repetition'][1]['values'][0]['value']
        self.assertEqual((row['samples'], row['excluded_samples'], row['observed_on_limit']), (4, 0, 4))
        self.assertIsNone(row['on_limit_probability'])
        starts, = (r for r in value['tables']['metrics'] if r['key']['subject'] == 'loop:last:on_limit'
                   and r['key']['metric'] == 'starts')
        self.assertEqual([c['status'] for c in starts['values']], ['reported', 'not_applicable'])


def sensitivity(points, query=4, configuration=None):
    files = {f'{name}.relca': source for name, source in points}
    manifest = {'schema': 'relca-sensitivity/v1',
                'scope': {'kind': 'enumerated_models', 'rationale': 'finite hand-calculated points'},
                'points': [{'id': name, 'model': f'{name}.relca', 'configuration': configuration,
                            'scenario': None} for name, _ in points],
                'targets': [{'kind': 'probability_query', 'id': f'probability reached #{query}'}],
                'confidence': None}
    return invoke(json.dumps(manifest), 'sensitivity', ['--mode', 'exhaustive'], files)


class Sensitivity(unittest.TestCase):
    def test_round_kernel_rows_are_inputs(self):
        half = model('L4')
        quarter = replace(half, '0.5: fail; 0.5: pass', '0.25: fail; 0.75: pass')
        hand = (DATA / 'witnesses/L4.relca').read_text()
        hand_quarter = replace(hand, '0.5: fail with round = v2; 0.5: pass',
                               '0.25: fail with round = v2; 0.75: pass')
        for points, loop in [([('half', hand), ('quarter', hand_quarter)], False),
                             ([('half', half), ('quarter', quarter)], True)]:
            out, value = sensitivity(points)
            verdict(out, value, 'violated')
            row, = value['summaries']
            self.assertEqual(row['id'], 'probability reached #4')
            self.assertEqual(row['kind'], 'finite_point_extrema')
            self.assertEqual(row['value'], {'min': '1/200', 'max': '1/100'})
            self.assertEqual((row['min_point_ids'], row['max_point_ids']), (['quarter'], ['half']))
            for point, p in zip(value['points'], ('1/100', '1/200'), strict=True):
                prop, = (r for r in point['report']['properties'] if r['id'] == row['id'])
                self.assertEqual(prop['probability'], {'min': p, 'max': p})
                if loop:
                    eff = point['effective_inputs']
                    self.assertFalse(any(row['variable'].startswith('loop.') for row in eff['initials']))
                    review, = (op for op in eff['capabilities'] if op['capability'] == 'Rev.review')
                    self.assertEqual(review['kernel']['given'], ['loop.rework.round'])
                    self.assertEqual([r['conditions'] for r in review['kernel']['rows']],
                                     [[{'variable': 'loop.rework.round', 'value': k}] for k in (1, 2, 3)])

    def test_at_most_and_gap_change_the_question(self):
        half = model('L4')
        quarter = replace(half, '0.5: fail; 0.5: pass', '0.25: fail; 0.75: pass')
        short = replace(replace(quarter, 'at_most 3', 'at_most 2'), '; 3 -> { 0.1: fail; 0.9: pass }', '')
        gap = replace(quarter, 'at_most 3', 'at_most 3 gap fixed 5min')
        hand = (DATA / 'witnesses/L4.relca').read_text()
        hand_short = replace(hand, 'r3 = Rev.review(); if r3 == pass { end done } else { f3 = Clerk.fix(); end escalated }',
                             'end escalated')
        for base, broken in [(hand, hand_short), (half, short), (half, gap)]:
            with self.subTest(loop=base == half, gap=broken == gap):
                out, value = sensitivity([('half', base), ('quarter', broken)])
                verdict(out, value, 'invalid')
                self.assertTrue(any(d['code'] == 'E-CMP-0' and 'sensitivity_contract_mismatch' in d['message']
                                    and '/process' in d['message'] for d in value['diagnostics']), value['diagnostics'])


class ContractCommands(unittest.TestCase):
    R0 = (DATA.parent / 'record_provenance/r0.relca').read_text()
    K2 = append(R0, '  process NoRegister {' + replace(R0.split('  process Main {')[1].split('  bounds')[0],
                '    r = Clerk.register()\n', '') +
                '  configuration A { cut S; process Main }\n  configuration B { cut S; process NoRegister }')

    def compare_or_diff(self, source, cmd):
        args = ['--plans', 'A,B', '--mode', 'exhaustive'] if cmd == 'compare' else ['--from', 'A', '--to', 'B']
        out, value = invoke(source, cmd, args)
        verdict(out, value, 'violated')
        self.assertIn('contract_witnesses', value['unchecked'])
        return value

    def property_rows(self, value, cmd):
        if cmd == 'compare':
            return {r['id']: tuple(c['verdict'] for c in r['values']) for r in value['tables']['properties']}
        return {r['id']: (r['from'], r['to']) for r in value['properties']}

    def test_k1_reports_contract_verdicts_not_call_witnesses(self):
        for cmd in ('compare', 'diff'):
            with self.subTest(cmd=cmd):
                value = self.compare_or_diff(model('K1'), cmd)
                self.assertEqual(self.property_rows(value, cmd), {
                    'reachable end.posted': ('satisfied', 'satisfied'),
                    'contract Basis #1 evidence_basis': ('violated', 'satisfied'),
                    'contract Version #2 same_version': ('satisfied', 'violated'),
                })
                if cmd == 'diff':
                    self.assertEqual(value['differences'], [])
                    self.assertFalse(any(r['item'] == 'via_records' for r in value['agency']['differences']['rows']))

    def test_k2_registration_and_via_records_are_not_silently_omitted(self):
        for cmd in ('compare', 'diff'):
            with self.subTest(cmd=cmd):
                value = self.compare_or_diff(self.K2, cmd)
                self.assertEqual(self.property_rows(value, cmd)['contract Basis #1 evidence_basis'],
                                 ('satisfied', 'violated'))
                if cmd == 'diff':
                    self.assertEqual({(r['kind'], r['target'], r['change']) for r in value['differences']}, {
                        ('relation', 'Clerk.register', 'removed'), ('authority', 'Clerk.register', 'removed'),
                        ('record', 'Notes', 'modified'),
                    })
                    row, = (r for r in value['differences'] if r['kind'] == 'record')
                    self.assertEqual(row['from'], {'registrars': ['Clerk.register'], 'readers': ['Reviewer.review']})
                    self.assertEqual(row['to'], {'registrars': [], 'readers': ['Reviewer.review']})
                    row, = (r for r in value['agency']['differences']['rows'] if r['viewpoint'] == 'origin'
                            and r['actor'] == 'Reviewer' and r['item'] == 'via_records')
                    self.assertEqual(row['to'], [])
                    origin, = row['from']
                    self.assertEqual(origin, {'record': 'Notes', 'registrars': [{'call': 'r', 'people': ['p1']}],
                                              'sources': [{'source': 'Know', 'route': 'registrar_direct'}]})

    def assert_extrema(self, points, expected, low, high, configuration=None):
        out, value = sensitivity(points, 1, configuration)
        verdict(out, value, expected)
        row, = value['summaries']
        self.assertEqual(row['kind'], 'finite_point_extrema')
        self.assertEqual(row['value'], {'min': low, 'max': high})

    def assert_question_changed(self, points, location, configuration=None):
        out, value = sensitivity(points, 1, configuration)
        verdict(out, value, 'invalid')
        self.assertTrue(any(d['code'] == 'E-CMP-0' and 'sensitivity_contract_mismatch' in d['message']
                            and location in d['message'] for d in value['diagnostics']), value['diagnostics'])

    def test_k1_sensitivity(self):
        p80 = replace(model('K1'), 'result nondet { pass, fail }', 'outcomes kernel M')
        p80 = append(p80, 'kernel M { * -> { 0.8: pass; 0.2: fail } }')
        p80 = replace(p80, 'verify reachable end.posted', 'verify probability reached end.posted >= 0.5')
        p60 = replace(p80, '0.8: pass; 0.2: fail', '0.6: pass; 0.4: fail')
        changed = replace(p80, 'evidence_basis Approver.approve [PurchaseOrder]', 'evidence_basis Approver.approve [Invoice]')
        self.assert_extrema([('p80', p80), ('p60', p60)], 'violated', '3/5', '4/5', 'B')
        self.assert_question_changed([('p80', p80), ('c', changed)], '/contracts', 'B')
        direct = replace(p80, 'evidence_basis Approver.approve', 'evidence_basis direct Approver.approve')
        self.assert_question_changed([('p80', p80), ('direct', direct)], '/contracts', 'B')
        version = replace(p80, 'same_version Clerk.match_po(e), Approver.approve(e)',
                          'same_version Approver.approve(e), Clerk.match_po(e)')
        self.assert_question_changed([('p80', p80), ('version', version)], '/contracts', 'B')

    def test_registration_sensitivity(self):
        a = replace(replace(self.R0, 'reads [Know]; duration fixed 1min; result nondet { pass, fail }',
                            'reads [Know]; duration fixed 1min; outcomes kernel J'),
                    'reads [Notes]; duration fixed 1min; result nondet { pass, fail }',
                    'reads [Notes]; duration fixed 1min; outcomes kernel J')
        a = append(a, 'kernel J { * -> { 0.9: pass; 0.1: fail } }')
        a = replace(a, 'verify reachable end.posted', 'verify probability reached end.posted >= 0.5')
        b = replace(a, '0.9: pass; 0.1: fail', '0.8: pass; 0.2: fail')
        c = replace(a, '; registers [Notes]', '')
        c = replace(c, 'reads [Invoice];', 'reads [Invoice]; registers [Notes];')
        self.assert_extrema([('r-a', a), ('r-b', b)], 'satisfied', '16/25', '81/100')
        self.assert_question_changed([('r-a', a), ('r-c', c)], '/capabilities')
        reader = replace(a, 'reads [Notes];', '')
        self.assert_question_changed([('r-a', a), ('reader', reader)], '/capabilities')

    def test_compare_names_the_uncompared_record_difference_without_contracts(self):
        source = replace(self.K2, '  contract Basis { evidence_basis Reviewer.review [Know] }\n', '')
        out, value = invoke(source, 'compare', ['--plans', 'A,B', '--mode', 'exhaustive'])
        verdict(out, value, 'satisfied')
        self.assertEqual(value['unchecked'].count('record_differences'), 1)
        self.assertNotIn('contract_witnesses', value['unchecked'])


class Diff(unittest.TestCase):
    @staticmethod
    def paired_lineage():
        loop = replace(model('lineage'), 'post(e: Entry)', 'post(e: V)')
        loop = replace(loop, 'Ledger.post(entry)', 'Ledger.post(r)')
        hand = (DATA / 'witnesses/lineage.relca').read_text()
        for k in (1, 2, 3):
            hand = replace(hand, f'p{k} = Ledger.post(entry)', f'p{k} = Ledger.post(r{k})')
        body = hand[hand.index('process P {'):hand.index('\ncut C')].replace('process P {', 'process Handwritten {')
        return append(loop, body + '\nconfiguration Hand { cut C; process Handwritten }\nconfiguration Loop { cut C; process P }')

    def test_repetition_and_alias_producer(self):
        source = self.paired_lineage()
        out, check = invoke(source, args=['--configuration', 'Hand'])
        verdict(out, check, 'satisfied')
        out, check_loop = invoke(source, args=['--configuration', 'Loop'])
        verdict(out, check_loop, 'satisfied')
        self.assertEqual(sorted(check['terminals'], key=lambda t: t['end']),
                         sorted(check_loop['terminals'], key=lambda t: t['end']))
        for a, b, change in [('Hand', 'Loop', 'added'), ('Loop', 'Hand', 'removed')]:
            with self.subTest(a=a, b=b):
                out, value = invoke(source, 'diff', ['--from', a, '--to', b])
                verdict(out, value, 'satisfied')
                rows = value['differences']
                self.assertFalse(any(r['kind'] in ('authority', 'resource', 'visibility', 'time') for r in rows))
                rep, = (r for r in rows if r['kind'] == 'repetition')
                self.assertEqual((rep['target'], rep['change']), ('rework', change))
                loop_side = 'to' if b == 'Loop' else 'from'
                self.assertEqual(rep[loop_side], {'at_most': 3, 'gap': 0, 'sites': ['r', 'f'],
                                                 'on_limit_ends': ['escalated']})
                rel, = (r for r in rows if r['kind'] == 'relation' and r['target'] == 'r')
                self.assertEqual(rel[loop_side], {'producer': 'Rev.review'})
                self.assertFalse(any(r['target'].startswith('@loop:') for r in rows))

    def test_fold_repeated_producers_across_plans(self):
        source = model('lineage')
        changed = source[source.index('  process P {'):source.index('  cut C')]
        changed = changed.replace('process P', 'process Alternative').replace('repeat rework', 'repeat next')
        changed = changed.replace('Rev.review(entry)', 'Rev.other(entry)')
        source = replace(source, 'capability review(e: Entry)',
                         'capability other(e: Entry) -> V { reads [e]; duration fixed 1min; result nondet { pass, fail } }; capability review(e: Entry)')
        source = append(source, changed + 'configuration A { cut C; process P }\nconfiguration B { cut C; process Alternative }')
        out, value = invoke(source, 'diff', ['--from', 'A', '--to', 'B'])
        verdict(out, value, 'satisfied')
        rows = [r for r in value['differences'] if r['kind'] == 'relation' and r['change'] == 'modified']
        row, = rows
        self.assertEqual((row['target'], row['from'], row['to']),
                         ('r', {'producer': 'Rev.review'}, {'producer': 'Rev.other'}))


class RepeatedContracts(unittest.TestCase):
    def test_missing_alias_source_remains_visible_across_commands(self):
        source = replace(model('eb-alias-e2'), 'capability patch() -> V { reads [Doc];',
                         'capability patch() -> V {')
        source = append(source, 'simulation replications 64 seed 7 confidence 0.9\nverify probability reached end.escalated <= 1')
        clause = 'contract Basis #1 evidence_basis'

        def row(report):
            prop, = (p for p in report['properties'] if p['id'] == clause)
            return prop

        out, value = invoke(source, 'compare', ['--plans', 'Loop,Hand', '--mode', 'exhaustive'])
        verdict(out, value, 'violated')
        prop, = (p for p in value['tables']['properties'] if p['id'] == clause)
        self.assertEqual([c['verdict'] for c in prop['values']], ['violated', 'violated'])
        out, value = invoke(source, 'diff', ['--from', 'Hand', '--to', 'Loop'])
        verdict(out, value, 'violated')
        self.assertEqual((row(value)['from'], row(value)['to']), ('violated', 'violated'))
        for plan in ['Loop', 'Hand']:
            out, value = sensitivity([('only', source)], 1, plan)
            verdict(out, value, 'violated')
            self.assertEqual(row(value['points'][0]['report'])['verdict'], 'violated')
            out, value = invoke(source, 'simulate', ['--configuration', plan])
            verdict(out, value, 'violated')
            self.assertEqual(row(value)['verdict'], 'violated')
            out, value = invoke(source, 'explain', ['--configuration', plan])
            verdict(out, value, 'violated')
            trace, = (t for t in value['explanation']['counterexamples'] if t['id'] == clause)
            self.assertEqual(trace['replay']['status'], 'verified')
            self.assertEqual(trace['evidence']['missing'], ['Doc'])
            if plan == 'Loop':
                self.assertEqual(trace['evidence']['repetition_exits'], [{'loop_name': 'rework', 'round': 2, 'phase': 'body'}])
            else:
                self.assertEqual(trace['evidence']['binding'], 'u2')

    def test_sensitivity_reports_the_record_alias_gate(self):
        source = replace(model('eb-alias-e1'), 'actor Boss { capacity 1', '''actor Notes {}
  actor Boss { capacity 1
    capability save(d: V) -> V { registers [Notes]; duration fixed 1min; result nondet { pass } }''')
        source = replace(source, 'include [Doc, Clerk, Boss]', 'include [Doc, Clerk, Boss, Notes]')
        source = replace(source, 'use(d: V) -> V {', 'use() -> V { reads [Notes];')
        for suffix in ['', '1', '2']:
            source = replace(source, f'u{suffix} = Boss.use(d{suffix})', f's{suffix} = Boss.save(d{suffix}); u{suffix} = Boss.use()')
        source = append(source, 'verify probability reached end.escalated <= 1')
        out, value = sensitivity([('only', source)], 1, 'Loop')
        verdict(out, value, 'unsupported')
        self.assertTrue(any(d['code'] == 'U-SEM-0' and 'loop_feature_combination' in d['message']
                            for d in value['diagnostics']), value['diagnostics'])

    def test_repeat_contracts_also_reach_sensitivity(self):
        base = replace(model('C1'), '  cut C { include [Rev, Clerk] }',
                       '  actor Doc { source document }\n  cut C { include [Rev, Clerk, Doc] }')
        base = replace(base, 'outcomes kernel rev', 'reads [Doc]; outcomes kernel rev')
        basis = append(base, 'contract Basis { evidence_basis Rev.review [Doc] }')
        o, v = sensitivity([('loop', basis)], 1, 'Loop3')
        verdict(o, v, 'satisfied')
        self.assertEqual(v['points'][0]['status'], 'evaluated')
        alias = append(Diff.paired_lineage(), 'contract Basis { evidence_basis Ledger.post [Invoice] }\nverify probability reached end.escalated <= 1')
        o, v = sensitivity([('loop', alias)], 1, 'Loop')
        verdict(o, v, 'satisfied')
        self.assertEqual(v['points'][0]['status'], 'evaluated')
        row, = (p for p in v['points'][0]['report']['properties'] if p['id'] == 'contract Basis #1 evidence_basis')
        self.assertEqual(row['verdict'], 'satisfied')

    def test_second_review_has_the_source_of_the_last_actual_fix(self):
        source = model('E5')
        a = source.index('  process P {')
        b = source.index('  cut C')
        hand = source[:a] + '''  process P { e = Reader.enter(); r1 = Rev.review();
          if r1 == pass { end done } else { f1 = Clerk.fix(); r2 = Rev.review();
            if r2 == pass { end done } else { f2 = Clerk.fix(); end escalated } }
        }
''' + source[b:]
        for s, folded in [(hand, False), (source, True)]:
            with self.subTest(repeat=folded):
                out, value = invoke(s, 'explain')
                verdict(out, value, 'violated')
                trace, = (t for t in value['explanation']['counterexamples'] if t['id'] == 'contract Basis #1 evidence_basis')
                ev = trace['evidence']
                self.assertEqual(ev['binding'], '@loop:rework:2:r' if folded else 'r2')
                self.assertEqual(ev['missing'], ['Invoice'])
                self.assertEqual(ev['sources'], ['Mail'])
                if folded:
                    self.assertEqual(ev['repetition'], {'loop_name': 'rework', 'round': 2,
                                                        'phase': 'body', 'call_site': 'r'})
                self.assertEqual(trace['replay']['status'], 'verified')
                out, value = invoke(replace(s, 'reads [Mail]', 'reads [Mail, Invoice]'))
                verdict(out, value, 'satisfied')

    def test_registration_finishes_before_the_read_each_round(self):
        r0 = ContractCommands.R0
        a = r0.index('  process Main {')
        b = r0.index('  bounds')
        loop = '''  process Main {
          e = Clerk.enter()
          repeat rework at_most 2 {
            r = Clerk.register()
            b = Reviewer.review(e)
            if b == pass { leave } else { again }
          } on_limit { end returned }
          x = Ledger.post(e, b)
          end posted
        }
'''
        hand = '''  process Main { e = Clerk.enter(); r1 = Clerk.register(); b1 = Reviewer.review(e);
          if b1 == pass { x1 = Ledger.post(e, b1); end posted } else {
            r2 = Clerk.register(); b2 = Reviewer.review(e);
            if b2 == pass { x2 = Ledger.post(e, b2); end posted } else { end returned }
          }
        }
'''
        for body, broken in [(hand, hand.replace('r1 = Clerk.register(); b1 = Reviewer.review(e);', 'b1 = Reviewer.review(e); r1 = Clerk.register();')),
                             (loop, loop.replace('r = Clerk.register()\n            b = Reviewer.review(e)', 'b = Reviewer.review(e)\n            r = Clerk.register()'))]:
            with self.subTest(body=body):
                out, value = invoke(r0[:a] + body + r0[b:])
                verdict(out, value, 'satisfied')
                self.assertEqual({t['end']: (t['elapsed']['min'], t['elapsed']['max']) for t in value['terminals']},
                                 {'posted': (4, 6), 'returned': (5, 5)})
                if body == loop:
                    row, = (r for r in value['agency']['origin'] if r['actor'] == 'Reviewer')
                    self.assertEqual(row['via_records'], [{'record': 'Notes', 'registrars': [{'call': 'r', 'people': ['p1']}],
                                                           'sources': [{'source': 'Know', 'route': 'registrar_direct'}]}])
                out, value = invoke(r0[:a] + broken + r0[b:])
                verdict(out, value, 'violated')

    def test_last_examined_plan_keeps_its_result_identity(self):
        directory = ROOT / 'skills/relca-design/examples'
        clause = 'same_version Reviewer.check_plan(p), Dev.refine_a(p)'
        hand = (directory / 'split-first.relca').read_text()
        loop = (directory / 'split-first-repeat.relca').read_text()
        marker = 'separation Lead.split.finish, Reviewer.check.finish'
        for text in (hand, loop):
            out, value = invoke(replace(text, marker, marker + '\n    ' + clause))
            verdict(out, value, 'satisfied')
            self.assertEqual({t['end']: t['probability']['min'] for t in value['terminals']},
                             {'delivered': '20801/25000', 'escalated': '36/625', 'late_fail': '2759/25000'})
            row, = (r for r in value['properties'] if r['id'] == 'probability reached #1')
            self.assertEqual(row['probability'], {'min': '93/12500', 'max': '93/12500'})
        wrong = replace(loop, 'process SplitFirst {', 'process SplitFirst { p0 = Lead.split()')
        wrong = replace(wrong, 'a = Dev.refine_a(p)', 'a = Dev.refine_a(p0)')
        out, value = invoke(replace(wrong, marker, marker + '\n    ' + clause))
        verdict(out, value, 'violated')
        row, = (p for p in value['properties'] if p['id'] == 'contract Controls #3 same_version')
        self.assertEqual(row['verdict'], 'violated')


class ComparisonRefusal(unittest.TestCase):
    def test_child_refusal_is_visible_under_adr_0002_precedence(self):
        source = replace(model('C1'), '  cut C { include [Rev, Clerk] }', '''
  actor Doc { source document }
  actor Notes {}
  actor Gate { capacity 1;
    capability save(v:V) -> F { registers [Notes]; duration fixed 1min; result nondet { ok } }
    capability accept(v:V) -> F { reads [Doc,Notes]; duration fixed 1min; result nondet { ok } }
  }
  cut C { include [Rev, Clerk, Doc, Gate, Notes] }''')
        source = replace(source, '    end done\n  }\n  process Looped2',
                         '    saved = Gate.save(r); p = Gate.accept(r); end done\n  }\n  process Looped2')
        start = source.index('  process Unrolled3')
        end = source.index('  process Unrolled2')
        hand = source[start:end]
        for k in (1, 2, 3):
            hand = replace(hand, f'if r{k} == pass {{ end done }}',
                           f'if r{k} == pass {{ s{k} = Gate.save(r{k}); p{k} = Gate.accept(r{k}); end done }}')
        source = source[:start] + hand + source[end:]
        source = append(source, 'contract GateEvidence applies_to [Loop3] reason "only Loop3 uses this requirement" { evidence_basis Gate.accept [Doc] }\nsimulation replications 4 seed 7 confidence 0.5')
        for mode, other, mass, threshold, clock in [
            ('exhaustive', 'satisfied', 'true: 0.1, false: 0.9', '0.2', '30'),
            ('exhaustive', 'violated', 'true: 0.1, false: 0.9', '0.05', '30'),
            ('exhaustive', 'inconclusive', 'true: 0.1, false: 0.9', '0.05', '7'),
            ('simulation', 'satisfied', 'true: 0, false: 1', '1', '30'),
            ('simulation', 'violated', 'true: 1, false: 0', '0.2', '30'),
            ('simulation', 'inconclusive', 'true: 0, false: 1', '0.05', '30'),
        ]:
            for cmd in (('compare', 'diff') if mode == 'exhaustive' else ('compare',)):
                with self.subTest(cmd=cmd, mode=mode, other=other):
                    s = replace(source, 'true: 0.1, false: 0.9', mass)
                    s = replace(replace(s, '<= 0.2', '<= ' + threshold), 'clock 30min', f'clock {clock}min')
                    args = ['--plans', 'Loop3,Hand3', '--mode', mode] if cmd == 'compare' else ['--from', 'Loop3', '--to', 'Hand3']
                    out, v = invoke(s, cmd, args)
                    verdict(out, v, 'violated' if other == 'violated' else 'unsupported')
                    sides = v['plans'] if cmd == 'compare' else [v['from'], v['to']]
                    self.assertEqual([p['report']['verdict'] for p in sides], ['unsupported', other])
                    self.assertTrue(any(d['code'] == 'U-SEM-0' and 'loop_feature_combination' in d['message']
                                        for d in sides[0]['report']['diagnostics']))
        for cmd, args in [('compare', ['--plans', 'Loop3,Missing', '--mode', 'exhaustive']),
                          ('diff', ['--from', 'Loop3', '--to', 'Missing'])]:
            out, v = invoke(source, cmd, args)
            verdict(out, v, 'invalid')


class RepeatedFork(unittest.TestCase):
    def test_parallel_rounds_across_commands(self):
        ordered = model('fork-pair')
        source = replace(ordered, '  policy fifo tie_by case_id then call_order\n', '')
        out, v = invoke(source, 'compare', ['--plans', 'Loop,Hand', '--mode', 'exhaustive'])
        verdict(out, v, 'violated')
        starts = {r['key']['subject']: r['values'] for r in v['tables']['metrics']
                  if r['key']['scope'] == 'task' and r['key']['metric'] == 'starts'}
        for call in ('a', 'b'):
            for k in (1, 2):
                row = starts[f'loop:{call}:round{k}']
                self.assertEqual(row[0]['value'], starts[f'{call}{k}'][1]['value'])
                self.assertEqual([c['status'] for c in row], ['reported', 'not_applicable'])
        row, = v['tables']['repetition']
        self.assertEqual(row['values'][0]['value']['rounds'], {'min': 2, 'max': 2})
        out, v = invoke(source, 'diff', ['--from', 'Hand', '--to', 'Loop'])
        verdict(out, v, 'violated')
        row, = v['differences']
        self.assertEqual((row['kind'], row['target'], row['to']['sites']), ('repetition', 'wave', ['a', 'b']))
        simulations = {}
        for plan in ('Loop', 'Hand'):
            o, v = sensitivity([('base', source)], 1, plan)
            verdict(o, v, 'violated')
            prop, = v['points'][0]['report']['properties']
            self.assertEqual(prop['cases'][0]['probability'], {'min': '1', 'max': '1'})
            self.assertEqual(v['summaries'][0]['status'], 'unavailable')
            self.assertEqual(v['summaries'][0]['reason'], 'incomplete_points')
            simulations[plan] = invoke(source, 'simulate', ['--configuration', plan])
            o, v = invoke(source, 'explain', ['--configuration', plan])
            verdict(o, v, 'violated')
            prop, = v['properties']
            self.assertEqual(prop['cases'][0]['probability'], {'min': '1', 'max': '1'})
        # All four samples reach the end, so the probability constraint (<= 0.1) is violated.
        expected = 'violated'

        def assert_completed(report, *, same_time=True):
            self.assertEqual(report['verdict'], expected)
            sim = report['simulation']
            self.assertEqual(sim['status'], 'completed')
            self.assertEqual(sim['completed_replications'], sim['replications'])
            if same_time:
                self.assertIn('simulation.same_time', [a['field'] for a in report['assumptions']])

        for plan, (out, report) in simulations.items():
            with self.subTest(command='simulate', plan=plan):
                verdict(out, report, expected)
                assert_completed(report)
        out, v = invoke(source, 'compare', ['--plans', 'Loop,Hand', '--mode', 'simulation'])
        verdict(out, v, expected)
        self.assertEqual([p['configuration'] for p in v['plans']], ['Loop', 'Hand'])
        for p in v['plans']:
            with self.subTest(command='compare', plan=p['configuration']):
                assert_completed(p['report'])
        for plan in ('Loop', 'Hand'):
            o, sampled = invoke(ordered, 'simulate', ['--configuration', plan])
            # Each round starts both tasks at the same minute (different
            # actors); their elapsed time is max(2,3)=3 and work is 2+3=5.
            # Two rounds give work 10 and all four trials hit done. At alpha
            # 1/4 the one-sided lower end (1/4)^(1/4) exceeds the bound 0.1.
            verdict(o, sampled, 'violated')
            assert_completed(sampled, same_time=False)
            prop, = sampled['properties']
            self.assertEqual((prop['verdict'], prop['cases'][0]['hits']), ('violated', 4))
            self.assertEqual(sampled['metrics']['overall']['work'],
                             {'mean': '10', 'min': '10', 'max': '10', 'samples': 4, 'reason': None})
            for actor in sampled['metrics']['actors']:
                self.assertEqual(actor['queue']['wait_total']['stats']['mean'], '0')

    def test_condition_wait_in_body_is_not_counted_as_progress(self):
        source = append(BASE, 'observed case { flag: bool = true }')
        good = replace(source, 'r = A.c(); again', 'wait until flag == true; r = A.c(); again')
        out, v = invoke(good)
        verdict(out, v, 'satisfied')
        refusal(replace(good, 'until flag == true; r = A.c(); again', 'until flag == true; again'),
                'E-CON-0', 'zero_time_iteration')

    def test_full_contracts_and_hand_times(self):
        examples = ROOT / 'skills/relca-design/examples'
        for name, prefix, body, wanted, bad, totals in [
            ('check-at-end', 'CheckAtEnd', '''
              repeat attempt at_most 2 {
                p = Lead.split()
                fork w { a = Dev.refine_a(p); b = Dev.refine_b(p) } join all w
                x = Lead.integrate(a,b)
                v = Reviewer.check(p,x)
                if v == pass { leave } else { again }
              } on_limit { end escalated }
              r = Lead.release(x); end delivered
            ''', 'violated', '783/20000', {'delivered': ('36279/40000', 121, 241, 181, 361),
                                         'escalated': ('3721/40000', 240, 240, 360, 360)}),
            ('split-first', 'SplitFirst', '''
              repeat attempt at_most 2 {
                p = Lead.split()
                c = Reviewer.check_plan(p)
                if c == pass {
                  fork w { a = Dev.refine_a(p); b = Dev.refine_b(p) } join all w
                  x = Lead.integrate(a,b)
                  v = Reviewer.check(p,x)
                  if v == pass { r = Lead.release(x); end delivered } else { end late_fail }
                } else { again }
              } on_limit { end escalated }
            ''', 'satisfied', '93/12500', {'delivered': ('20801/25000', 131, 171, 191, 231),
                                          'late_fail': ('2759/25000', 130, 170, 190, 230),
                                          'escalated': ('36/625', 80, 80, 80, 80)}),
        ]:
            source = (examples / (name + '.relca')).read_text()
            a, b = source.index('  process '), source.index('  cut ')
            loop = source[:a] + f'  process {prefix} {{\n{body}\n  }}\n' + source[b:]
            for s, folded in [(source, False), (loop, True)]:
                with self.subTest(name=name, repeat=folded):
                    s = append(s, 'metrics { window 400min; currency "JPY"; breakdown waits }')
                    out, v = invoke(s)
                    verdict(out, v, wanted)
                    for p in v['properties']:
                        if p['id'] == 'probability reached #1':
                            self.assertEqual(p['probability'], {'min': bad, 'max': bad})
                        else:
                            self.assertEqual(p['verdict'], 'satisfied', p)
                    self.assertEqual(len(v['properties']), 6)
                    observed = {t['end']: (t['probability']['min'], t['elapsed']['min'], t['elapsed']['max'],
                                           t['work']['min'], t['work']['max']) for t in v['terminals']}
                    self.assertEqual(observed, totals)
                    if folded:
                        joins = v['metrics']['cases'][0]['joins']
                        self.assertEqual([j['fork'] for j in joins], ['@loop:attempt:1:w', '@loop:attempt:2:w'])

    def test_join_all_shortest_time_is_not_its_shortest_branch(self):
        source = '''model ForkTime version "0.2" {
          type V = enum { ok }
          actor A { capacity 1; capability zero() -> V { duration fixed 0min; result nondet { ok } } }
          actor B { capacity 1; capability long() -> V { duration fixed 60min; result nondet { ok } } }
          process P { repeat job at_most 2 { fork w { x = A.zero(); y = B.long() } join all w; again }
                       on_limit { end done } }
          bounds { cases 1; live_tasks 2; clock 200min }
          verify reachable end.done
        }'''
        out, v = invoke(source)
        verdict(out, v, 'satisfied')
        self.assertEqual(v['terminals'][0]['elapsed'], {'min': 120, 'max': 120})
        refusal(replace(source, 'join all w', 'join any w where result == ok tie by [x,y] remaining drain if none -> done'),
                'U-SEM-0', 'loop_feature_combination')
        refusal(replace(source, 'fixed 60min', 'fixed 0min'), 'E-CON-0', 'zero_time_iteration')
        for command in ('wait until flag == true', 'fork v { l = A.zero() } join all v'):
            s = append(source, 'observed case { flag: bool = true }')
            refusal(replace(s, 'on_limit { end done }', f'on_limit {{ {command}; end done }}'),
                    'U-SEM-0', 'loop_feature_combination')
        known = replace(source, 'join all w; again', 'join all v; fork v { z = A.zero() } join all v; again')
        refusal(known, 'U-SEM-0', 'join must target its immediately preceding fork')


class RepeatQuantiles(unittest.TestCase):
    def test_ninety_percent_reach_at_one_minute_but_not_ninety_five(self):
        additions = '''property case.Fin = reached(end.done)
metric quantile.P90 = quantile(0.9, case.Fin)
metric quantile.P95 = quantile(0.95, case.Fin)'''
        hand = (DATA / 'witnesses/L2.relca').read_text()
        for source in (hand, model('L2')):
            with self.subTest(repeat=source != hand):
                s = append(source, additions)
                out, value = invoke(s)
                verdict(out, value, 'satisfied')
                rows = {r['name']: r['value'] for r in value['quality']['quantiles']}
                self.assertEqual(rows, {
                    'P90': {'lower': '1', 'upper': '1', 'basis': 'exact', 'reason': None},
                    'P95': {'lower': None, 'upper': None, 'basis': None, 'reason': 'beyond_reached_mass'},
                })
                cut, cut_value = invoke(replace(s, 'clock 20min', 'clock 6min'))
                verdict(cut, cut_value, 'inconclusive')
                p95, = (r for r in cut_value['quality']['quantiles'] if r['name'] == 'P95')
                self.assertIsNone(p95['value']['upper'])
                self.assertIsNotNone(p95['value']['reason'])


class RepeatedMutation(unittest.TestCase):
    def test_outer_refusal_guard_reports_unavailable_on_repeat_only(self):
        # The guard prevents the posting after a fail. Removing it destroys
        # that refusal without changing any assigned invariant (none declared).
        source = replace(model('M3'), '  contract Guard { invariant Ledger.post.commit -> reviewed == pass }\n', '')
        a, b = source.index('  process P {'), source.index('  cut C')
        def process(middle):
            return source[:a] + ('  process P { entry = Reader.enter(); q = Rev.review(entry); '
                'if q == fail { end rejected } else {}; ' + middle + '; p = Ledger.post(entry); end done }\n') + source[b:]
        for middle, verdict_name, result, reason in [
            ('wait 1min', 'satisfied', 'detected', None),
            ('repeat wave at_most 1 { wait 1min; leave } on_limit { leave }', 'unsupported', 'unsupported', 'loop_control'),
        ]:
            out, v = invoke(process(middle), 'mutate', ['--kinds', 'refusal_path_cut'])
            verdict(out, v, verdict_name)
            row, = (r for r in v['mutations'] if r['kind'] == 'refusal_path_cut')
            mutant, = row['mutants']
            self.assertEqual(mutant['site']['statement_path'], 'body/2')
            self.assertEqual((mutant['result'], mutant['reason']), (result, reason))

    def test_inner_repeat_does_not_supply_outer_guard_control(self):
        source = model('M3')
        a, b = source.index('  process P {'), source.index('  cut C')
        def process(branch):
            return source[:a] + ('  process P { entry = Reader.enter(); q = Rev.review(entry); '
                'if q == pass { ' + branch + ' } else {}; p = Ledger.post(entry); end done }\n') + source[b:]
        for branch, expected_count in [
            ('f1 = Clerk.fix(); f2 = Clerk.fix()', 0),
            ('repeat polish at_most 2 { f = Clerk.fix(); again } on_limit { leave }', 0),
            ('f1 = Clerk.fix(); f2 = Clerk.fix(); end escalated', 1),
            ('repeat polish at_most 2 { f = Clerk.fix(); again } on_limit { end escalated }', 1),
        ]:
            out, v = invoke(process(branch), 'mutate', ['--kinds', 'hold_delayed'])
            row, = (r for r in v['mutations'] if r['kind'] == 'hold_delayed')
            self.assertEqual(row['candidates'], expected_count, v.get('diagnostics'))
            if expected_count:
                verdict(out, v, 'violated')  # fail already reaches post on the original else path
                mutant, = row['mutants']
                self.assertEqual(mutant['site']['retained_branch'], 'else')
                self.assertEqual(mutant['result'], 'undetected')
            else:
                self.assertEqual((out.returncode, v['verdict']), (5, 'vacuous'))

    def test_eligible_guard_keeps_early_control_and_does_not_lose_its_candidate(self):
        source = replace(replace(model('M3'), 'enum { pass, fail }', 'enum { pass, fail, reject }'),
                         'result nondet { pass, fail }', 'result nondet { pass, fail, reject }')
        for branch, hand, expected_property, expected_result in [
            ('if r == pass { leave } else { if r == reject { end rejected } else {} }; f = Clerk.fix(); again',
             'r1 = Rev.review(entry); p = Ledger.post(entry); end done', 'violated', 'detected'),
            ('if r == reject { end rejected } else { if r == pass { leave } else {} }; f = Clerk.fix(); again',
             'r1 = Rev.review(entry); end rejected; p = Ledger.post(entry); end done', 'vacuous', 'undetected'),
            ('if r == fail { f = Clerk.fix(); again } else { if r == reject { end rejected } else {} }; n = Clerk.fix(); leave',
             'r1 = Rev.review(entry); f1 = Clerk.fix(); r2 = Rev.review(entry); f2 = Clerk.fix(); r3 = Rev.review(entry); f3 = Clerk.fix(); end escalated; p = Ledger.post(entry); end done',
             'vacuous', 'undetected'),
        ]:
            s = replace(source, 'if r == pass { leave } else { f = Clerk.fix(); again }', branch)
            a, b = s.index('  process P {'), s.index('  cut C')
            _, written = invoke(s[:a] + '  process P { entry = Reader.enter(); ' + hand + ' }\n' + s[b:])
            _, v = invoke(s, 'mutate', ['--kinds', 'hold_delayed'])
            row, = (r for r in v['mutations'] if r['kind'] == 'hold_delayed')
            self.assertEqual(row['candidates'], 3)
            mutant, = (m for m in row['mutants'] if m['site']['statement_path'] == 'body/1/repeat/1'
                       and m['site']['retained_branch'] == 'then')
            guard = lambda r: next(p['verdict'] for p in r['properties'] if p['id'] == 'contract Guard #1 invariant')
            self.assertEqual(guard(written), expected_property)
            self.assertEqual(guard(mutant['report']), expected_property)
            self.assertEqual(mutant['result'], expected_result)

    def test_repeat_refuses_response_families_even_outside_the_loop(self):
        source = '''model Mixed version "0.2" {
          type V = enum { ok }
          actor Ledger { capacity 1; capability post(k: CaseId) -> V { commits; duration fixed 1min; result nondet { ok } } }
          cut C { include [Ledger] }
          process P { outer = Ledger.post(case.id) response_loss none;
            repeat wave at_most 1 { inner = Ledger.post(case.id); leave } on_limit { end done };
            undeclared = Ledger.post(case.id); end done }
          bounds { cases 1; live_tasks 1; clock 20min }
          verify reachable end.done
        }'''
        loss = replace(source, 'response_loss none', 'response_loss sequence [delivered]')
        refusal(loss, 'U-SEM-0', 'repeat mutation with response_loss or retry is not connected',
                'mutate', ['--kinds', 'receipt_unknown'])
        outer_only = replace(source, 'inner = Ledger.post(case.id); leave', 'wait 1min; leave')
        for s in (source, outer_only):
            out, v = invoke(s)
            verdict(out, v, 'satisfied')
            for name in ('receipt_unknown', 'command_resent'):
                out, v = invoke(s, 'mutate', ['--kinds', name])
                verdict(out, v, 'unsupported')
                row, = (r for r in v['mutations'] if r['kind'] == name)
                self.assertEqual((row['status'], row['reason'], row['candidates'], row['mutants']),
                                 ('unsupported', 'loop_feature_combination', None, []))

    def test_summary_reuse_with_physical_lineage_reports_unsupported(self):
        repeat = model('summary-round-lineage')
        start, end = repeat.index('  process P {'), repeat.index('  bounds')
        calls = 'a=A.produce(); b=A.consume(a); c=A.consume(a); '
        written = (repeat[:start] + '  process P { ' + calls
                   + 'if c==ok { p=A.post(); end done } else { end rejected } }\n' + repeat[end:])
        for text, overall, result, reason, judgement in [
            (written, 'satisfied', 'detected', None, 'c'),
            (repeat, 'unsupported', 'unsupported', 'loop_control', '@loop:wave:1:c'),
        ]:
            out, v = invoke(text, 'mutate', ['--kinds', 'summary_reused'])
            verdict(out, v, overall)
            row, = (r for r in v['mutations'] if r['kind'] == 'summary_reused')
            mutant, = row['mutants']
            self.assertEqual(mutant['site']['affected_bindings'], ['a', 'c'])
            self.assertEqual((mutant['result'], mutant['reason']), (result, reason))
            self.assertEqual([l['call'] for l in v['original']['agency']['lineage']], [judgement])

    def test_on_limit_leave_matches_written_guard_candidates(self):
        source = model('M3')
        start, end = source.index('  process P {'), source.index('  cut C')
        for body, prefix in [
            ('repeat wave at_most 1 { f = Clerk.fix(); again } on_limit { r = Rev.review(entry); '
             'if r == pass { leave } else { n = Clerk.fix(); end rejected } }; p = Ledger.post(entry); end done', 'body/1/on_limit/1'),
            ('f = Clerk.fix(); r = Rev.review(entry); if r == pass { p1 = Ledger.post(entry); end done } else { n = Clerk.fix(); end rejected }; '
             'p = Ledger.post(entry); end done', 'body/3'),
        ]:
            text = source[:start] + '  process P { entry = Reader.enter(); ' + body + ' }\n' + source[end:]
            out, v = invoke(text, 'mutate', ['--kinds', 'hold_delayed'])
            verdict(out, v, 'violated')
            row, = (r for r in v['mutations'] if r['kind'] == 'hold_delayed')
            self.assertEqual(row['candidates'], 2)
            self.assertEqual([(m['site']['statement_path'], m['site']['retained_branch'], m['result'])
                              for m in row['mutants']], [(prefix, 'then', 'detected'), (prefix, 'else', 'undetected')])

    def test_lineage_retained_on_physical_rounds_detects_the_same_read_loss(self):
        s = model('M3')
        s = replace(s, '  actor Invoice { source document }',
                    '  actor Invoice { source document }\n  actor Rules { source document }')
        s = replace(s, 'review(e: Entry) -> V {', 'review(e: Entry) -> V { reads [Rules];')
        s = replace(s, 'include [Invoice, Reader', 'include [Invoice, Rules, Reader')
        s = replace(s, '  contract Guard { invariant Ledger.post.commit -> reviewed == pass }\n', '')
        hand = '''entry = Reader.enter(); r1 = Rev.review(entry);
        if r1 == pass { p1 = Ledger.post(entry); end done } else {
          f1 = Clerk.fix(); r2 = Rev.review(entry);
          if r2 == pass { p2 = Ledger.post(entry); end done } else {
            f2 = Clerk.fix(); r3 = Rev.review(entry);
            if r3 == pass { p3 = Ledger.post(entry); end done } else { f3 = Clerk.fix(); end escalated }
          }
        }'''
        a, b = s.index('  process P {'), s.index('  cut C')
        written = s[:a] + '  process P { ' + hand + ' }\n' + s[b:]
        # Each judgement uses Invoice as its subject and Rules as its own
        # independent input. Removing Rules removes exactly that input.
        for source in (s, written):
            out, v = invoke(source, 'mutate', ['--kinds', 'evidence_missing'])
            verdict(out, v, 'violated')
            row, = (r for r in v['mutations'] if r['kind'] == 'evidence_missing')
            loss, = (m for m in row['mutants'] if m['id'] == 'evidence_missing:Rev.review:reads:Rules')
            self.assertEqual(loss['result'], 'detected')
            self.assertEqual(sum(c['check'] == 'agency.lineage' and c['direction'] == 'worsened'
                                 for c in loss['assigned_changes']), 3)

    def test_unconnected_repeat_mutation_families_and_combinations_are_named(self):
        base = model('M3')
        for family in ('authority_expired', 'old_promise_denominator_removed', 'original_access_removed'):
            with self.subTest(family=family):
                out, v = invoke(base, 'mutate', ['--kinds', family])
                verdict(out, v, 'unsupported')
                row, = (r for r in v['mutations'] if r['kind'] == family)
                self.assertEqual((row['status'], row['reason'], row['mutants']), ('unsupported', 'loop_feature_combination', []))
        for feature, source in [
            ('wait until', replace(replace(base, 'observed case { reviewed: optional<V> }',
                                            'observed case { reviewed: optional<V>; ready: bool = true }'),
                                   'r = Rev.review(entry)', 'wait until ready == true; r = Rev.review(entry)')),
            ('quantile', append(model('L2'), 'property case.Fin = reached(end.done)\nmetric quantile.P90 = quantile(0.9, case.Fin)')),
            ('evidence_basis', append(base, 'contract Basis { evidence_basis Rev.review [Invoice] }')),
            ('registers', replace(replace(replace(base, '  actor Invoice', '  actor Notes {}\n  actor Invoice'),
                                          'reads [Invoice];', 'reads [Invoice]; registers [Notes];'),
                                  'include [Invoice,', 'include [Notes, Invoice,')),
        ]:
            with self.subTest(feature=feature):
                out, v = invoke(source)
                verdict(out, v, 'satisfied')
                refusal(source, 'U-SEM-0', f'repeat mutation with {feature} is not connected',
                        'mutate', ['--kinds', 'hold_delayed'])
        source = replace(base, 'r = Rev.review(entry)', 'r = Rev.review(entry); inside = Ledger.post(entry)')
        out, v = invoke(source, 'mutate', ['--kinds', 'command_resent,receipt_unknown'])
        verdict(out, v, 'unsupported')
        for row in v['mutations']:
            if row['kind'] in ('command_resent', 'receipt_unknown'):
                self.assertEqual((row['status'], row['reason'], row['candidates'], row['mutants']),
                                 ('unsupported', 'loop_feature_combination', None, []))

    def test_summary_written_before_the_next_round_guard_is_not_duplicated(self):
        source = '''model RoundGuard version "0.2" {
          type V = enum { ok }
          observed case { written: bool = false }
          actor Doc { source document }
          actor A { capacity 1
            capability produce() -> V { reads [Doc]; duration fixed 1min; result nondet { ok }; writes written; finish { written = true } }
            capability consume(v: V) -> V { duration fixed 1min; result nondet { ok } }
          }
          cut C { include [Doc,A] }
          process P {
            repeat wave at_most 2 {
              if written == true { leave } else {}
              a = A.produce(); b = A.consume(a); c = A.consume(a); again
            } on_limit { end done }; end done
          }
          bounds { cases 1; live_tasks 1; clock 20min }
          verify reachable end.done
        }'''
        out, value = invoke(source, 'mutate', ['--kinds', 'summary_reused'])
        self.assertEqual((out.returncode, value['verdict']), (5, 'vacuous'), value['diagnostics'])
        row, = (r for r in value['mutations'] if r['kind'] == 'summary_reused')
        self.assertEqual(row['candidates'], 0)

    def test_repeat_mutation_selection_and_budget_do_not_add_trials(self):
        source = append(model('M3'), 'configuration AsIs { cut C; process P }')
        args = ['--configuration', 'AsIs']
        def trials(more):
            _, value = invoke(source, 'mutate', args + more)
            return {m['id'] for r in value['mutations'] for m in r['mutants'] if m['report'] is not None}
        all_trials = trials([])
        hold = trials(['--kinds', 'hold_delayed'])
        self.assertEqual(len(hold), 2)
        self.assertLess(hold, all_trials)
        ident = 'hold_delayed:P:body/1/repeat/1:keep_then'
        self.assertEqual(trials(['--kinds', 'hold_delayed', '--sites', ident]), {ident})
        self.assertEqual(trials(['--kinds', 'hold_delayed', '--mutant-limit', '1']), {ident})

    def test_guard_candidates_depend_on_the_discarded_direction(self):
        source = model('M3')
        old = 'if r == pass { leave } else { f = Clerk.fix(); again }'
        a = replace(source, old, 'if r == fail { f = Clerk.fix(); again } else {}; n = Clerk.fix(); leave')
        b = replace(source, old, 'if r == pass { leave } else {}; f = Clerk.fix(); again')
        c = replace(source, '    repeat rework', '    q = Rev.review(entry)\n    if q == fail { repeat rework')
        c = replace(c, '} on_limit { end escalated }', '} on_limit { end escalated } } else {}')
        # For (a), before mutation only passing reviews reach post; removing
        # the early control makes both results reach it in the first round.
        # For (b), removing the exit makes all three rounds take the fix and
        # escalate, never activating Guard. For (c), post uses q without a
        # guard after removing the whole repeat, so a fail reaches commit.
        for label, s, expected_count, path, hand, expected, detection in [
            ('a', a, 1, 'body/1/repeat/1',
             'r = Rev.review(entry); n = Clerk.fix(); p = Ledger.post(entry); end done', 'violated', 'detected'),
            ('b', b, 1, 'body/1/repeat/1',
             'r1 = Rev.review(entry); f1 = Clerk.fix(); r2 = Rev.review(entry); f2 = Clerk.fix(); r3 = Rev.review(entry); f3 = Clerk.fix(); end escalated; p = Ledger.post(entry); end done', 'vacuous', 'undetected'),
            ('c', c, 3, 'body/2', 'q = Rev.review(entry); p = Ledger.post(entry); end done', 'violated', 'detected'),
        ]:
            with self.subTest(shape=label):
                start, end = s.index('  process P {'), s.index('  cut C')
                _, written = invoke(s[:start] + '  process P { entry = Reader.enter(); ' + hand + ' }\n' + s[end:])
                out, mutation = invoke(s, 'mutate', ['--kinds', 'hold_delayed'])
                self.assertNotEqual(out.returncode, 3, mutation['diagnostics'])
                row, = (r for r in mutation['mutations'] if r['kind'] == 'hold_delayed')
                self.assertEqual(row['candidates'], expected_count)
                mutant, = (m for m in row['mutants'] if m['site']['statement_path'] == path)
                self.assertEqual(mutant['site']['retained_branch'], 'else')
                guard = lambda r: next(p['verdict'] for p in r['properties'] if p['id'] == 'contract Guard #1 invariant')
                self.assertEqual(guard(written), expected)
                self.assertEqual(guard(mutant['report']), expected)
                self.assertEqual(mutant['result'], detection)
                if label == 'a':
                    original_hand = '''entry = Reader.enter();
                    r1 = Rev.review(entry); f1 = Clerk.fix();
                    if r1 == pass { p1 = Ledger.post(entry); end done } else {
                      r2 = Rev.review(entry); f2 = Clerk.fix();
                      if r2 == pass { p2 = Ledger.post(entry); end done } else {
                        r3 = Rev.review(entry); f3 = Clerk.fix();
                        if r3 == pass { p3 = Ledger.post(entry); end done } else { end escalated }
                      }
                    }'''
                    for original in (s, s[:start] + '  process P { ' + original_hand + ' }\n' + s[end:]):
                        out, checked = invoke(original)
                        verdict(out, checked, 'satisfied')
                        self.assertEqual(guard(checked), 'satisfied')
                        # Entry 1 + review/fix 3 per round + post 1 on pass only.
                        self.assertEqual({t['end']: (t['elapsed'], t['work']) for t in checked['terminals']}, {
                            'done': ({'min': 5, 'max': 11}, {'min': 5, 'max': 11}),
                            'escalated': ({'min': 10, 'max': 10}, {'min': 10, 'max': 10}),
                        })

    def test_two_directions_per_guard_not_per_expanded_round(self):
        source = model('M3')
        out, original = invoke(source)
        verdict(out, original, 'satisfied')
        # Direct counterfactuals, with all copies of the guarded behavior
        # edited in the unrolled copy; the engine never builds these from IR.
        a, b = source.index('  process P {'), source.index('  cut C')
        for body, contract_status in [
            ('e = Reader.enter(); r1 = Rev.review(e); p1 = Ledger.post(e); end done', 'violated'),
            ('e = Reader.enter(); r1 = Rev.review(e); f1 = Clerk.fix(); r2 = Rev.review(e); f2 = Clerk.fix(); r3 = Rev.review(e); f3 = Clerk.fix(); end escalated; p1 = Ledger.post(e); end done', 'vacuous'),
        ]:
            _, manual = invoke(source[:a] + f'  process P {{ {body} }}\n' + source[b:])
            row, = (p for p in manual['properties'] if p['id'] == 'contract Guard #1 invariant')
            self.assertEqual(row['verdict'], contract_status)
        out, report = invoke(source, 'mutate', ['--kinds', 'hold_delayed,refusal_path_cut'])
        for family, reverse_result, reverse_reason in [
            ('hold_delayed', 'undetected', 'no_worsening'),
            ('refusal_path_cut', 'unsupported', 'loop_control'),
        ]:
            row, = (m for m in report['mutations'] if m['kind'] == family)
            self.assertEqual(row['candidates'], 2)
            mutants = {m['site']['retained_branch']: m for m in row['mutants']}
            self.assertEqual(set(mutants), {'then', 'else'})
            self.assertEqual(mutants['then']['result'], 'detected')
            self.assertEqual((mutants['else']['result'], mutants['else']['reason']), (reverse_result, reverse_reason))
            for m in mutants.values():
                self.assertEqual(m['site']['statement_path'], 'body/1/repeat/1')
        verdict(out, report, 'unsupported')


class Gates(unittest.TestCase):
    def test_fork_mutation_stays_closed(self):
        refusal(model('fork-pair'), 'U-SEM-0', 'loop_feature_combination', 'mutate', ['--configuration', 'Loop'])

    def test_conform(self):
        source = replace(BASE, 'process P {', 'process P {')
        source = append(source, 'configuration AsIs { cut C; process P }\nconfiguration ToBe { cut C; process P }')
        for command, args, code, reason in [
            ('conform', ('--configuration', 'AsIs', '--log', '@events.jsonl', '--scope', 'event-contracts'), 'U-LOG-1', ''),
        ]:
            with self.subTest(command=command):
                selected = BASE if command == 'mutate' else source
                value = refusal(selected, code, reason, command, args, {'events.jsonl': ''})
                assert any(d['message'] for d in value['diagnostics'] if d['code'] == code)
                assert any('loop' in d['message'] or 'repeat' in d['message'] for d in value['diagnostics'] if d['code'] == code), value['diagnostics']

    def test_contest_target_in_body(self):
        source = '''model LoopContest version "0.2" {
type T = enum { pass, fail }
people [j,p]
actor Judge { executors [j]; capability judge() -> T { duration fixed 1min; result nondet { fail } } }
actor Route { executors [p]; capability respond() -> T { duration fixed 2min; result nondet { pass } } }
actor Party { control party; standing over [end.returned] basis "claim" }
permit j to Judge.judge
permit p to Route.respond
authority G { basis "decision"; grants j to Judge.judge }
process Flow { repeat rework at_most 3 { d = Judge.judge(); if d == fail { end returned } else { again } } on_limit { end posted } }
cut C { include [Judge,Route,Party] }
bounds { cases 1; live_tasks 2; clock 20min }
verify safety no_resource_overcommit
contract Controls { contest Party on end.returned route Route.respond reversible_by [j] respond_by 2min }
}'''
        refusal(source, 'U-SEM-0', 'loop_feature_combination')

    def test_agency_warnings_are_code_specific_unchecked(self):
        out, value = invoke((DATA / 'models/lineage.relca').read_text())
        verdict(out, value, 'satisfied')
        # Follow the existing response_control mark convention without fixing
        # a new loop report schema. Each code needs its own reasoned mark.
        marks = [*value['assumptions'], *value['agency'].get('unchecked', [])]
        for code in ('W-AGY-0', 'W-AGY-2', 'W-LIN-2', 'W-RESP-2'):
            assert not any(d['code'] == code for d in value['diagnostics'])
            assert any(code in json.dumps(row) and 'loop_control' in json.dumps(row)
                       and ('unchecked' in json.dumps(row) or 'not_derived' in json.dumps(row))
                       for row in marks), (code, marks)


class Preservation(unittest.TestCase):
    def test_context_words_remain_binding_names(self):
        for word in ('repeat', 'again', 'leave', 'at_most', 'on_limit', 'gap', 'over'):
            with self.subTest(word=word):
                source = replace(BASE, 'repeat rework at_most 3 { r = A.c(); again } on_limit { end limit }', f'{word} = A.c();')
                out, value = invoke(source)
                verdict(out, value, 'satisfied')
                assert value['terminals'][0]['elapsed'] == {'min': 1, 'max': 1}

    def test_old_invalid_shapes_keep_diagnostic_and_location(self):
        for head in ('repeat 3 {', 'loop {'):
            with self.subTest(head=head):
                source = replace(BASE, 'repeat rework at_most 3 {', head)
                value = refusal(source, 'E-SYN-0', 'expected =')
                d = next(d for d in value['diagnostics'] if d['code'] == 'E-SYN-0')
                # Fixed by token location in this handwritten input, not by
                # capturing the current report. The offending token follows
                # the leading identifier, where the old binding expects '='.
                offset = source.index(head) + len(head.split()[0]) + 1
                assert d['span'] == {'start': offset, 'end': offset + 1}, d
                assert d['message'] == 'expected =' and d['severity'] == 'error', d

    def test_existing_golden_bytes(self):
        result = subprocess.run([sys.executable, '-B', 'scripts/snapshots.py', '--binary', BINARY], text=True, capture_output=True, cwd=ROOT)
        assert result.returncode == 0, result.stdout + result.stderr

    def test_oracles_reject_corrupt_observations(self):
        # GREEN before repeat exists; calibrate the assertions, not just parser errors.
        out, value = invoke(expanded.source('L2-clock'))
        verdict(out, value, 'inconclusive')
        hand.check_clock(value)
        wrong = deepcopy(value)
        wrong['verdict'] = 'satisfied'
        with self.assertRaises(AssertionError):
            hand.check_clock(wrong)
        out, value = invoke(expanded.source('L1-FFF'), 'explain')
        verdict(out, value, 'violated')
        hand.check_explain(value)
        wrong = deepcopy(value)
        for trace in wrong['explanation']['counterexamples']:
            for step in trace['steps']:
                if any(e.get('kind') == 'start' and e.get('actor') == 'Rev' for e in step['events']):
                    step['time'] = 6
        with self.assertRaises(AssertionError):
            hand.check_explain(wrong)


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--binary', required=True)
    parser.add_argument('--expanded', action='store_true')
    args, rest = parser.parse_known_args()
    BINARY = str(Path(args.binary).resolve())
    EXPANDED = args.expanded
    (ROOT / '.tmp').mkdir(exist_ok=True)
    unittest.main(argv=[sys.argv[0], *rest], verbosity=2)
