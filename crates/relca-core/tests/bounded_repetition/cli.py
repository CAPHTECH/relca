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
            'fork': 'fork f { x = A.c(); y = A.c() } join all f; again',
            'condition_wait': 'wait until flag == true; r = A.c(); again',
            'response_loss': 'r = A.c() response_loss sequence [delivered]; again',
            'retry': 'r = A.c() response_loss none retry max_attempts 2 when result [fail] backoff fixed 1min exhausted end limit; again',
            'receipt_handler': 'r = A.c() response_loss sequence [unknown] on receipt_unknown { stop receipt_unknown }; again',
            'degrade': 'r = A.c(); degrade Manual when r == pass { wait 1min; again } else { wait 1min; leave }',
        }
        for name, body in bodies.items():
            with self.subTest(name=name):
                source = append(BASE, 'observed case { flag: bool = true }') if name == 'condition_wait' else BASE
                refusal(replace(source, 'r = A.c(); again', body), 'U-SEM-0', 'loop_feature_combination')

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


class Gates(unittest.TestCase):
    def test_compare_diff_mutate_conform(self):
        source = replace(BASE, 'process P {', 'process P {')
        source = append(source, 'configuration AsIs { cut C; process P }\nconfiguration ToBe { cut C; process P }')
        for command, args, code, reason in [
            ('compare', ('--plans', 'AsIs,ToBe', '--mode', 'exhaustive'), 'U-SEM-0', 'loop_feature_combination'),
            ('diff', ('--from', 'AsIs', '--to', 'ToBe'), 'U-SEM-0', ''),
            ('mutate', (), 'U-SEM-0', ''),
            ('conform', ('--configuration', 'AsIs', '--log', '@events.jsonl', '--scope', 'event-contracts'), 'U-LOG-1', ''),
        ]:
            with self.subTest(command=command):
                selected = BASE if command == 'mutate' else source
                value = refusal(selected, code, reason, command, args, {'events.jsonl': ''})
                assert any(d['message'] for d in value['diagnostics'] if d['code'] == code)
                assert any('loop' in d['message'] or 'repeat' in d['message'] for d in value['diagnostics'] if d['code'] == code), value['diagnostics']

    def test_sensitivity(self):
        source = (DATA / 'models/L2.relca').read_text()
        manifest = {'schema': 'relca-sensitivity/v1', 'scope': {'kind': 'enumerated_models', 'rationale': 'bounded loop gate'},
                    'points': [{'id': 'loop', 'model': 'model.relca', 'configuration': None, 'scenario': None}],
                    'targets': [{'kind': 'probability_query', 'id': 'probability reached #1'}], 'confidence': None}
        with tempfile.TemporaryDirectory(prefix='bounded-sensitivity-', dir=ROOT / '.tmp') as d:
            p = Path(d)
            (p / 'model.relca').write_text(source)
            (p / 'manifest.json').write_text(json.dumps(manifest))
            out = subprocess.run([BINARY, 'sensitivity', str(p / 'manifest.json'), '--mode', 'exhaustive'], text=True, capture_output=True, cwd=ROOT)
            value = json.loads(out.stdout)
            verdict(out, value, 'unsupported')
            assert 'U-SEM-0' in out.stderr and 'loop' in out.stderr, (out.stderr, value)

    def test_quantile(self):
        source = replace(BASE, 'r = A.c(); again', 'r = A.c(); leave')
        source = append(source, 'policy fifo tie_by case_id\nproperty case.Done = reached(end.done)\nmetric quantile.Median = quantile(0.5, case.Done)')
        value = refusal(source, 'U-SEM-0')
        assert any('loop' in d['message'] or 'repeat' in d['message'] for d in value['diagnostics'] if d['code'] == 'U-SEM-0')

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
