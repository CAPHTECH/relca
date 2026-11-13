"""Single-source semantic faults; every expected rejection is hand-derived."""
import argparse
from fractions import Fraction as F
import json
from pathlib import Path
import subprocess
from acceptance import PROBABILITIES, probabilities


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--binary', required=True)
    parser.add_argument('--models', type=Path, default=Path(__file__).parent / 'models')
    parser.add_argument('--scratch', type=Path, required=True)
    args = parser.parse_args()
    args.scratch.mkdir(exist_ok=True)
    for name in ('L2', 'L3', 'L4', 'P1'):
        source = (args.models / f'{name}.relca').read_text()
        if name == 'L2':
            source = source.replace('actor Clerk', 'kernel Reset { * -> { 0.1: pass with bad = true; 0.9: pass with bad = false } }\nactor Clerk')
            source = source.replace('duration fixed 2min; result nondet { pass }', 'duration fixed 2min; outcomes kernel Reset; writes bad')
        if name == 'L4':
            source = source.replace('0.2: fail; 0.8: pass', '0.5: fail; 0.5: pass').replace('0.1: fail; 0.9: pass', '0.5: fail; 0.5: pass')
        if name == 'L3':
            source = source.replace('0.75: pass with bad = false; 0.25: pass with bad = true', '1: pass with bad = true')
        if name == 'P1':
            source = source.replace('end gave_up', 'wait 30min; end gave_up')
        path = args.scratch / f'{name}.relca'
        path.write_text(source)
        result = subprocess.run([args.binary, 'check', str(path)], text=True, capture_output=True)
        assert result.returncode == 1, (name, result.returncode, result.stderr, result.stdout)
        report = json.loads(result.stdout)
        values = [p['probability'] for p in report['properties'] if p.get('probability') is not None]
        if name == 'L2':
            assert F(values[0]['min']) == F(values[0]['max']) == F(1, 1000)
        if name == 'L4':
            assert F(values[-1]['min']) == F(values[-1]['max']) == F(1, 8)
        if name == 'L3':
            assert F(values[-1]['min']) == F(values[-1]['max']) == F(1, 10)
        if name == 'P1':
            terminal = next(t for t in report['terminals'] if t['end'] == 'gave_up')
            assert terminal['elapsed'] == {'min': 93, 'max': 93}
        caught = False
        try:
            probabilities(values, PROBABILITIES[name])
        except AssertionError:
            caught = True
        assert caught, 'the original hand-value assertion must reject the fault'
        print('PASS semantic fault caught', name)


if __name__ == '__main__':
    main()
