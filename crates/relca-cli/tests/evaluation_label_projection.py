"""T12 current-output relabeling oracle, without historical metric/position ledgers."""
import argparse
import copy
import hashlib
import json
import re
import sys
import os
from typing import Any

BIJECTION = {"Zulu": "One", "Alpha": "Two", "A0": "OnTime", "Z1": "OnTimePerArrival", "Desk": "A"}


def require(condition, detail):
    if not condition:
        raise AssertionError(detail)


def rename(text):
    return re.sub(r"[A-Za-z_][A-Za-z_0-9]*", lambda m: BIJECTION.get(m.group(), m.group()), text)


def normalize(raw, source, labeled) -> Any:
    doc = json.loads(raw)
    caps = [m.span() for m in re.finditer(
        rb"capability run\(\) -> T \{ duration fixed [34]min; result nondet \{ pass \} \}", source.encode())]
    require(len(caps) == 2, 'two source-derived Capability locations')
    for index, plan in enumerate(doc['plans']):
        report = plan['report']
        require(report['model']['sha256'] == hashlib.sha256(source.encode()).hexdigest(), 'true source hash')
        report['model']['sha256'] = 'source-identity-checked'
        grain = report['actors'][0]['grain']
        require(grain == dict(status='declared' if labeled else 'default', value='team' if labeled else None),
                'only the declared grain may differ')
        report['actors'][0]['grain'] = dict(status='label-checked', value=None)
        # Plan P calls the first Capability, R the second. The other one is
        # lexically unused, and W-UNREACH-0 names that complete declaration.
        expected = dict(start=caps[1-index][0], end=caps[1-index][1])
        require(report['diagnostics'][0]['span'] == expected, 'source-derived unused Capability span')
        report['diagnostics'][0]['span'] = dict(start='unused-capability', end='unused-capability')
    def walk(v: Any) -> Any:
        if isinstance(v, str):
            return rename(v) if labeled else v
        if isinstance(v, list):
            return [walk(x) for x in v]
        if isinstance(v, dict):
            return {k: walk(x) for k, x in v.items()}
        return v
    return walk(doc)


def equal(left, right, require_permutation=True):
    left, right = copy.deepcopy(left), copy.deepcopy(right)
    key = lambda row: json.dumps(row['key'], sort_keys=True, separators=(',', ':'))
    order = [key(r) for r in left['tables']['metrics']]
    rows = right['tables']['metrics']
    if require_permutation:
        require([key(row) for row in rows] != order, 'label must exercise a real metric-key permutation')
    require(len({key(r) for r in rows}) == len(rows), 'duplicate metric key')
    require(set(order) == {key(r) for r in rows}, 'metric identities changed')
    by_key = {key(r): r for r in rows}
    right['tables']['metrics'] = [by_key[k] for k in order]
    # No other array (especially plans, evaluation or Pareto) is permuted.
    return left == right


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--verify', action='store_true')
    parser.add_argument('--self-test', action='store_true')
    args = parser.parse_args()
    data = json.load(sys.stdin)
    left = normalize(data['base'], data['base_source'], False)
    right = normalize(data['label'], data['label_source'], True)
    if os.environ.get('RELCA_LABEL_FAULT')=='identity':
        print('fault applied evaluation identity metric order',flush=True)
        key=lambda row:json.dumps(row['key'],sort_keys=True,separators=(',',':'))
        by_key={key(row):row for row in right['tables']['metrics']}
        right['tables']['metrics']=[by_key[key(row)] for row in left['tables']['metrics']]
    if os.environ.get('RELCA_LABEL_FAULT')=='work':
        print('fault applied evaluation work 6 to 7',flush=True)
        assert right['plans'][0]['report']['metrics']['overall']['work']['max']=='6'
        right['plans'][0]['report']['metrics']['overall']['work']['max']='7'
    require(equal(left, right), 'relabeling changed semantics outside names/grain/source locations')
    if args.self_test:
        damaged = copy.deepcopy(right)
        damaged['plans'][0]['report']['verdict'] = 'invented-verdict'
        require(not equal(left, damaged), 'verdict fault escaped')
        damaged = copy.deepcopy(right)
        damaged['plans'].reverse()
        require(not equal(left, damaged), 'plan permutation escaped')
        damaged = copy.deepcopy(right)
        def change_work(node):
            if isinstance(node, dict):
                if 'work' in node and isinstance(node['work'], dict) and node['work'].get('max') == '6':
                    node['work']['max'] = '7'
                    return True
                return any(change_work(child) for child in node.values())
            if isinstance(node, list):
                return any(change_work(child) for child in node)
            return False
        require(change_work(damaged), 'work 6 control must exist')
        require(not equal(left, damaged), 'work 6 to 7 fault escaped')
        aligned = copy.deepcopy(right)
        key = lambda row: json.dumps(row['key'], sort_keys=True, separators=(',', ':'))
        by_key = {key(row): row for row in aligned['tables']['metrics']}
        aligned['tables']['metrics'] = [by_key[key(row)] for row in left['tables']['metrics']]
        try:
            equal(left, aligned)
        except AssertionError:
            pass
        else:
            raise AssertionError('identity metric order did not exercise permutation control')
    print('T12 current-output relabeling: source-derived spans, hand bijection, metric-key alignment only')


if __name__ == '__main__':
    main()
