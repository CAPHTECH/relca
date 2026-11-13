"""Backend adapter contract: JSON observations from two independent public runs.

The adapter supplies engine/reference probabilities, terminal envelopes, metrics,
default reference stop reason, visited count, and expanded IR statement count.
This file has no imports from the discarded experiment. The JSON-producing
driver can be replaced, not the hand-derived assertions.
"""
from fractions import Fraction as F
import json
from pathlib import Path
import sys
from acceptance import PROBABILITIES, TERMINALS, probabilities, check_metrics, check_means


def main(directory, names=None):
    root = Path(directory)
    for name in ('L1', 'L2', 'L3', 'L4', 'P1', 'L1-FFF', 'P1-r1', 'P1-r2', 'P1-r3', 'P1-never'):
        if names is not None and name not in names:
            continue
        value = json.loads((root / f'{name}.json').read_text())
        assert value['reference_stop'] == 'Complete', (name, value['reference_diagnostics'])
        assert value['reference_verdict'] == ('violated' if name == 'L1-FFF' else 'satisfied')
        assert 0 < value['visited'] < 1_000_000
        for backend in ('engine', 'reference'):
            if name in PROBABILITIES:
                probabilities(value[f'{backend}_probabilities'], PROBABILITIES[name])
                check_means(name, value[f'{backend}_probabilities'])
            if name in TERMINALS:
                expected = TERMINALS[name]
                assert {t['end'] for t in value[f'{backend}_terminals']} == set(expected)
                for terminal in value[f'{backend}_terminals']:
                    low, high, wlow, whigh = expected[terminal['end']]
                    assert terminal['elapsed'] == [low, high], (name, backend, terminal)
                    assert terminal['work'] == [wlow, whigh]
            check_metrics(name, value[f'{backend}_metrics'])
        print('PASS independent hand values', name)
    for name in ('L1', 'L2', 'L3', 'L4', 'P1'):
        if names is not None and name + '-32' not in names:
            continue
        value = json.loads((root / f'{name}-32.json').read_text())
        assert value['ir_statements'] <= 10_000
        assert value['reference_stop'] == 'Complete', (name, value['reference_diagnostics'])
        assert 0 < value['visited'] < 1_000_000
        for backend in ('engine', 'reference'):
            terminals = {t['end']: t for t in value[f'{backend}_terminals']}
            if name == 'P1':
                assert terminals['gave_up']['elapsed'] == [962, 962]
                assert terminals['gave_up']['work'] == [32, 32]
                assert terminals['shipped']['elapsed'] == [6, 68]
                probabilities(value[f'{backend}_probabilities'], PROBABILITIES['P1'])
                continue
            assert terminals['escalated']['elapsed'] == [96, 96]
            assert terminals['done']['elapsed'] == ([1, 1] if name == 'L2' else [1, 94])
            if name == 'L2':
                probabilities(value[f'{backend}_probabilities'], ['1/10', '1/10'])
            if name == 'L3':
                probabilities(value[f'{backend}_probabilities'], ['9/10', '39/40', '159/160', str(F(1, 10 * 4**31))])
            if name == 'L4':
                probabilities(value[f'{backend}_probabilities'], ['1/2', '9/10', '99/100', str(F(1, 10**31))])
        print('PASS N32 bounds and independent hand values', name)
    if names is not None and 'L2-clock' not in names:
        return
    clock = json.loads((root / 'L2-clock.json').read_text())
    assert clock['reference_stop'] == 'Complete'
    assert clock['reference_verdict'] == 'inconclusive'
    for backend in ('engine', 'reference'):
        for p in clock[f'{backend}_probabilities']:
            assert F(p['min']) == 0 and F(p['max']) == F(1, 10)
        assert not any(t['end'] == 'escalated' for t in clock[f'{backend}_terminals'])
    print('PASS independent clock cut does not enter on_limit')


if __name__ == '__main__':
    main(sys.argv[1], sys.argv[2:] or None)
