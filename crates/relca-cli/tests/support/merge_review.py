"""Hand-specified merge review regressions; never rewrites the frozen acceptance."""
import argparse
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[4]
BINARY = ''
MODELS = ROOT / 'crates/relca-core/tests/merge_if'


def invoke(source, args=(), command='check'):
    with tempfile.TemporaryDirectory(prefix='merge-review-', dir=ROOT / '.tmp') as tmp:
        path = Path(tmp) / 'model.relca'
        path.write_text(source)
        p = subprocess.run([BINARY, command, str(path), *args], cwd=ROOT, capture_output=True, text=True, check=False)
        return p.returncode, json.loads(p.stdout)


def warning_rows(r, code):
    return sorted((d['code'], d['message']) for d in r['diagnostics'] if d['code'] == code)


def independent(merge, duty=False, bypass=False):
    process = 'd=Reader.enter(); r=Rev.review(d); '
    if bypass:
        process += 'other=Rev.review(d); if r==pass || other==pass { posted=Ledger.post(r) } else {} '
    else:
        process += 'if r==pass { posted=Ledger.post(r) } else {} '
    process += ('unused=if r==pass {yield d} else {yield d}; ' if merge else 'if r==pass {} else {}; ')
    process += 'end done'
    return '''model Independent version "0.2" {
 type Entry=enum{entry}; type V=enum{pass,fail}; type Ack=enum{committed}
 people [p]
 actor Invoice {source document}
 actor Reader {capacity 1; capability enter()->Entry {reads [Invoice]; duration fixed 1min; result nondet{entry}}}
 actor Rev {executors [p]; capability review(e:Entry)->V {duration fixed 1min; result nondet{pass,fail}}}
 actor Ledger {capacity 1; capability post(v:V)->Ack {commits; duration fixed 1min; result nondet{committed}}}
 ''' + ('responsible p for Rev.review\n' if duty else '') + 'process Flow {' + process + '''}
 cut Scope {include [Invoice,Reader,Rev,Ledger]}
 bounds {cases 1; live_tasks 2; clock 20min}
 verify safety no_resource_overcommit
}'''


class SelectiveDiagnostics(unittest.TestCase):
    def test_real_projection_budget_is_reported_as_a_limit(self):
        statement = 'slot0=if r==pass {yield d} else {yield d}'
        for i in range(1, 35):
            statement = 'slot%d=if r==pass { %s; yield d } else {yield d}' % (i, statement)
        source = independent(True, duty=True).replace('unused=if r==pass {yield d} else {yield d}', statement)
        code, r = invoke(source)
        self.assertEqual(code, 0, r['diagnostics'])
        self.assertEqual(r['agency']['reason'], 'derivation_limit')
        self.assertEqual(r['agency']['limit']['counter'], 'depth')
        self.assertEqual(r['agency']['limit']['limit'], 32)
        self.assertEqual(r['responsibilities'][0]['observation']['reason'], 'derivation_limit')
    def test_review_pair_classifies_dependent_fields_not_limit(self):
        data = ROOT / 'crates/relca-core/tests/merge_review'
        _, d = invoke((data / 'responsibility-duplicated.relca').read_text())
        _, m = invoke((data / 'responsibility.relca').read_text())
        self.assertEqual((d['responsibilities'][0]['observation']['status'], d['responsibilities'][0]['refusal']['status']), ('missing', 'missing'))
        self.assertTrue(warning_rows(d, 'W-RESP-1') and warning_rows(d, 'W-RESP-2'))
        for field in ('observation', 'refusal'):
            self.assertEqual(m['responsibilities'][0][field]['reason'], 'merge_control')

    def test_m1_independent_review_actor_warning_matches_duplication(self):
        m = invoke((MODELS / 'models/M1.relca').read_text())[1]
        d = invoke((MODELS / 'witnesses/M1.relca').read_text())[1]
        self.assertEqual(warning_rows(d, 'W-RESP-0'), [('W-RESP-0', 'Ledger has no declared responsibility'), ('W-RESP-0', 'Rev has no declared responsibility')])
        self.assertEqual(warning_rows(m, 'W-RESP-0'), warning_rows(d, 'W-RESP-0'))

    def test_unrelated_review_warnings_are_not_lost(self):
        _, d = invoke(independent(False, bypass=True))
        _, m = invoke(independent(True, bypass=True))
        self.assertTrue(warning_rows(d, 'W-AGY-1'))
        self.assertTrue(any('Rev has no declared' in msg for _, msg in warning_rows(d, 'W-RESP-0')))
        for code in ('W-RESP-0', 'W-AGY-0', 'W-AGY-1', 'W-AGY-2'):
            self.assertEqual(warning_rows(m, code), warning_rows(d, code), code)

    def test_unrelated_duty_keeps_observation_refusal_and_w_resp_1(self):
        _, d = invoke(independent(False, duty=True))
        _, m = invoke(independent(True, duty=True))
        self.assertEqual(d['responsibilities'][0]['observation']['status'], 'missing')
        self.assertTrue(warning_rows(d, 'W-RESP-1'))
        for field in ('observation', 'refusal', 'warnings'):
            self.assertEqual(m['responsibilities'][0][field], d['responsibilities'][0][field], field)
        self.assertEqual(warning_rows(m, 'W-RESP-1'), warning_rows(d, 'W-RESP-1'))

    def test_unrelated_missing_refusal_keeps_w_resp_2(self):
        original = independent(False, duty=True).replace('if r==pass { posted=Ledger.post(r) } else {}', 'posted=Ledger.post(r);')
        merged = independent(True, duty=True).replace('if r==pass { posted=Ledger.post(r) } else {}', 'posted=Ledger.post(r);')
        _, d = invoke(original)
        _, m = invoke(merged)
        self.assertTrue(warning_rows(d, 'W-RESP-2'))
        self.assertEqual(m['responsibilities'][0]['refusal'], d['responsibilities'][0]['refusal'])
        self.assertEqual(warning_rows(m, 'W-RESP-2'), warning_rows(d, 'W-RESP-2'))

    def test_independent_via_paths_are_not_globally_erased(self):
        _, d = invoke(independent(False).replace('actor Reader {capacity 1;', 'actor Reader {via model "input@v1"; capacity 1;'))
        _, m = invoke(independent(True).replace('actor Reader {capacity 1;', 'actor Reader {via model "input@v1"; capacity 1;'))
        self.assertTrue(d['via_model_paths']['paths'])
        self.assertEqual(m['via_model_paths']['paths'], d['via_model_paths']['paths'])

    def test_dependent_duty_is_merge_control_not_a_computation_limit(self):
        base = (MODELS / 'models/M1.relca').read_text()
        base = base.replace('actor Rev { capacity 1;', 'people [p]\n actor Rev { executors [p];')
        base = base.replace('actor Ledger { capacity 1;', 'actor Ledger { capacity 1;')
        base = base.replace('s = Ledger.send(d2)', 'a = Rev.review(d2); s = Ledger.send(d2)')
        base = base[:base.rfind('}')] + 'responsible p for Rev.review\n}'
        _, m = invoke(base)
        row = m['responsibilities'][0]
        self.assertEqual(row['observation']['status'], 'not_derived')
        self.assertEqual(row['observation']['reason'], 'merge_control')
        self.assertEqual(row['refusal']['reason'], 'merge_control')
        self.assertNotIn('responsibility_derivation_limit', m['agency']['unchecked'])
        self.assertTrue(any('W-RESP-1' in x and 'merge_control' in x and 'p' in x for x in m['agency']['unchecked']))

    def test_unused_fallback_warning_is_independent(self):
        d = (ROOT / 'fixtures/agy2-keep-fallback/model.relca').read_text()
        m = d.replace('e = Extractor.extract(); v = Approver.approve(e);',
                      'e = Extractor.extract(); v = Approver.approve(e); unused=if v==pass {yield e} else {yield e};')
        _, d = invoke(d, ('--configuration', 'To'))
        _, m = invoke(m, ('--configuration', 'To'))
        self.assertTrue(warning_rows(d, 'W-AGY-3'))
        self.assertEqual(warning_rows(m, 'W-AGY-3'), warning_rows(d, 'W-AGY-3'))

    def test_dependent_default_warning_has_a_targeted_unchecked_mark(self):
        _, m = invoke((MODELS / 'models/M3.relca').read_text())
        self.assertTrue(any('W-RESP-0' in x and 'merge_control' in x and ('Auditor' in x or ': a' in x) for x in m['agency']['unchecked']))


class ReadPositions(unittest.TestCase):
    def test_all_non_process_if_arg_yield_readers_stay_closed(self):
        base = (MODELS / 'models/M1.relca').read_text()
        variants = {
            'wait': base.replace('s = Ledger.send(d2)', 'wait until d2 == repaired; s = Ledger.send(d2)'),
            'requires': base.replace('commits; duration fixed 1min', 'requires d2 == repaired else sent; commits; duration fixed 1min'),
            'property': base[:base.rfind('}')] + 'property case.Fixed = reached(d2 == repaired)\n}',
            'contract': base[:base.rfind('}')] + 'contract Value { invariant Ledger.send.finish -> d2 == repaired }\n}',
            'probability': base[:base.rfind('}')] + 'verify probability reached d2 == repaired >= 0.25\n}',
            'join': base.replace('s = Ledger.send(d2)', 'fork work { one = Clerk.fix(d); two = Clerk.fix(d) } join any work where result == d2 tie by [one,two] remaining drain if none -> done_as_is; s = Ledger.send(d2)'),
            'assignment': base.replace('s = Ledger.send(d2)', 'other = d2; s = Ledger.send(d2)'),
            'field-assignment': base.replace('s = Ledger.send(d2)', 'other = d2.field; s = Ledger.send(d2)'),
            'receiver': base.replace('s = Ledger.send(d2)', 'other = d2.work(); s = Ledger.send(d2)'),
            'simulation-setting': base[:base.rfind('}')] + 'simulation replications d2 seed 7 confidence 0.5\n}',
            'duration': base.replace('duration fixed 2min', 'duration fixed d2'),
            'probability-literal': base.replace('0.75: pass', 'd2: pass'),
            'kernel-given': base.replace('result nondet { repaired }', 'outcomes kernel K')[:base.replace('result nondet { repaired }', 'outcomes kernel K').rfind('}')] + 'kernel K given [d2] { * -> { 1: repaired } }\n}',
            'finish-assignment': base.replace('result nondet { repaired }', 'result nondet { repaired }; writes doc; finish { doc=d2 }')[:base.replace('result nondet { repaired }', 'result nondet { repaired }; writes doc; finish { doc=d2 }').rfind('}')] + 'observed case { doc:optional<D> }\n}',
            'integer-effect': base.replace('result nondet { repaired }', 'result nondet { repaired }; writes n; finish { n=d2 }')[:base.replace('result nondet { repaired }', 'result nondet { repaired }; writes n; finish { n=d2 }').rfind('}')] + 'observed case { n:0..10=0 }\n}',
            'initial-value': base[:base.rfind('}')] + 'observed case { n:0..10=d2 }\n}',
            'reads': base.replace('reads [Manual]', 'reads [d2]'),
            'reads-field': base.replace('reads [Manual]', 'reads [d2.field]'),
            'version-target': base.replace('commits; duration fixed 1min', 'version_check [d2] else sent; commits; duration fixed 1min'),
            'atomic': base.replace('result nondet { sent }', 'atomic { if d2 == repaired { result sent } else { result sent } }'),
            'responsibility': base[:base.rfind('}')] + 'people [p]\n responsible p for d2 == repaired\n}',
            'responsibility-condition': base[:base.rfind('}')] + 'people [p]\n responsible p for (d2 == repaired)\n}',
            'responsibility-formula': base[:base.rfind('}')] + 'people [p]\n responsible p for (d2)\n}',
            'authority-scope': base.replace('actor Ledger { capacity 1;', 'actor Ledger { executors [p];')[:base.replace('actor Ledger { capacity 1;', 'actor Ledger { executors [p];').rfind('}')] + 'people [p]\n authority G { basis "scope"; grants p to Ledger.send; scope d2 == repaired }\n}',
        }
        for name, src in variants.items():
            with self.subTest(reader=name):
                code, r = invoke(src)
                self.assertIn(code, (3, 4), r)
                self.assertTrue(any(d['severity'] == 'error' and 'merge_read_position' in d['message'] for d in r['diagnostics']), r['diagnostics'])


class RoundTwo(unittest.TestCase):
    def test_unchecked_rows_are_unique_and_only_fallback_has_w_agy_3(self):
        _, m = invoke((MODELS / 'models/M3.relca').read_text())
        rows = [s for s in m['agency']['unchecked'] if s.startswith('W-RESP-0(merge_control)')]
        self.assertTrue(rows)
        self.assertEqual(len(rows), len(set(rows)), rows)
        self.assertFalse(any(s.startswith('W-AGY-3(merge_control)') for s in m['agency']['unchecked']))
        base = (ROOT / 'fixtures/agy2-keep-fallback/model.relca').read_text()
        merged = base.replace('e = Extractor.extract(); v = Approver.approve(e);',
                              'e = Extractor.extract(); merged=if e==entry {yield e} else {yield e}; v = Approver.approve(e);')
        code, m = invoke(merged, ('--configuration', 'To'))
        self.assertNotIn(code, (3, 4), m['diagnostics'])
        marks = [s for s in m['agency']['unchecked'] if s.startswith('W-AGY-3(merge_control)')]
        self.assertFalse(marks, marks)
        self.assertTrue(warning_rows(m, 'W-AGY-3'))

    def test_merge_dependent_evidence_is_unsupported(self):
        source = (ROOT / 'crates/relca-core/tests/merge_review/evidence-dependent.relca').read_text()
        for command in ('check', 'explain', 'simulate'):
            src = source if command != 'simulate' else source[:source.rfind('}')] + 'simulation replications 4 seed 7 confidence 0.5\n}'
            code, report = invoke(src, command=command)
            self.assertEqual((code, report['verdict']), (4, 'unsupported'))
            self.assertTrue(any(d['code'] == 'U-SEM-0' and 'merge_control' in d['message'] for d in report['diagnostics']))

    def test_unrelated_merge_does_not_hide_evidence_satisfaction_or_missing_read(self):
        source = (ROOT / 'crates/relca-core/tests/merge_review/evidence-independent.relca').read_text()
        for src, code, verdict in [(source, 0, 'satisfied'), (source.replace('reads [Invoice];', ''), 1, 'violated')]:
            actual, report = invoke(src)
            self.assertEqual((actual, report['verdict']), (code, verdict))
            row = next(p for p in report['properties'] if p['id'] == 'contract Basis #1 evidence_basis')
            self.assertEqual(row['verdict'], verdict)
            self.assertEqual(len(report['terminals']), 1)
            self.assertEqual(report['terminals'][0]['elapsed'], {'min': 3, 'max': 3})
            self.assertEqual(report['terminals'][0]['work'], {'min': 3, 'max': 3})

        missing = source.replace('reads [Invoice];', '')
        code, report = invoke(missing, command='explain')
        self.assertEqual((code, report['verdict']), (1, 'violated'))
        trace = next(t for t in report['explanation']['counterexamples'] if t['id'] == 'contract Basis #1 evidence_basis')
        self.assertEqual(trace['replay']['status'], 'verified')
        self.assertEqual(trace['evidence'], {'binding': 't', 'actor': 'Judge', 'capability': 'assess', 'required': ['Invoice'], 'sources': [], 'missing': ['Invoice'], 'excluded_edges': []})
        self.assertEqual((trace['steps'][-1]['binding'], trace['steps'][-1]['kind'], trace['steps'][-1]['time']), ('t', 'finish', 3))
        for src, code, verdict in [(source, 2, 'inconclusive'), (missing, 1, 'violated')]:
            src = src.replace('result nondet { pass, fail }', 'result nondet { pass }')
            src = src[:src.rfind('}')] + 'simulation replications 4 seed 7 confidence 0.5\n}'
            actual, report = invoke(src, command='simulate')
            self.assertEqual((actual, report['verdict']), (code, verdict))
            row = next(p for p in report['properties'] if p['id'] == 'contract Basis #1 evidence_basis')
            self.assertEqual(row['verdict'], verdict)
            self.assertEqual(report['simulation']['status'], 'completed')


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--binary', required=True)
    args, rest = parser.parse_known_args()
    BINARY = str(Path(args.binary).resolve())
    unittest.main(argv=[__file__, *rest], verbosity=2)
