"""Portable behavioral tests. Only the public CLI JSON/exit protocol is used.

Run with python3 -B acceptance.py --binary /path/to/relca.
--models selects a directory containing equivalent inputs (used by the spike's
source-to-source adapter). The assertions never depend on that adapter.
"""
import argparse
from fractions import Fraction as F
import json
from pathlib import Path
import subprocess

PROBABILITIES = {
    'L2': ['1/10', '1/10'],
    'L3': ['9/10', '39/40', '159/160', '1/160'],
    'L4': ['1/2', '9/10', '99/100', '1/100'],
    'P1': ['1/2', '5/8', '3/4', '1/4'],
}
TERMINALS = {
    'L1': {'done': (1, 7, 1, 7), 'escalated': (9, 9, 9, 9)},
    'L2': {'done': (1, 1, 1, 1), 'escalated': (9, 9, 9, 9)},
    'L3': {'done': (1, 7, 1, 7), 'escalated': (9, 9, 9, 9)},
    'L4': {'done': (1, 7, 1, 7), 'escalated': (9, 9, 9, 9)},
    'P1': {'shipped': (6, 68, 6, 8), 'gave_up': (63, 63, 3, 3)},
}


def scalar(value, low, high=None):
    assert value['reason'] is None, value
    assert F(value['min']) == F(low), (value, low)
    assert F(value['max']) == F(low if high is None else high), (value, high)


def probabilities(values, expected):
    assert len(values) == len(expected), (values, expected)
    for value, fraction in zip(values, expected):
        assert F(value['min']) == F(value['max']) == F(fraction), (value, fraction)


def check_means(name, values):
    p = [F(value['min']) for value in values]
    if name == 'L2':
        masses = [1 - p[0], p[0]]
        assert sum(m * t for m, t in zip(masses, [1, 9])) == F(9, 5)
        assert sum(m * n for m, n in zip(masses, [1, 3])) == F(6, 5)
        assert sum(m * n for m, n in zip(masses, [0, 3])) == F(3, 10)
    elif name in ('L3', 'L4', 'P1'):
        masses = [p[0], p[1] - p[0], p[2] - p[1], p[3]]
        assert sum(masses) == 1
        times = [6, 37, 68, 63] if name == 'P1' else [1, 4, 7, 9]
        assert sum(m * t for m, t in zip(masses, times)) == {'L3': F(111, 80), 'L4': F(141, 50), 'P1': F(255, 8)}[name]
        if name == 'P1':
            assert sum(m * t for m, t in zip(masses, [6, 7, 8, 3])) == F(45, 8)
            assert sum(m * n for m, n in zip(masses, [1, 2, 3, 3])) == F(15, 8)


def breakdown(row):
    b = row['breakdown']
    for bound in ('min', 'max'):
        total = sum(F(b[k][bound]) for k in ('external', 'in_service', 'contest_route', 'intentional', 'unattributed'))
        total += F(b['queue']['combined'][bound])
        if 'condition' in b:
            total += F(b['condition'][bound])
        assert total == F(row['lead_time'][bound]), (row, total)


def check_metrics(name, metrics):
    assert metrics['status'] == 'complete', metrics['status']
    case = metrics['cases'][0]
    if name in ('L1', 'L3', 'L4', 'P1', 'L1-FFF') or name.startswith('P1-'):
        for row in case['terminals']:
            if row['count']['stats']['max'] != '0':
                breakdown(row)
    if name == 'L1-FFF':
        review = next(c for c in metrics['capabilities'] if c['capability'] == 'Rev.review')
        scalar(review['starts'], 3)
        rows = [t for t in case['tasks'] if t['capability'] == 'Rev.review']
        assert len(rows) == 3, rows
        for task in rows:
            scalar(task['starts'], 1)
            scalar(task['work'], 1)
        scalar(case['work'], 9)
    if name.startswith('P1-'):
        expected = {'r1': (6, 6, 0, 1), 'r2': (37, 7, 30, 2), 'r3': (68, 8, 60, 3), 'never': (63, 3, 60, 3)}[name[3:]]
        lead, work, gap, starts = expected
        scalar(case['work'], work)
        row = next(t for t in case['terminals'] if t['count']['stats']['max'] != '0')
        scalar(row['lead_time'], lead)
        scalar(row['breakdown']['intentional'], gap)
        scalar(row['breakdown']['in_service'], work)
        check = next(c for c in metrics['capabilities'] if c['capability'] == 'Wh.check')
        scalar(check['starts'], starts)


def check_report(name, report):
    assert report['verdict'] == ('violated' if name == 'L1-FFF' else 'satisfied'), (name, report['verdict'], report['diagnostics'])
    assert report['bounds']['truncated'] is False
    if name in PROBABILITIES:
        values = [p['probability'] for p in report['properties'] if p.get('probability') is not None]
        probabilities(values, PROBABILITIES[name])
        check_means(name, values)
    if name in TERMINALS:
        for terminal in report['terminals']:
            lo, hi, work_lo, work_hi = TERMINALS[name][terminal['end']]
            assert terminal['elapsed'] == {'min': lo, 'max': hi}
            assert terminal['work'] == {'min': work_lo, 'max': work_hi}
    check_metrics(name, report['metrics'])


def check_explain(report):
    assert report['verdict'] == 'violated'
    traces = report['explanation']['counterexamples']
    assert traces
    for trace in traces:
        assert trace['replay']['status'] == 'verified', trace['replay']
        starts = []
        for step in trace['steps']:
            for event in step['events']:
                if event.get('kind') == 'start' and event.get('actor') == 'Rev':
                    starts.append(step['time'])
        assert starts == [0, 3, 6], starts


def check_lineage(report):
    assert report['verdict'] == 'satisfied'
    rows = [r for r in report['agency']['lineage'] if r['capability'] == 'Rev.review']
    assert rows, report['agency']
    for row in rows:
        assert row['subject_sources'] == ['Invoice'], row
        assert row['independent_sources'] == [], row
    groups = [g for g in report['agency']['lineage_groups'] if g['subject_sources'] == ['Invoice']]
    assert groups
    for group in groups:
        assert group['independent_sources'] == []
        assert group['actors'] == 1


def check_clock(report):
    assert report['verdict'] == 'inconclusive'
    assert report['bounds']['truncated'] is True
    values = [p['probability'] for p in report['properties'] if p.get('probability') is not None]
    assert values
    for value in values:
        assert F(value['min']) == 0 and F(value['max']) == F(1, 10)
    assert not any(t['end'] == 'escalated' for t in report['terminals'])


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--binary', required=True)
    parser.add_argument('--models', type=Path, default=Path(__file__).parent / 'models')
    args = parser.parse_args()
    for name in ('L1', 'L2', 'L3', 'L4', 'P1', 'L1-FFF'):
        run = subprocess.run([args.binary, 'check', str(args.models / (name + '.relca'))], text=True, capture_output=True)
        assert run.returncode == (1 if name == 'L1-FFF' else 0), (name, run.returncode, run.stderr, run.stdout)
        check_report(name, json.loads(run.stdout))
        print('PASS CLI', name)
    run = subprocess.run([args.binary, 'explain', str(args.models / 'L1-FFF.relca')], text=True, capture_output=True)
    assert run.returncode == 1, run.stderr
    check_explain(json.loads(run.stdout))
    print('PASS explain first review starts 0,3,6 and independent replay verified')
    run = subprocess.run([args.binary, 'check', str(args.models / 'lineage.relca')], text=True, capture_output=True)
    assert run.returncode == 0, run.stderr
    check_lineage(json.loads(run.stdout))
    print('PASS repeated review shares one subject source and adds no independent evidence')
    run = subprocess.run([args.binary, 'check', str(args.models / 'L2-clock.relca')], text=True, capture_output=True)
    assert run.returncode == 2, run.stderr
    check_clock(json.loads(run.stdout))
    print('PASS clock cut stays inconclusive and does not enter on_limit')


if __name__ == '__main__':
    main()
