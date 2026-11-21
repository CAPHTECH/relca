"""Refresh the unedited stdout, stderr and exit status from the repository CLI.

From the repository root: python3 skills/relca-model/scripts/testdata/numerical/capture.py
Mutate is tested live: its full output includes terminology not used in the
public source tree. Keep that capture in the ignored .tmp directory.
"""
from pathlib import Path
import subprocess

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[4]
REPEAT = 'crates/relca-core/tests/bounded_repetition/models'
GENERATED = 'skills/relca-model/scripts/testdata/numerical'
RUNS = {
    'exact': ('check', 'fixtures/mean-repeat-policy/model.relca', '--configuration', 'Loop'),
    'interval': ('explain', 'fixtures/mean-contention/model.relca'),
    'unavailable': ('check', 'fixtures/mean-work.broken-missing-rate/model.relca'),
    'lower': ('check', 'fixtures/mean-first-hit-fork.broken-clock/model.relca'),
    'sample': ('simulate', 'fixtures/mean-simulate/model.relca', '--scenario', 'Busy'),
    'checks': ('check', 'fixtures/mean-metric-check/model.relca'),
    'sample-checks': ('simulate', f'{GENERATED}/sample-checks.relca', '--scenario', 'Busy'),
    'censored': ('simulate', f'{GENERATED}/censored.relca', '--scenario', 'Busy'),
    'cohort': ('simulate', 'fixtures/mean-mixed-family/model.relca'),
    'quantile': ('check', 'fixtures/qtl-censor/model.relca'),
    'load': ('simulate', 'examples/invoice/busyday.relca', '--scenario', 'BusyDaySmoke', '--configuration', 'ToBeP'),
    'repeat-sample': ('simulate', 'fixtures/mean-repeat-policy/model.relca', '--configuration', 'Loop'),
    'compare': ('compare', 'fixtures/mean-compare/model.relca', '--plans', 'Fast,Slow', '--mode', 'exhaustive'),
    'mutate': ('mutate', f'{REPEAT}/summary-round-lineage.relca', '--kinds', 'summary_reused,authority_expired'),
}

if __name__ == '__main__':
    # DSL variants from crates/relca-core/tests/simulation_means.rs. JSON is not edited.
    source = (ROOT / 'fixtures/mean-simulate/model.relca').read_text(encoding='utf-8')
    (HERE / 'sample-checks.relca').write_text(source.replace('  metric mean.Lead =',
        '  verify metric mean.Lead <= 100min\n  metric mean.Lead ='), encoding='utf-8')
    (HERE / 'censored.relca').write_text(source.replace('horizon 12min', 'horizon 8min'), encoding='utf-8')
    for name, args in RUNS.items():
        p = subprocess.run(['target/debug/relca', *args], cwd=ROOT, capture_output=True)
        out = HERE if name != 'mutate' else ROOT / '.tmp' / 'p1-raw-mutate'
        out.mkdir(parents=True, exist_ok=True)
        (out / f'{name}.json').write_bytes(p.stdout)
        (out / f'{name}.err').write_bytes(p.stderr)
        (out / f'{name}.exit').write_text(f'{p.returncode}\n', encoding='ascii')
