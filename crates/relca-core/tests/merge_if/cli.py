"""ADR 0069 stage-1 acceptance through the real CLI, with handwritten oracles.

python3 -B cli.py --binary target/debug/relca [--witnesses] [Class.test_name]
--witnesses selects old-DSL inputs for behavior, join-all and simulation calibration.
Refusals and preservation always use their original inputs. No xfail/ignore.
"""
import argparse
from concurrent.futures import ThreadPoolExecutor
from copy import deepcopy
from fractions import Fraction as F
import importlib.util
import json
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import unittest

DATA = Path(__file__).resolve().parent
ROOT = DATA.parents[3]
BINARY = ''
WITNESSES = False
MERGE = 'd2 = if r == fail { f = Clerk.fix(d); yield f } else { yield d }'


def source(name, witness=None):
    witness = WITNESSES if witness is None else witness
    return (DATA / ('witnesses' if witness else 'models') / (name + '.relca')).read_text()


def replace(s, old, new):
    assert s.count(old) == 1, (old, s.count(old))
    return s.replace(old, new)


def append(s, text):
    i = s.rfind('}')
    return s[:i] + '\n' + text + '\n' + s[i:]


def no_deadline(s):
    return replace(s, '  contract Deadline { response case.received -> one_of(end.done_fixed,end.done_as_is) by 4min after case.received otherwise unresolved }\n', '')


def recovery_source(handler):
    base = no_deadline(source('M1', False))
    start = base.index('  process Flow {')
    end = base.index('  cut C {')
    body = '  process Flow { d = Clerk.write(); r = Rev.review(d); pay = Psp.charge(case.id) response_loss sequence [unknown] on receipt_unknown { ' + handler + ' }; end done_fixed }\n'
    s = base[:start] + body + base[end:]
    s = append(s, '''type Receipt = enum { charged, duplicate }
actor Psp { capacity 1; state keys: set<CaseId>
 capability charge(key:CaseId) -> Receipt { idempotency key key; commits; writes keys; duration fixed 1min;
 atomic { if key in keys { result duplicate } else { keys += key; result charged } }; cancel forbidden_after_start }
}''')
    return replace(s, 'include [Invoice, Manual, Clerk, Rev, Ledger]', 'include [Invoice, Manual, Clerk, Rev, Ledger, Psp]')


def contest_source(witness=False):
    s = append(source('M1', witness), 'actor Party { control party; standing over [end.done_fixed] basis "claim" }\ncontract Appeal { contest Party on end.done_fixed route Rev.review reversible_by [judge] respond_by 2min }')
    s = replace(s, 'actor Rev { capacity 1;', 'people [judge]\n  actor Rev { executors [judge];')
    s = replace(s, 'capability review(d:D)', 'capability review()')
    s = replace(s, 'r = Rev.review(d)', 'r = Rev.review()')
    return replace(s, 'include [Invoice, Manual, Clerk, Rev, Ledger]', 'include [Invoice, Manual, Clerk, Rev, Ledger, Party]')


def invoke(s, command='check', args=(), files=None, manifest=None):
    with tempfile.TemporaryDirectory(prefix='merge-if-', dir=ROOT / '.tmp') as tmp:
        p = Path(tmp)
        (p / 'model.relca').write_text(s)
        for name, content in (files or {}).items():
            (p / name).write_text(content)
        path = p / 'model.relca'
        if manifest is not None:
            path = p / 'manifest.json'
            path.write_text(json.dumps(manifest))
        args = [str(p / a[1:]) if a.startswith('@') else a for a in args]
        out = subprocess.run([BINARY, command, str(path), *args], cwd=ROOT,
                             capture_output=True, text=True, check=False)
        assert out.stdout.count('\n') == 1, (command, out.returncode, out.stdout, out.stderr)
        return out, json.loads(out.stdout)


def verdict(out, value, expected):
    code = {'satisfied': 0, 'violated': 1, 'inconclusive': 2,
            'invalid': 3, 'unsupported': 4, 'vacuous': 5}[expected]
    assert out.returncode == code, (expected, out.returncode, value.get('diagnostics'), out.stderr)
    assert value['verdict'] == expected, (expected, value)


def diagnostics(value):
    if isinstance(value, dict):
        if 'code' in value and 'severity' in value:
            yield value
        for v in value.values():
            yield from diagnostics(v)
    elif isinstance(value, list):
        for v in value:
            yield from diagnostics(v)


def check_refusal(out, value, code, reason='', gate=False):
    verdict(out, value, 'invalid' if code.startswith('E-') else 'unsupported')
    rows = [d for d in diagnostics(value) if d['code'] == code and d['severity'] == 'error']
    assert rows and any(reason in d['message'] for d in rows), (code, reason, rows, list(diagnostics(value)))
    assert code in out.stderr, out.stderr
    assert not any(d['code'] in ('E-SYN-0', 'U-SYN-0') for d in diagnostics(value)), rows
    if gate:
        assert any('merge' in d['message'].lower() or '合流' in d['message'] for d in rows), rows


def refusal(s, code, reason='', command='check', args=(), files=None, manifest=None, gate=False):
    out, value = invoke(s, command, args, files, manifest)
    check_refusal(out, value, code, reason, gate)
    if command == 'check':
        for connected in ('explain', 'simulate'):
            text = append(s, 'simulation replications 4 seed 7 confidence 0.5') if connected == 'simulate' else s
            extra_out, extra_value = invoke(text, connected)
            check_refusal(extra_out, extra_value, code, reason, gate)
    return value


def probability(row, lo, hi=None):
    assert F(row['min']) == F(lo), (row, lo)
    assert F(row['max']) == F(lo if hi is None else hi), (row, hi)


def scalar(row, lo, hi=None):
    assert row['reason'] is None, row
    probability(row, lo, hi)


TERMINALS = {
    'M1': {'done_fixed': (5, 5, '1/4', '1/4'), 'done_as_is': (3, 3, '3/4', '3/4')},
    'M2': {'rejected': (2, 2, '1/4', '1/4'), 'done_fixed': (5, 5, '1/4', '1/4'), 'done_as_is': (3, 3, '1/2', '1/2')},
    'M3': {'done': (4, 6, '0', '1'), 'held': (3, 5, '0', '1')},
    'M4': {'done': (3, 5, '19/20', '19/20'), 'flagged': (3, 3, '1/20', '1/20')},
    'M5': {'done_fixed': (5, 5, '1/4', '1/4'), 'done_as_is': (3, 3, '3/4', '3/4')},
}


def check_terminals(name, rows, arrival=None):
    expected = TERMINALS[name]
    assert len(rows) == len(expected), (name, rows)
    assert {r['end'] for r in rows} == set(expected), (name, rows)
    for row in rows:
        lo, hi, p, q = expected[row['end']]
        assert row['elapsed'] == {'min': lo, 'max': hi}, row
        assert row['work'] == {'min': lo, 'max': hi}, row
        assert row['result_at'] is None, row
        probability(row['probability'], p, q)
        if arrival is not None:
            assert row['kind'] == 'end', row
            assert row['completed_at'] == {'min': arrival + lo, 'max': arrival + hi}, row


def check_hand(name, value):
    assert value['verdict'] == ('violated' if name == 'M1' else 'satisfied'), value['diagnostics']
    assert value['bounds']['truncated'] is False
    if name == 'M5':
        cases = value['case_outcomes']
        assert [(c['case_id'], c['arrival']) for c in cases] == [(1, 0), (2, 10)], cases
        for case in cases:
            check_terminals(name, case['terminals'], case['arrival'])
    else:
        check_terminals(name, value['terminals'])
    ps = [p for p in value['properties'] if 'probability' in p]
    if name == 'M5':
        ps = [case for p in value['properties'] for case in p.get('cases', []) if 'probability' in case]
    expected = {'M1': [('1/4', '1/4')], 'M2': [('1/4', '1/4'), ('1/2', '1/2'), ('1/4', '1/4')],
                'M3': [('0', '1'), ('0', '1')], 'M4': [('1/20', '1/20')], 'M5': [('1/4', '1/4'), ('1/4', '1/4')]}[name]
    assert len(ps) == len(expected), ps
    for p, (lo, hi) in zip(ps, expected):
        assert p['verdict'] == 'satisfied', p
        probability(p['probability'], lo, hi)
    metrics = value['metrics']
    assert metrics['status'] == 'complete', metrics
    work, high, cost, cost_high = {'M1': ('3', '5', '6', '9'),
                                  'M2': ('2', '5', '4', '9'),
                                  'M3': ('3', '6', '6', '11'),
                                  'M4': ('3', '5', '6', '9'),
                                  'M5': ('3', '5', '6', '9')}[name]
    for case in metrics['cases']:
        scalar(case['work'], work, high)
        scalar(case['observed_cost_total'], cost, cost_high)
        for task in case['tasks']:
            scalar(task['wait'], 0)
        if name != 'M3':
            for row in case['terminals']:
                if row['count']['stats']['max'] == '0':
                    continue
                b = row['breakdown']
                assert b['in_service'] == row['lead_time'], row
                for field in ('external', 'intentional', 'contest_route', 'unattributed'):
                    scalar(b[field], 0)
                scalar(b['queue']['combined'], 0)


def check_merge_lineage(value):
    a = value['agency']
    rows = a['lineage']
    for call, independent in [('r', []), ('f', ['Manual'])]:
        row = [r for r in rows if r['call'] == call]
        assert len(row) == 1, (call, rows)
        assert row[0]['subject_sources'] == ['Invoice'], row
        assert row[0]['independent_sources'] == independent, row
    assert not any(r['call'] in ('a', 'd2', 'later') for r in rows), rows
    assert unchecked_call(a['unchecked'], 'a'), a['unchecked']


def unchecked_call(marks, call):
    token = r'(?<![A-Za-z0-9_])' + re.escape(call) + r'(?![A-Za-z0-9_])'
    return any('lineage(merge_control)' in mark and re.search(token, mark) for mark in marks)


def check_agency(value):
    check_merge_lineage(value)
    a = value['agency']
    for code in ('W-AGY-0', 'W-AGY-2', 'W-LIN-2', 'W-RESP-2'):
        assert not any(d['code'] == code for d in value['diagnostics']), value['diagnostics']
        assert any(code in mark and 'merge_control' in mark for mark in a['unchecked']), (code, a['unchecked'])
    assert a['viewpoints']['framing']['status'] == 'not_derived', a['viewpoints']['framing']
    assert a['viewpoints']['framing']['reason'] == 'merge_control', a['viewpoints']['framing']
    origin = next(r for r in a['origin'] if r['actor'] == 'Clerk')
    assert origin['direct'] == ['Invoice', 'Manual'], origin
    effects = a['viewpoints']['effects']['rows']
    send = next(r for r in effects if r['call'] == 's')
    assert send['e']['actors'] == ['Ledger'], send
    assert send.get('u') is None, send
    assert send['process_judgements'] == {'status': 'not_derived', 'reason': 'merge_control'}, send


def check_verified(trace):
    assert trace['replay']['status'] == 'verified', trace['replay']


def check_explanation(s, value):
    traces = value['explanation']['counterexamples']
    assert traces, value['explanation']
    deadline = [t for t in traces if 'Deadline' in t['id']]
    assert len(deadline) == 1, traces
    trace = deadline[0]
    check_verified(trace)
    merged = [step for step in trace['steps'] if step['kind'] == 'merged']
    assert len(merged) == 1, merged
    step = merged[0]
    assert step['time'] == 4 and step['binding'] == 'd2', step
    assert step['source'] == 'f', step
    assert step['case_id'] in (None, 1), step
    # 751 bytes in lines 1..16 + 42 bytes before yield; yield f is 7 bytes.
    # syntax::Parser::span excludes separators and ends after the f token.
    assert s.encode()[793:800] == b'yield f'
    assert step['location']['span'] == {'start': 793, 'end': 800}, step['location']
    # Match the existing Stage's required nullable Task columns, not omissions.
    for field in ('actor', 'capability', 'executor'):
        assert step[field] is None, step
    starts = [(step['time'], e['actor'], e['capability'], e['binding'])
              for step in trace['steps'] for e in step['events'] if e.get('kind') == 'start']
    assert starts == [(0, 'Clerk', 'write', 'd'), (1, 'Rev', 'review', 'r'),
                      (2, 'Clerk', 'fix', 'f'), (4, 'Ledger', 'send', 's')], starts


class Behavior(unittest.TestCase):
    def test_m1(self):
        out, value = invoke(source('M1'))
        verdict(out, value, 'violated')
        check_hand('M1', value)
        if not WITNESSES:
            tasks = value['metrics']['cases'][0]['tasks']
            assert {t['binding'] for t in tasks} == {'d', 'r', 'f', 's'}
            scalar(next(t['starts'] for t in tasks if t['binding'] == 'f'), 0, 1)
            scalar(next(t['starts'] for t in tasks if t['binding'] == 's'), 1)

    def test_m2(self):
        out, value = invoke(source('M2'))
        verdict(out, value, 'satisfied')
        check_hand('M2', value)

    def test_m3(self):
        out, value = invoke(source('M3'))
        verdict(out, value, 'satisfied')
        check_hand('M3', value)
        if WITNESSES:
            rows = value['agency']['lineage']
            assert next(r for r in rows if r['call'] == 'a1')['subject_sources'] == ['Invoice', 'Manual']
            assert next(r for r in rows if r['call'] == 'a1')['independent_sources'] == []
            assert next(r for r in rows if r['call'] == 'a2')['subject_sources'] == ['Invoice']
            assert next(r for r in rows if r['call'] == 'a2')['independent_sources'] == ['Manual']
        else:
            check_agency(value)

    def test_m4_quality(self):
        out, value = invoke(source('M4'))
        verdict(out, value, 'satisfied')
        check_hand('M4', value)
        row = next(r for r in value['quality']['cohort'] if r['name'] == 'FlaggedPerArrival')
        assert row['value'] == {'lower': '1/20', 'upper': '1/20', 'basis': 'exact', 'reason': None}, row

    def test_m5(self):
        out, value = invoke(source('M5'))
        verdict(out, value, 'satisfied')
        check_hand('M5', value)


class Connected(unittest.TestCase):
    def test_explain_merged_and_independent_verified_replay(self):
        s = source('M1', False)
        out, value = invoke(s, 'explain')
        verdict(out, value, 'violated')
        check_hand('M1', value)
        check_explanation(s, value)

    def test_transitive_lineage_is_not_claimed(self):
        s = source('M3', False)
        s = replace(s, 'actor Auditor { capacity 1;', 'actor Auditor { capacity 1; capability follow(v:V) -> V { reads [Manual]; duration fixed 1min; result nondet { pass, fail } };')
        s = replace(s, 'if a == pass { s = Ledger.send(d2); end done } else { end held }',
                    'later = Auditor.follow(a); if a == pass && later == pass { s = Ledger.send(d2); end done } else { end held }')
        out, value = invoke(s)
        verdict(out, value, 'satisfied')
        check_agency(value)
        assert unchecked_call(value['agency']['unchecked'], 'later')

    def test_simulate_preserves_sample_safety_and_task_inventory(self):
        for outcome, end, elapsed in [('pass', 'done_as_is', 3), ('fail', 'done_fixed', 5)]:
            with self.subTest(outcome=outcome):
                s = no_deadline(source('M1'))
                s = replace(s, '0.75: pass; 0.25: fail', '1: ' + outcome)
                s = replace(s, 'verify probability reached end.done_fixed >= 0.25', '')
                s = append(s, 'simulation replications 4 seed 7 confidence 0.5')
                out, value = invoke(s, 'simulate')
                verdict(out, value, 'inconclusive')
                safety = next(p for p in value['properties'] if p['id'] == 'safety no_resource_overcommit')
                assert safety['verdict'] == 'inconclusive'
                assert value['simulation']['completed_replications'] == 4
                case = value['case_outcomes'][0]
                assert case['unfinished_count'] == 0
                assert case['terminals'] == [{'kind': 'end', 'end': end, 'count': 4}], case
                metrics = value['metrics']['cases'][0]
                assert metrics['work']['mean'] == str(elapsed), metrics
                expected = {'d', 'r', 'f', 's1', 's2'} if WITNESSES else {'d', 'r', 'f', 's'}
                assert {t['binding'] for t in metrics['tasks']} == expected

    def test_simulate_m3_unresolved_nondeterminism_stays_closed(self):
        refusal(append(source('M3', False), 'simulation replications 4 seed 7 confidence 0.5'),
                'U-SEM-0', command='simulate')

    def test_simulate_quality_observes_post_fix_state(self):
        s = source('M4')
        s = replace(s, 'bad: bool ~ { true: 0.2, false: 0.8 }', 'bad: bool = true')
        s = replace(s, 'true -> { 0.75: fail; 0.25: pass }', 'true -> { 1: fail }')
        s = replace(s, 'verify probability reached end.flagged <= 0.05', '')
        s = append(s, 'simulation replications 4 seed 7 confidence 0.5')
        out, value = invoke(s, 'simulate')
        verdict(out, value, 'inconclusive')
        assert value['case_outcomes'][0]['terminals'] == [{'kind': 'end', 'end': 'done', 'count': 4}]
        assert value['metrics']['cases'][0]['work']['mean'] == '5'
        quality = value['quality']
        assert quality['analysis'] == 'simulation'
        flagged = next(p for p in quality['properties'] if p['name'] == 'Flagged')
        assert flagged['cases'] == [{'case_id': 1, 'arrival': 0, 'hits': 0, 'unknown': 0, 'trials': 4}]
        rate = next(r for r in quality['cohort'] if r['name'] == 'FlaggedPerArrival')
        assert rate['value']['point'] == '0' and rate['value']['samples'] == 4

    def test_simulate_arrivals_keep_per_case_result(self):
        s = source('M5')
        s = replace(s, '0.75: pass; 0.25: fail', '1: pass')
        s = replace(s, 'verify probability reached end.done_fixed >= 0.25', '')
        out, value = invoke(append(s, 'simulation replications 4 seed 7 confidence 0.5'), 'simulate')
        verdict(out, value, 'inconclusive')
        assert value['simulation']['completed_replications'] == 4
        for case in value['case_outcomes']:
            assert case['terminals'] == [{'kind': 'end', 'end': 'done_as_is', 'count': 4}], case
            assert case['unfinished_count'] == 0, case
        assert len(value['case_outcomes']) == 2

    def test_join_all_result_can_be_yielded(self):
        s = no_deadline(source('M1'))
        s = replace(s, 'f = Clerk.fix(d)',
                    'fork work { f = Clerk.fix(d); other = Rev.review(d) } join all work')
        out, value = invoke(s)
        verdict(out, value, 'satisfied')
        assert {row['end'] for row in value['terminals']} == {'done_fixed', 'done_as_is'}
        for row in value['terminals']:
            fixed = row['end'] == 'done_fixed'
            elapsed, work, mass = (5, 6, '1/4') if fixed else (3, 3, '3/4')
            assert row['elapsed'] == {'min': elapsed, 'max': elapsed}, row
            assert row['work'] == {'min': work, 'max': work}, row
            probability(row['probability'], mass)

    def test_merge_before_repeat_is_fixed_input(self):
        s = no_deadline(source('M1', False))
        s = replace(s, 's = Ledger.send(d2)',
                    'repeat dispatch at_most 1 { s = Ledger.send(d2); leave } on_limit { end limit }')
        out, value = invoke(s)
        verdict(out, value, 'satisfied')
        check_terminals('M1', value['terminals'])


class Refusals(unittest.TestCase):
    def test_b01_missing_yield(self):
        refusal(replace(source('M1', False), 'yield f', ''), 'E-CON-0', 'merge_fallthrough')

    def test_b02_wrong_type(self):
        refusal(replace(source('M1', False), 'yield d }', 'yield r }'), 'E-TYPE-0', 'merge_type_mismatch')

    def test_b03_undefined_source(self):
        refusal(replace(source('M1', False), 'yield f', 'yield g'), 'E-NAME-0')

    def test_b04_cancelled_branch(self):
        s = replace(source('M1', False), 'f = Clerk.fix(d); yield f',
                    'fork work { f = Clerk.fix(d); other = Clerk.fix(d) } join any work where result == repaired tie by [f,other] remaining cancel_then_drain if none -> done_as_is; yield other')
        s = replace(s, 'duration fixed 2min; result nondet { repaired }', 'duration fixed 2min; result nondet { repaired }; cancel cooperative ack_within 1min')
        refusal(s, 'U-SEM-0', 'result is not definitely available (not yet defined or possibly cancelled)')

    def test_b05_latent_source(self):
        refusal(replace(source('M4', False), 'yield f', 'yield bad'), 'E-POL-LATENT')

    def test_b06_observed_source(self):
        s = append(source('M1', False), 'observed case { doc: optional<D> }')
        refusal(replace(s, 'yield f', 'yield doc'), 'U-SEM-0', 'merge_source_unsupported')

    def test_b07_unreachable_after_yield(self):
        refusal(replace(source('M1', False), 'yield f', 'yield f; wait 1min'), 'E-CON-0', 'merge_unreachable_after_yield')

    def test_b08_repeat_body(self):
        s = replace(source('M1', False), MERGE,
                    'repeat rework at_most 1 { ' + MERGE + '; wait 1min; leave } on_limit { end limit }')
        refusal(s, 'U-SEM-0', 'loop_feature_combination')

    def test_b09_retry_source(self):
        s = replace(source('M1', False), 'f = Clerk.fix(d); yield f',
                    'f = Clerk.fix(d) response_loss none retry max_attempts 2 when result [draft] backoff fixed 1min exhausted end exhausted; yield f')
        refusal(s, 'U-SEM-0', 'merge_feature_combination')

    def test_b10_duplicate_binding(self):
        refusal(source('M1', False).replace('d2', 'd'), 'U-SEM-0', 'duplicate or shadowed binding')

    def test_b11_branch_local_does_not_escape(self):
        refusal(replace(source('M1', False), 'Ledger.send(d2)', 'Ledger.send(f)'), 'U-SEM-0', 'argument result is not definitely available')

    def test_b12_all_end(self):
        s = replace(source('M1', False), MERGE, 'd2 = if r == fail { end done_fixed } else { end done_as_is }')
        refusal(s, 'E-CON-0', 'merge_no_continuing_branch')

    def test_b13_statement_yield(self):
        s = replace(source('M1', False), MERGE, 'if r == fail { f = Clerk.fix(d); yield f } else { }')
        refusal(s, 'E-CON-0', 'merge_yield_outside')

    def test_b15_contract_reads_merge(self):
        s = append(source('M1', False), 'contract Value { invariant Ledger.send.finish -> d2 == repaired }')
        refusal(s, 'U-SEM-0', 'merge_feature_combination')

    def test_b16_latent_condition(self):
        refusal(replace(source('M4', False), 'if r == fail', 'if bad == true'), 'E-POL-LATENT')

    def test_constant_source(self):
        refusal(replace(source('M1', False), 'yield f', 'yield repaired'), 'U-SEM-0', 'merge_source_unsupported')

    def test_repeat_limit(self):
        s = replace(source('M1', False), MERGE,
                    'repeat rework at_most 1 { wait 1min; again } on_limit { ' + MERGE + '; leave }')
        refusal(s, 'U-SEM-0', 'loop_feature_combination')

    def test_repeat_public_source(self):
        s = replace(source('M1', False), MERGE,
                    'repeat rework at_most 1 { f = Clerk.fix(d); leave } on_limit { end limit }; d2 = if r == fail { yield f } else { yield d }')
        refusal(s, 'U-SEM-0', 'merge_feature_combination')

    def test_response_alias_source(self):
        s = replace(source('M1', False), 'f = Clerk.fix(d); yield f',
                    'f = Clerk.fix(d) response_loss none; yield f')
        refusal(s, 'U-SEM-0', 'merge_feature_combination')

    def test_recovery_alias_source(self):
        s = recovery_source('stop receipt_unknown')
        s = replace(s, '; end done_fixed }', '; merged = if r == fail { yield pay } else { yield pay }; end done_fixed }')
        refusal(s, 'U-SEM-0', 'merge_feature_combination')

    def test_recovery_handler(self):
        s = recovery_source(MERGE + '; stop receipt_unknown')
        refusal(s, 'U-SEM-0', 'merge_feature_combination')

    def test_requires_reads_merge(self):
        s = replace(source('M1', False), 'commits; duration fixed 1min',
                    'requires d2 == repaired else sent; commits; duration fixed 1min')
        refusal(s, 'U-SEM-0', 'merge_feature_combination')

    def test_property_reads_merge(self):
        s = append(source('M1', False), 'property case.Fixed = reached(d2 == repaired)')
        refusal(s, 'U-SEM-0', 'merge_feature_combination')

    def test_metric_via_property_reads_merge(self):
        s = append(source('M1', False), 'property case.Fixed = reached(d2 == repaired)\nmetric cohort.FixedRate = expected(count(case.Fixed)) / arrivals')
        refusal(s, 'U-SEM-0', 'merge_feature_combination')

    def test_join_where_reads_merge(self):
        s = replace(source('M1', False), 's = Ledger.send(d2)',
                    'fork work { one = Clerk.fix(d); two = Clerk.fix(d) } join any work where result == d2 tie by [one,two] remaining drain if none -> done_as_is; s = Ledger.send(d2)')
        refusal(s, 'U-SEM-0', 'merge_feature_combination')

    def test_same_shape_different_nominal_type(self):
        s = append(source('M1', False), 'type Other = enum { draft, repaired }\nactor OtherSource { capacity 1; capability make() -> Other { duration fixed 1min; result nondet { draft } } }')
        s = replace(s, 'yield d }', 'other = OtherSource.make(); yield other }')
        s = replace(s, 'include [Invoice, Manual, Clerk, Rev, Ledger]', 'include [Invoice, Manual, Clerk, Rev, Ledger, OtherSource]')
        refusal(s, 'E-TYPE-0', 'merge_type_mismatch')

    def test_yield_in_plain_inner_if_is_not_outer_closure(self):
        s = replace(source('M1', False), 'f = Clerk.fix(d); yield f',
                    'f = Clerk.fix(d); if r == fail { yield f } else { yield d }')
        refusal(s, 'E-CON-0', 'merge_yield_outside')

    def test_top_level_yield_is_outside(self):
        s = replace(PLAIN, 'd = A.c(); end done', 'd = A.c(); yield d; end done')
        refusal(s, 'E-CON-0', 'merge_yield_outside')


class Gates(unittest.TestCase):
    def test_b14_compare(self):
        s = append(source('M1', False), 'configuration AsIs { cut C; process Flow }\nconfiguration ToBe { cut C; process Flow }')
        refusal(s, 'U-SEM-0', 'merge_feature_combination', 'compare',
                ('--plans', 'AsIs,ToBe', '--mode', 'exhaustive'), gate=True)

    def test_b14_diff(self):
        s = append(source('M1', False), 'configuration AsIs { cut C; process Flow }\nconfiguration ToBe { cut C; process Flow }')
        refusal(s, 'U-SEM-0', 'merge_feature_combination', 'diff', ('--from', 'AsIs', '--to', 'ToBe'), gate=True)

    def test_b14_mutate(self):
        refusal(source('M1', False), 'U-SEM-0', 'merge_feature_combination', 'mutate', gate=True)

    def test_b14_conform(self):
        refusal(source('M1', False), 'U-LOG-1', command='conform',
                args=('--log', '@events.jsonl', '--scope', 'event-contracts'), files={'events.jsonl': ''}, gate=True)

    def test_b14_sensitivity(self):
        manifest = {'schema': 'relca-sensitivity/v1',
                    'scope': {'kind': 'enumerated_models', 'rationale': 'merge gate'},
                    'points': [{'id': 'merge', 'model': 'model.relca', 'configuration': None, 'scenario': None}],
                    'targets': [{'kind': 'probability_query', 'id': 'probability reached #1'}], 'confidence': None}
        refusal(source('M1', False), 'U-SEM-0', 'merge_feature_combination', 'sensitivity',
                ('--mode', 'exhaustive'), manifest=manifest, gate=True)

    def test_quantile(self):
        s = append(source('M1', False), 'policy fifo tie_by case_id\nproperty case.Done = reached(end.done_fixed)\nmetric quantile.Median = quantile(0.5, case.Done)')
        refusal(s, 'U-SEM-0', 'merge_feature_combination', gate=True)

    def test_contest(self):
        refusal(contest_source(), 'U-SEM-0', 'merge_feature_combination', gate=True)


PLAIN = '''model Old version "0.2" {
type D = enum { draft, repaired }
actor A { capacity 1; capability c() -> D { duration fixed 1min; result nondet { draft } } }
process P { d = A.c(); end done }
cut C { include [A] }
bounds { cases 1; live_tasks 2; clock 10min }
verify safety no_resource_overcommit
}'''


class Preservation(unittest.TestCase):
    def test_yield_remains_a_binding_and_argument_name(self):
        for word in ('yield', 'merge', 'merged'):
            with self.subTest(word=word):
                s = replace(PLAIN, 'd = A.c(); end done',
                            f'{word} = A.c(); if {word} == draft {{ end done }} else {{ end wrong }}')
                out, value = invoke(s)
                verdict(out, value, 'satisfied')
                assert value['terminals'] == [{'end': 'done', 'elapsed': {'min': 1, 'max': 1},
                                               'work': {'min': 1, 'max': 1}, 'result_at': None}]

    def test_yield_remains_actor_capability_parameter_and_argument_name(self):
        s = append(PLAIN, 'actor yield { capacity 1; capability yield(yield:D) -> D { duration fixed 1min; result nondet { draft } } }')
        s = replace(s, 'd = A.c(); end done', 'd = A.c(); yield = yield.yield(d); if yield == draft { end done } else { end wrong }')
        s = replace(s, 'include [A]', 'include [A, yield]')
        out, value = invoke(s)
        verdict(out, value, 'satisfied')
        assert value['terminals'][0]['elapsed'] == {'min': 2, 'max': 2}

    def test_old_unavailable_and_duplicate_diagnostics(self):
        for body, reason in [
            ('d = A.c(); if d == draft { f = A.c() } else { }; g = A.use(f); end done', 'argument result is not definitely available'),
            ('d = A.c(); if d == draft { f = A.c() } else { f = A.c() }; end done', 'duplicate or shadowed binding'),
            ('d = A.c(); if d == draft { f = A.c() } else { }; if f == draft { end done } else { end wrong }', 'result is not definitely available (not yet defined or possibly cancelled)'),
        ]:
            with self.subTest(reason=reason):
                s = replace(PLAIN, 'd = A.c(); end done', body)
                s = replace(s, 'actor A { capacity 1;', 'actor A { capacity 1; capability use(d:D) -> D { duration fixed 1min; result nondet { draft } };')
                s = s.replace('result nondet { draft }', 'result nondet { draft, repaired }')
                refusal(s, 'U-SEM-0', reason)

    def test_old_assignment_error_keeps_token_location(self):
        for statement in ('d2 = d', 'd2 = merge(d, d)', 'd2 = either d or d'):
            with self.subTest(statement=statement):
                s = replace(PLAIN, 'd = A.c(); end done', 'd = A.c(); ' + statement + '; end done')
                out, value = invoke(s)
                verdict(out, value, 'invalid')
                ds = [d for d in value['diagnostics'] if d['code'] == 'E-SYN-0']
                assert len(ds) == 1 and ds[0]['message'] == 'expected .', ds
                start = s.index(statement) + len(statement.split()[0]) + 3 + len(statement.split()[2].split('(')[0])
                # Whitespace is skipped before the expected-dot token.
                while s[start].isspace():
                    start += 1
                assert ds[0]['span']['start'] == start, (statement, start, ds)
                assert ds[0]['span']['end'] == start + 1, ds

    def test_old_latent_condition_is_still_invalid(self):
        s = replace(source('M4', True), 'if r == fail', 'if bad == true')
        refusal(s, 'E-POL-LATENT')

    def test_old_repeat_alias_and_duplicate_names(self):
        s = replace(PLAIN, 'd = A.c(); end done',
                    'repeat rework at_most 1 { d = A.c(); leave } on_limit { end limit }; if d == draft { end done } else { end wrong }')
        out, value = invoke(s)
        verdict(out, value, 'satisfied')
        assert value['terminals'][0]['end'] == 'done'
        s = replace(s, 'on_limit { end limit }', 'on_limit { d = A.c(); leave }')
        refusal(s, 'U-SEM-0', 'duplicate or shadowed binding')

    def test_old_observed_field_is_not_an_argument_binding(self):
        s = append(PLAIN, 'observed case { doc: optional<D> }')
        s = replace(s, 'actor A { capacity 1;', 'actor A { capacity 1; capability fix() -> D { writes doc; duration fixed 1min; result nondet { repaired }; finish { doc = repaired } }; capability keep() -> D { writes doc; duration fixed 1min; result nondet { draft }; finish { doc = draft } }; capability use(d:D) -> D { duration fixed 1min; result nondet { draft } };')
        s = replace(s, 'd = A.c(); end done', 'd = A.c(); if d == draft { f = A.fix() } else { k = A.keep() }; if doc == repaired { end done } else { end wrong }')
        out, value = invoke(s)
        verdict(out, value, 'satisfied')
        assert value['terminals'][0]['end'] == 'done'
        s = replace(s, 'if doc == repaired { end done } else { end wrong }', 'g = A.use(doc); end done')
        refusal(s, 'E-NAME-0', 'undefined argument binding')

    def test_existing_golden_stdout_stderr_exit_bytes(self):
        # Reuse only the framing/invocation utilities. This new test executes no
        # existing test suite and never updates a snapshot or learns a hand value.
        spec = importlib.util.spec_from_file_location('merge_snapshots', ROOT / 'scripts/snapshots.py')
        assert spec is not None and spec.loader is not None
        snapshots = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(snapshots)
        jobs = json.loads((snapshots.SAVED / 'jobs.json').read_bytes())
        import gzip
        def compare(job):
            paths = [snapshots.SAVED / (job['id'] + suffix) for suffix in ('.snap', '.snap.gz')]
            paths = [p for p in paths if p.is_file()]
            assert len(paths) == 1, job
            raw = paths[0].read_bytes()
            expected = snapshots.unpack(gzip.decompress(raw) if paths[0].suffix == '.gz' else raw)
            actual = snapshots.capture(Path(BINARY), job)
            return snapshots.compare(job, expected, actual)
        with ThreadPoolExecutor(max_workers=4) as pool:
            for job, errors in zip(jobs, pool.map(compare, jobs)):
                with self.subTest(job=job['id']):
                    assert not errors, errors


class Prerequisites(unittest.TestCase):
    def test_old_dsl_contest_scaffold(self):
        out, value = invoke(contest_source(True))
        assert out.returncode == 1, (out.returncode, value['diagnostics'])
        assert not any(d['severity'] == 'error' for d in value['diagnostics']), value['diagnostics']

    def test_independent_fraction_arithmetic(self):
        assert F(1, 4) * 5 + F(3, 4) * 3 == F(7, 2)
        assert F(1, 4) * 2 + F(1, 4) * 5 + F(1, 2) * 3 == F(13, 4)
        repaired = F(1, 5) * F(3, 4)
        missed = F(1, 5) * F(1, 4)
        assert repaired == F(3, 20) and missed == F(1, 20)
        assert F(4, 5) + repaired == F(19, 20)
        assert F(4, 5) * 3 + repaired * 5 + missed * 3 == F(33, 10)
        assert [a * b for a in (F(1, 4), F(3, 4)) for b in (F(1, 4), F(3, 4))] == [F(1, 16), F(3, 16), F(3, 16), F(9, 16)]
        m1 = source('M1', False).encode()
        assert [len(line) for line in m1.splitlines(keepends=True)[:16]] == [25, 36, 39, 27, 36, 33, 52, 27, 94, 97, 4, 104, 116, 17, 22, 22]
        assert m1[793:800] == b'yield f'

    def test_old_dsl_scaffolds_for_feature_combinations(self):
        # These ordinary-call models validate the syntax around a future merge.
        # They are not adapters that could make a raw new-syntax test pass.
        base = no_deadline(source('M1', False))
        start = base.index('  process Flow {')
        end = base.index('  cut C {')
        def scaffold(body, extra=''):
            s = base[:start] + '  process Flow { d = Clerk.write(); r = Rev.review(d); ' + body + '; end done_fixed }\n' + base[end:]
            return append(s, extra) if extra else s
        samples = {
            'join_any': scaffold('fork work { f = Clerk.fix(d); other = Clerk.fix(d) } join any work where result == repaired tie by [f,other] remaining cancel_then_drain if none -> done_fixed').replace('duration fixed 2min; result nondet { repaired }', 'duration fixed 2min; result nondet { repaired }; cancel cooperative ack_within 1min'),
            'join_all': scaffold('fork work { f = Clerk.fix(d); other = Rev.review(d) } join all work'),
            'retry': scaffold('f = Clerk.fix(d) response_loss none retry max_attempts 2 when result [draft] backoff fixed 1min exhausted end exhausted'),
            'response': scaffold('f = Clerk.fix(d) response_loss none'),
            'recovery': recovery_source('stop receipt_unknown'),
            'repeat_body': scaffold('repeat rework at_most 1 { f = Clerk.fix(d); leave } on_limit { end limit }'),
            'repeat_limit': scaffold('repeat rework at_most 1 { wait 1min; again } on_limit { f = Clerk.fix(d); leave }'),
            'contract': scaffold('d2 = Clerk.fix(d); s = Ledger.send(d2)', 'contract Value { invariant Ledger.send.finish -> d2 == repaired }'),
            'quantile': scaffold('d2 = Clerk.fix(d)', 'policy fifo tie_by case_id\nproperty case.Done = reached(end.done_fixed)\nmetric quantile.Median = quantile(0.5, case.Done)'),
        }
        for name, s in samples.items():
            with self.subTest(name=name):
                out, value = invoke(s)
                assert out.returncode in (0, 1), (name, out.returncode, value['diagnostics'])
                assert not any(d['severity'] == 'error' for d in value['diagnostics']), (name, value['diagnostics'])
        s = scaffold('d2 = Clerk.fix(d)', 'property case.Fixed = reached(d2 == repaired)\nmetric cohort.FixedRate = expected(count(case.Fixed)) / arrivals')
        refusal(s, 'U-SEM-0', 'probability conditions require state')


class Calibration(unittest.TestCase):
    def test_fi1_fi2_union_and_intersection_lineage_claims_are_rejected(self):
        out, value = invoke(source('M3', True))
        verdict(out, value, 'satisfied')
        # Controlled observation seam only: the compiler does not implement
        # merging yet. Keep this distinct from mutating production Agency.
        value['agency']['lineage'] = [r for r in value['agency']['lineage'] if r['call'] not in ('a1', 'a2')]
        value['agency']['unchecked'].append('lineage(merge_control): a')
        check_merge_lineage(value)
        for subjects, independent in [(['Invoice', 'Manual'], []), (['Invoice'], ['Manual'])]:
            broken = deepcopy(value)
            broken['agency']['lineage'].append({'call': 'a', 'capability': 'Auditor.audit',
                                                'subject_sources': subjects, 'independent_sources': independent})
            with self.assertRaises(AssertionError):
                check_merge_lineage(broken)
        value['agency']['unchecked'].remove('lineage(merge_control): a')
        value['agency']['unchecked'].append('lineage(merge_control)')
        with self.assertRaises(AssertionError):
            check_merge_lineage(value)

    def test_fi3_fi4_wrong_existing_source_is_rejected(self):
        s = source('M1', True)
        # Taking the old value instead of the selected fixed value is the
        # expressible dual of failing to publish the already-existing source.
        s = replace(s, 'if f == repaired', 'if d == repaired')
        out, value = invoke(s)
        verdict(out, value, 'violated')
        assert {r['end'] for r in value['terminals']} == {'done_as_is'}
        with self.assertRaises(AssertionError):
            check_hand('M1', value)

    def test_fi5_copy_time_is_rejected(self):
        s = source('M1', True).replace('s1 = Ledger.send(f)', 'wait 1min; s1 = Ledger.send(f)').replace('s2 = Ledger.send(d)', 'wait 1min; s2 = Ledger.send(d)')
        out, value = invoke(s)
        verdict(out, value, 'violated')
        assert {r['end']: r['elapsed']['min'] for r in value['terminals']} == {'done_fixed': 6, 'done_as_is': 4}
        with self.assertRaises(AssertionError):
            check_hand('M1', value)

    def test_fi6_second_case_missing_copy_is_rejected(self):
        out, value = invoke(source('M5', True))
        verdict(out, value, 'satisfied')
        check_hand('M5', value)
        value['case_outcomes'][1]['terminals'] = [r for r in value['case_outcomes'][1]['terminals'] if r['end'] != 'done_as_is']
        with self.assertRaises(AssertionError):
            check_hand('M5', value)

    def test_fi7_state_write_loss_is_rejected(self):
        s = source('M4', True).replace('with bad = false', 'with bad = true')
        out, value = invoke(s)
        verdict(out, value, 'violated')
        probability(next(r for r in value['terminals'] if r['end'] == 'flagged')['probability'], '1/5')
        with self.assertRaises(AssertionError):
            check_hand('M4', value)

    def test_fi8_unsupported_replay_is_not_a_verified_counterexample(self):
        out, value = invoke(source('M1', True), 'explain')
        verdict(out, value, 'violated')
        traces = value['explanation']['counterexamples']
        assert traces
        for trace in traces:
            check_verified(trace)
            broken = deepcopy(trace)
            broken['replay']['status'] = 'unsupported'
            with self.assertRaises(AssertionError):
                check_verified(broken)

    def test_fi9_missing_reason_is_not_accepted_as_intended_refusal(self):
        s = replace(PLAIN, 'd = A.c(); end done', 'd = A.c(); if d == draft { f = A.c() } else { }; g = A.use(f); end done').replace('actor A { capacity 1;', 'actor A { capacity 1; capability use(d:D) -> D { duration fixed 1min; result nondet { draft } };')
        s = s.replace('result nondet { draft }', 'result nondet { draft, repaired }')
        out, value = invoke(s)
        check_refusal(out, value, 'U-SEM-0', 'argument result is not definitely available')
        broken = deepcopy(value)
        for row in broken['diagnostics']:
            if row['code'] == 'U-SEM-0':
                row['message'] = 'unrelated unsupported behavior'
        with self.assertRaises(AssertionError):
            check_refusal(out, broken, 'U-SEM-0', 'argument result is not definitely available')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', required=True)
    parser.add_argument('--witnesses', action='store_true')
    opts, tests = parser.parse_known_args()
    BINARY = str(Path(opts.binary).resolve())
    WITNESSES = opts.witnesses
    unittest.main(argv=[sys.argv[0], *tests], verbosity=2)
