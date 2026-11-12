"""K5 hand-source acceptance against current raw CLI, without historical gates."""
import argparse
import json
from pathlib import Path
import subprocess
import tempfile
from read_write_checks import require
from read_write_inline_inputs import inventory, ROOT


def compare_candidates(actual, expected):
    candidates = [r for r in actual['assumptions'] if r['field'] == 'concurrency.read_write']
    require(candidates == expected, 'hand candidate rows')


def rejects(actual, expected):
    try:
        compare_candidates(actual, expected)
    except AssertionError:
        return
    raise AssertionError('fault escaped the actual candidate comparator')


def self_test(binary):
    count = 0
    for row in inventory():
        with tempfile.TemporaryDirectory(dir=ROOT / '.tmp') as directory:
            model = Path(directory) / 'model.relca'
            model.write_text(row['source'])
            for command in row['commands']:
                args = [str(binary), command, str(model)]
                if row['configuration']:
                    args += ['--configuration', row['configuration']]
                result = subprocess.run(args, cwd=ROOT, capture_output=True, check=False)
                require(0 <= result.returncode <= 5, 'abnormal CLI exit')
                value = json.loads(result.stdout)
                candidates = [r for r in value['assumptions'] if r['field'] == 'concurrency.read_write']
                compare_candidates(value, row['rows'])
                # The exact same comparison must reject a one-character payload
                # and missing row. Neither expected side is taken from output.
                if row['rows']:
                    damaged = [dict(r) for r in candidates]
                    damaged[0]['value'] += 'x'
                    rejects(dict(assumptions=damaged), row['rows'])
                    rejects(dict(assumptions=[]), row['rows'])
                else:
                    rejects(dict(assumptions=[dict(field='concurrency.read_write', value='invented', affects='invented')]), row['rows'])
                count += 1
    require(count == 82, f'expected 82 command/selection cases, got {count}')
    print(f'K5 current raw hand acceptance: {count} command/selection cases')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--self-test', action='store_true', required=True)
    parser.add_argument('--binary', type=Path, required=True)
    args = parser.parse_args()
    self_test(args.binary.resolve())
