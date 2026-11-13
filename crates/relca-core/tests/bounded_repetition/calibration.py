"""Calibrate the initially GREEN preservation tests with single observation faults.

This does not edit production, fixed models, expected values or snapshots. The
temporary local CLI proxy changes one observation at a time and is removed on
exit. A rejection must be an AssertionError from the original acceptance.
"""
from pathlib import Path
import argparse
import subprocess
import sys
import tempfile

DATA = Path(__file__).resolve().parent
ROOT = DATA.parents[3]


def main():
    p = argparse.ArgumentParser()
    p.add_argument('--binary', required=True)
    args = p.parse_args()
    binary = str(Path(args.binary).resolve())
    faults = (
        ('elapsed', 'Preservation.test_context_words_remain_binding_names'),
        ('span', 'Preservation.test_old_invalid_shapes_keep_diagnostic_and_location'),
        ('bytes', 'Preservation.test_existing_golden_bytes'),
        ('verdict', 'Preservation.test_oracles_reject_corrupt_observations'),
    )
    with tempfile.TemporaryDirectory(prefix='bounded-calibration-', dir=ROOT / '.tmp') as d:
        for fault, test in faults:
            proxy = Path(d) / ('proxy-' + fault)
            proxy.write_text(f'''#!{sys.executable} -B
import json, subprocess, sys
out = subprocess.run([{binary!r}, *sys.argv[1:]], capture_output=True)
data = out.stdout
fault = {fault!r}
if fault == 'bytes':
    data = data.rstrip(b'\\n') + b' \\n'
else:
    value = json.loads(data)
    if fault == 'elapsed':
        for t in value.get('terminals', []):
            t['elapsed']['min'] += 1
            t['elapsed']['max'] += 1
    if fault == 'span':
        for diagnostic in value.get('diagnostics', []):
            if diagnostic.get('span'):
                diagnostic['span']['start'] += 1
    if fault == 'verdict':
        value['verdict'] = 'satisfied'
    data = (json.dumps(value, ensure_ascii=False, separators=(',', ':')) + '\\n').encode()
sys.stdout.buffer.write(data)
sys.stderr.buffer.write(out.stderr)
sys.exit(out.returncode)
''')
            proxy.chmod(0o700)
            out = subprocess.run([sys.executable, '-B', str(DATA / 'cli.py'), '--binary', str(proxy), test], cwd=ROOT, text=True, capture_output=True)
            assert out.returncode == 1 and 'AssertionError' in out.stderr, (fault, out.returncode, out.stdout, out.stderr)
            assert 'Traceback' in out.stderr and 'FAILED' in out.stderr
            print('PASS single observation fault rejected', fault, test)


if __name__ == '__main__':
    main()
