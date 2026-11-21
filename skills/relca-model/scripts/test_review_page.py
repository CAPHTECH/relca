#!/usr/bin/env python3
"""review_page.py の検査。標準ライブラリの unittest だけを使う。

examples/invoice の as-is(satisfied)・to-be-a(violated)・simulate(unsupported)・
compare・mutate を relca で実際に回し、その出力でページを作る。
わざと壊した入力(verdict の書換え、exit の偽装、未知の鍵、別モデルの JSON 等)で
警告と「確かめられていないもの」に出ることも確かめる。

relca の binary は環境変数 RELCA_BIN、無ければ <repo>/target/debug/relca。
無ければ skip する(skip は合格ではない。先に cargo build -p relca-cli)。

  python3 skills/relca-model/scripts/test_review_page.py
"""

import html
import io
import json
import os
import re
import subprocess
import sys
import tempfile
import unittest
from contextlib import redirect_stdout
from unittest.mock import patch
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
sys.path.insert(0, str(HERE))
import review_page  # noqa: E402

RELCA = Path(os.environ.get("RELCA_BIN", ROOT / "target" / "debug" / "relca"))
INV = ROOT / "examples" / "invoice"
MINIMAL = HERE.parent / "references" / "minimal.relca"


def relca(tmp, name, *args):
    """relca を回し、stdout/stderr を保存して (json の path, stderr の path, exit) を返す。"""
    out = Path(tmp) / (name + ".json")
    err = Path(tmp) / (name + ".err")
    with open(out, "wb") as fo, open(err, "wb") as fe:
        p = subprocess.run([str(RELCA)] + [str(a) for a in args], stdout=fo, stderr=fe, cwd=ROOT)
    return out, err, p.returncode


def page(tmp, name, *argv):
    out = Path(tmp) / (name + ".html")
    rc = review_page.main([str(a) for a in argv] + ["-o", str(out)])
    assert rc == 0
    text = out.read_text(encoding="utf-8")
    m = re.search(r'<script type="application/json" id="review-data">(.*?)</script>', text, re.S)
    data = json.loads(m.group(1))  # <, >, & は \u 形式で埋め込まれている
    return text, data


def section(html_text, sid):
    m = re.search(r'<section id="%s">(.*?)</section>' % re.escape(sid), html_text, re.S)
    assert m, "section %s が無い" % sid
    return re.sub(r"<[^>]+>", " ", m.group(1))


def rewrite(src, dst, fn):
    d = json.loads(Path(src).read_text(encoding="utf-8"))
    fn(d)
    Path(dst).write_text(json.dumps(d, ensure_ascii=False), encoding="utf-8")
    return dst


class NumericalPageTest(unittest.TestCase):
    """実物の採取コマンドは testdata/numerical/capture.py。改変は未知・欠落の試験だけ。"""

    def render(self, name, change=None):
        from testdata.numerical.capture import RUNS
        data = HERE / 'testdata' / 'numerical'
        if name in RUNS:
            command, model, *_ = RUNS[name]
        elif name == 'lower-check':
            command, model = 'check', RUNS['lower'][1]
        else:
            command = 'simulate' if name == 'ld-contest-not-filed' else 'compare'
            model = f'fixtures/{name}/model.relca'
        role = 'check' if command == 'explain' else command
        with tempfile.TemporaryDirectory() as tmp, redirect_stdout(io.StringIO()):
            if name == 'mutate' or name not in RUNS:
                args = RUNS.get(name)
                if args is None:
                    inv = json.loads((ROOT / model).with_name('invocation.json').read_text()) if name != 'lower-check' else {}
                    if name == 'lower-check':
                        src = Path(tmp) / 'lower-check.relca'
                        base = (ROOT / model).read_text()
                        i = base.rindex('}')
                        src.write_text(base[:i] + '  verify metric mean.Work <= 3min\n' + base[i:])
                        model = str(src)
                    args = [role, model]
                    if inv.get('plans'):
                        args += ['--plans', ','.join(inv['plans']), '--mode', inv['mode']]
                    if inv.get('scenario'):
                        args += ['--scenario', inv['scenario']]
                fixture, stderr, code = relca(tmp, name, *args)
                if name == 'mutate':
                    self.assertEqual(code, 4)
            else:
                fixture = data / (name + '.json')
                stderr = data / (name + '.err')
                code = int((data / (name + '.exit')).read_text())
            if change is not None:
                fixture = rewrite(fixture, Path(tmp) / 'altered.json', change)
            args = ['--model', ROOT / model]
            if role != 'check':
                args += ['--check', data / 'exact.json', '--check-exit', 0]
            return page(tmp, name, *args, '--' + role, fixture,
                        '--' + role + '-stderr', stderr, '--' + role + '-exit', code, '--no-mermaid')

    def row(self, text, heading, *prefix):
        rows = [r for r in table_after(text, heading) if tuple(map(plain, r[:len(prefix)])) == prefix]
        self.assertEqual(len(rows), 1, (heading, prefix))
        return rows[0]

    def test_exact_and_relaxed_intervals_keep_both_endpoints(self):
        for name, interval, basis in [('exact', '15/2 〜 15/2 分', 'exact'),
                                      ('interval', '3 〜 6 分', '案件ごとの緩和区間')]:
            with self.subTest(name=name):
                text, _ = self.render(name)
                value = plain(self.row(text, '<h4>品質の指標 (quality)', 'mean.Lead')[2])
                self.assertIn(interval, value)
                self.assertIn(basis, value)
                self.assertIn('受付拒否</td><td>0 〜 0 件', text)

    def test_missing_is_not_zero_and_lower_bound_is_not_a_value(self):
        text, data = self.render('unavailable')
        row = self.row(text, '<h4>品質の指標 (quality)', 'mean.Cost')[2]
        self.assertIn('算出なし: not_configured', row)
        self.assertNotRegex(row, r'>0(?: |<)')
        self.assertIn('mean.Cost', [x[1] for x in data['unverified']])
        text, data = self.render('lower')
        self.assertIn('算出なし: truncated / 下限: 8 JPY の最小単位', self.row(text, '<h4>品質の指標 (quality)', 'mean.Cost')[2])
        self.assertIn('mean.Cost', [x[1] for x in data['unverified']])

    def test_sample_is_not_a_verdict_and_censoring_withholds_point(self):
        text, data = self.render('sample-checks')
        heading = '<h4>品質の指標 (quality)'
        self.assertIn('標本平均: 5 分 / 観測数 6 / 打切り 0 件', self.row(text[text.index('2.3 simulate'):], heading, 'mean.Lead')[2])
        self.assertIn('合否の根拠ではない', text)
        row = self.row(text, '<h4>指標の条件 (metric_checks)', 'mean.Lead <= 100 分')
        self.assertEqual(plain(row[1]), '算出なし: no_statistical_bound')
        self.assertIn('badge warn', row[2]); self.assertNotIn('badge ok', row[2])
        self.assertTrue(any(v[2:] == ['unsupported', 'no_statistical_bound'] for v in data['unverified']))
        text, data = self.render('censored')
        self.assertEqual(plain(self.row(text[text.index('2.3 simulate'):], heading, 'mean.Lead')[2]),
                         '算出なし: undetermined / 観測数 0 / 打切り 6 件')
        self.assertIn('mean.Lead', [x[1] for x in data['unverified']])

    def test_metric_checks_cohort_quantile_and_compare(self):
        text, _ = self.render('checks')
        self.assertIn('mean.Cost &lt;= 50 JPY の最小単位', text)
        self.assertIn('93/2 〜 93/2 JPY の最小単位', text)
        text, _ = self.render('cohort')
        for value in ('cohort.OnTime', '931461/1048576 〜 1 (比率)', '1/80', '判定不能'):
            self.assertIn(value, text)
        text, data = self.render('quantile')
        self.assertIn('19/20', text)
        self.assertIn('quantile.P95', [x[1] for x in data['unverified']])
        text, _ = self.render('compare')
        for value in ('必須条件 (hard)', '目的 (objectives)', 'mean.Lead &lt;= 6 分',
                      'minimize mean.Lead', '2 〜 2 分', '5 〜 5 分'):
            self.assertIn(value, text)

    def test_load_assumptions_exclusion_and_metrics(self):
        text, data = self.render('load')
        for value in ('simulation.same_time', '同じ時刻の出来事の順序が結果に効かないことを確かめた',
                      'policy.dispatch', 'after_finishes', 'scenario.contest_not_filed',
                      'この異議が起票されない場合の値', '評価から外した', '稼働率',
                      '7/80 (比率)', '待ち時間', '処理件数', '405/2 分'):
            self.assertIn(value, text)
        self.assertTrue(any(w.startswith('W-CONTEST-1') for w in data['relca_warnings']))
        self.assertTrue(any('contest_response' in w[1] and w[2] == 'excluded' for w in data['unverified']))
        cap = self.row(text[text.index('2.3 simulate'):], '<h4>負荷・時間・費用', 'capability: Clerk.recheck', '開始回数')
        self.assertIn('評価から外した', cap[-1]); self.assertIn('scenario_contest_not_filed', cap[-1])
        self.assertTrue(any('Clerk.recheck' in w[1] and w[2] == 'excluded' for w in data['unverified']))

    def test_contest_notes_stay_next_to_values_and_same_time_is_limited(self):
        text, _ = self.render('load')
        sim = text[text.index('2.3 simulate'):]
        for start in ('<h4>終端ごとの値', '<h4>負荷・時間・費用', '<summary>案件・操作ごとの待ち時間と作業'):
            fragment = sim[sim.index(start):]
            self.assertIn('この異議が起票されない場合の値', fragment.split('<table', 1)[0], start)
        sentence = re.search(r'<p class="note"><code>simulation.same_time</code>.*?</p>', sim).group()
        self.assertIn('対象はこの実行で確認できた時刻と観測に限る', sentence)
        self.assertNotIn('全ての順序で安全', sentence)

    def test_cut_off_is_next_to_known_quality_count_and_counted(self):
        text, data = self.render('censored')
        sim = text[text.index('2.3 simulate'):]
        row = self.row(sim, '<h4>品質の指標 (quality)', 'case.Done')
        self.assertRegex(plain(row[-1]), r'既知の到達.*標本平均: 2 件.*未確定件数.*標本平均: 1 件')
        self.assertIn('算出なし: 対象の条件がない', self.row(sim, '<th>誤り率の割当て</th>', 'alpha_after')[-1])
        self.assertTrue(any(r[1:3] == ['case.Done', 'not_reported'] for r in data['unverified']))
        self.assertEqual(plain(self.row(sim, '<h4>観測終了時に確定しなかった案件', '3', '業務上の打切り')[-1]), '2')

    def test_not_applicable_and_same_cause_keep_page_counts_consistent(self):
        text, data = self.render('checks')
        self.assertEqual(data['unverified'], [])  # Agency の未導出は別枠。
        self.assertIn('該当なしで値が無い行', section(text, 'unverified'))
        self.assertGreater(data['terminal_exclusion_row_count'], 0)
        self.assertIn('終端別の集計対象外', section(text, 'unverified'))
        row = self.row(text, '<summary>案件・操作ごとの待ち時間と作業', '案件 1 / 受付拒否の区分', '終端までの経過時間')
        self.assertIn('該当なし: 受付拒否の区分には完了までの時間を定義しない', row[-1])
        text, data = self.render('load')
        count = data['unverified_count']
        self.assertIn(f'確かめられていないもの {count} 件', section(text, 'unverified'))
        self.assertIn(f'({count} 件)', section(text, 'conclusion'))
        self.assertRegex(text, r'<details><summary>同じ理由の数値 \d+ 行</summary>')
        self.assertLess(count, len(data['unverified']))

    def test_simulation_samples_from_both_models(self):
        for fixture, value, n in (('sample', '5', 6), ('repeat-sample', '15/2', 4)):
            text, _ = self.render(fixture)
            row = self.row(text[text.index('2.3 simulate'):], '<h4>品質の指標 (quality)', 'mean.Lead')
            self.assertIn(f'標本平均: {value} 分 / 観測数 {n} / 打切り 0 件', row[-1])

    @unittest.skipUnless(RELCA.exists() or 'RELCA_BIN' in os.environ, 'relca の binary が無い')
    def test_real_compare_and_check_rows(self):
        self.assertTrue(RELCA.is_file(), str(RELCA))
        hard, objectives = '<h4>必須条件 (hard)', '<h4>目的 (objectives)'
        text, _ = self.render('lower-check')
        self.assertIn('下限: 2 分 (上限なし)', self.row(text, '<h4>指標の条件 (metric_checks)', 'mean.Work <= 3 分')[1])
        text, data = self.render('eval-hard')
        row = self.row(text, hard, 'Fast', 'cohort.WrongPerArrival <= 1/20 (比率)')
        self.assertIn('badge bad', row[-1]); self.assertNotIn('badge ok', row[-1])
        self.assertIn('6 〜 8 分', self.row(text, objectives, 'Check', 'minimize metrics.overall.work')[-1])
        self.assertIn('到達経路の最小〜最大', self.row(text, objectives, 'Check', 'minimize metrics.overall.work')[-1])
        self.assertIn('条件を満たさず採用候補外', self.row(text, '<th>採用候補の判定</th>', 'Fast')[1])
        self.assertFalse(any('より重い' in a[1] and '理由は確かめていない' in a[1] for a in data['alerts']))
        _, data = self.render('eval-hard', lambda d: d.update(verdict='satisfied'))
        self.assertTrue(any('必須条件と採否' in a[1] for a in data['alerts']))
        text, _ = self.render('cmp7-clock-loose')
        row = self.row(text, hard, 'Check', 'cohort.WrongPerArrival <= 1/2 (比率)')
        self.assertIn('1/75 〜 26/75', row[2]); self.assertIn('未確定を両端に含む', row[2]); self.assertIn('badge ok', row[3])
        text, _ = self.render('mean-compare.broken-clock')
        row = self.row(text, hard, 'Slow', 'mean.Lead <= 6 分')
        self.assertIn('下限: 5 分 (上限なし)', row[2]); self.assertIn('badge warn', row[3])
        self.assertNotIn('badge ok', row[3])
        self.assertIn('採否を確定できない', self.row(text, '<th>採用候補の判定</th>', 'Slow')[1])
        text, _ = self.render('cmp7-abort')
        row = self.row(text, hard, 'Bad', 'cohort.WrongPerArrival <= 3/5 (比率)')
        self.assertIn('badge warn', row[3]); self.assertIn('参考の標本値 (判定に使わない)', row[3])
        self.assertNotIn('標本値', row[2])
        pareto = '<h4>目的の優劣 (Pareto、順位ではない)'
        for fixture, reason in (('cmp7-sim-no-hard', '標本推定のため'), ('cmp7-no-admissible', '採用候補がないため')):
            text, data = self.render(fixture)
            self.assertIn('優劣は計算していない。算出なし: ' + reason, self.row(text, pareto, '計算状態')[-1])
            self.assertTrue(any(r[1:3] == ['evaluation.pareto', 'not_computed'] for r in data['unverified']))
        text, data = self.render('cmp7-overlap')
        self.assertIn('目的の区間が重なるため', self.row(text, pareto, 'One / Two')[-1])
        self.assertTrue(any(r[1] == 'evaluation.pareto One / Two' for r in data['unverified']))
        text, _ = self.render('mean-compare.broken-tight')
        self.assertIn('badge bad', self.row(text, hard, 'Slow', 'mean.Lead <= 3 分')[-1])
        text, data = self.render('ld-contest-not-filed')
        sim = text[text.index('2.3 simulate'):]
        self.assertIn('評価から外した', self.row(sim, '<h4>負荷・時間・費用', 'capability: Clerk.recheck', '開始回数')[-1])

    def test_unknown_words_and_missing_keys_stay_visible(self):
        def alter(report):
            report['quality']['means'][0]['value']['basis'] = 'future_basis'
            report['quality']['means'][0]['measure'] = 'future_measure'
            report['quality']['means'][1]['value']['reason'] = 'future_reason'
            del report['quality']['means'][1]['value']['upper']
            report['metric_checks'][0]['verdict'] = 'future_verdict'
        text, data = self.render('checks', alter)
        self.assertIn('未解釈', text)
        for key in ('future_basis', 'future_measure', 'future_reason', 'future_verdict', 'upper'):
            self.assertTrue(any(key in str(row) for row in data['unreadable']), key)
        row = self.row(text, '<h4>品質の指標 (quality)', 'mean.Lead')
        self.assertIn('15/4 〜 15/4', row[-1]); self.assertIn('単位の記載なし', row[-1]); self.assertIn('future_basis', row[-1])
        issues = [r for r in data['unverified'] if r[1:3] == ['mean.Lead', 'uninterpreted']]
        self.assertEqual(len(issues), 1)

    def test_excluded_never_counts_as_satisfied_even_if_verdict_says_so(self):
        # CLI は excluded を inconclusive にする。誤読の試験だけ verdict を壊す。
        def alter(report):
            for row in report['properties']:
                if 'excluded' in row:
                    row.update(verdict='satisfied', cases=[])
        text, _ = self.render('load', alter)
        conclusion = text.split('<section id="conclusion">', 1)[1].split('</section>', 1)[0]
        row, = re.findall(r'<tr>(?:(?!</tr>).)*contest_response</code>(?:(?!</tr>).)*</tr>', conclusion)
        self.assertIn('評価から外した', row)
        self.assertNotIn('badge ok', row)
        self.assertNotIn('contest_response', text.split('満たす性質(')[-1].split('</table>')[0])

    @unittest.skipUnless(RELCA.exists() or 'RELCA_BIN' in os.environ, "relca の binary が無い")
    def test_loop_mutation_is_unsupported_not_undetected(self):
        self.assertTrue(RELCA.is_file(), str(RELCA))
        text, data = self.render('mutate')
        rows = table_after(text, '<th>kind</th>')
        authority, = [r for r in rows if plain(r[1]) == 'authority_expired']
        summary, = [r for r in rows if plain(r[1]) == 'summary_reused']
        self.assertIn('badge warn', authority[2]); self.assertIn('loop_feature_combination', authority[3])
        self.assertEqual(list(map(plain, summary[6:9])), ['0', '0', 'unsupported 1'])
        mutant, = [r for r in table_after(text, '<summary>変異体ごと') if 'loop_control' in r[-1]]
        self.assertIn('badge warn', mutant[2]); self.assertNotIn('badge bad', mutant[2])
        self.assertTrue(any(v[2] == 'unsupported' and 'loop_control' in v[3] for v in data['unverified']))


@unittest.skipUnless(RELCA.exists(), "relca の binary が無い: %s(cargo build -p relca-cli)" % RELCA)
class ReviewPageTest(unittest.TestCase):
    def test_defer_effects_and_warnings_are_shown_for_main_plans_and_mutants(self):
        check, check_err, check_exit = self.r["as-is-check"]
        compare, compare_err, compare_exit = self.r["compare-exhaustive"]
        mutate, mutate_err, mutate_exit = self.r["as-is-mutate"]
        before = Path(check).read_bytes()
        text, _ = page(self.tmp, "defer-effects", "--model", INV / "as-is.relca",
            "--check", check, "--check-exit", check_exit, "--check-stderr", check_err,
            "--compare-model", INV / "configurations.relca", "--compare", compare,
            "--compare-exit", compare_exit, "--compare-stderr", compare_err,
            "--mutate", mutate, "--mutate-exit", mutate_exit, "--mutate-stderr", mutate_err, "--no-mermaid")
        model = html.unescape(section(text, "model"))
        for phrase in ("defer による指標の撤回", "cut.defer[0]", "cut.defer[1]", "opaque",
                       "Resubmission", "Vendor.contest_rate", "compare 案 AsIs", "mutate original",
                       "mutate 変異体", "W-CUT-2", "not_configured", "再計算しない"):
            self.assertIn(phrase, model)
        self.assertEqual(Path(check).read_bytes(), before)
        self.assertEqual(json.loads(before)["verdict"], "satisfied")

    def test_defer_effects_scopes_escaping_and_unreadable_payloads(self):
        opaque = {"target": "<Ghost>", "resolution": "opaque", "capabilities": [],
                  "withdraws": "process", "because": [{"kind": "opaque"}], "scope": None}
        scoped = {"target": "<R.y>", "resolution": "capability", "capabilities": ["<R.y>"],
                  "withdraws": "scoped", "because": [], "scope": {"actors": ["<R>"],
                  "capabilities": ["<R.y>"], "people": ["<q>"], "resources": ["<Desk>"]}}
        empty = {"target": "Empty", "resolution": "actor", "capabilities": [],
                 "withdraws": "none", "because": [], "scope": None}
        cut = {"defer": [{"target": e["target"]} for e in (opaque, scoped, empty)],
               "defer_effects": {"process": True, "entries": [opaque, scoped, empty]}}
        def reader():
            p = object.__new__(review_page.Page)
            p.defer_effects, p.alerts, p.unverified = [], [], []
            return p
        p = reader()
        p._scan_defer_effects("hand", cut)
        self.assertEqual(p.defer_effects, [("hand", cut["defer_effects"])])
        text = p._defer_effects_html()
        self.assertIn("scoped", text)
        self.assertIn("none", text)
        for name in ("Ghost", "R.y", "R", "q", "Desk"):
            self.assertIn("&lt;%s&gt;" % name, text)
            self.assertNotIn("<%s>" % name, text)
        for corrupt in (lambda c: c["defer_effects"].__setitem__("process", False),
                        lambda c: c["defer_effects"].__setitem__("entries", None),
                        lambda c: c["defer_effects"]["entries"][1].__setitem__("resolution", "unknown"),
                        lambda c: c["defer_effects"]["entries"][1]["scope"].__setitem__("people", None),
                        lambda c: c["defer"][0].__setitem__("target", "Other")):
            bad = json.loads(json.dumps(cut))
            corrupt(bad)
            p = reader()
            p._scan_defer_effects("fault", bad)
            self.assertEqual(p.defer_effects, [])
            self.assertTrue(any("cut.defer_effects" in str(row) for row in p.unverified))
        p = reader()
        p._scan_defer_effects("legacy", {"defer": []})
        self.assertEqual(p._defer_effects_html(), "")
        self.assertEqual(p.unverified, [])

    def test_record_label_only_describes_missing_sources_carried_by_the_record(self):
        reader = object.__new__(review_page.Page)
        route = {"record": "Notes", "registrars": [{"call": "r", "people": []}],
                 "sources": [{"source": "Invoice", "route": "relayed"}]}
        for missing in (["Know"], ["Invoice", "Know"]):
            with self.subTest(missing=missing):
                text = reader._responsibility_stages_html({"stages_status": "derived", "stages": [{
                    "call": "b", "required": missing, "direct": [], "missing": missing, "via_records": [route]}]})
                self.assertNotIn("記録経由(直接には読まない) (Know)", text)
                self.assertNotIn("記録経由(直接には読まない) (Invoice, Know)", text)
                self.assertIn("この段で直接読まない根拠源がある (Know)", text)
                if "Invoice" in missing:
                    self.assertIn("記録経由(直接には読まない) (Invoice)", text)
                else:
                    self.assertNotIn("この段では記録経由", text)

    def test_legacy_stage_page_does_not_add_record_provenance_columns_or_notes(self):
        reader = object.__new__(review_page.Page)
        observation = {"status": "derived", "stages": [{"call": "b", "actor": "A", "capability": "A.read",
            "people": [], "unattributed": True, "direct": [], "via_outputs": ["Know"], "via_outputs_status": "derived"}]}
        self.assertNotIn("記録経由", reader._observation_stages_html(observation))
        self.assertNotIn("記録経由", reader._responsibility_stages_html({"stages_status": "derived", "stages": [{
            "call": "b", "required": ["Know"], "direct": [], "missing": ["Know"]}]}))
        out, err, code = self.r["as-is-check"]
        text, _ = page(self.tmp, "legacy-no-record", "--model", INV / "as-is.relca", "--check", out,
                       "--check-exit", code, "--check-stderr", err, "--no-mermaid")
        self.assertNotIn("記録経由", section(text, "model"))
        self.assertIn("書くもの・確定", section(text, "model"))
        self.assertNotIn("書くもの・登録先・確定", section(text, "model"))

    def test_record_example_keeps_person_reach_and_stage_direct_access_separate(self):
        model = ROOT / "examples/record-provenance.relca"
        out, err, code = relca(self.tmp, "registered-knowledge", "check", model)
        value = json.loads(out.read_text())
        self.assertEqual((code, value["verdict"]), (0, "satisfied"))
        self.assertEqual(value["terminals"], [{"end": "done", "elapsed": {"min": 3, "max": 3}, "work": {"min": 3, "max": 3}, "result_at": None}])
        observation = value["responsibilities"][0]["observation"]
        route = {"record": "Notes", "registrars": [{"call": "r", "people": ["p1"]}],
                 "sources": [{"source": "Know", "route": "registrar_direct"}]}
        self.assertEqual(observation["status"], "present_via_record")
        self.assertEqual(observation["direct"], [])
        self.assertEqual(observation["missing_sources"], [])
        self.assertEqual(observation["via_records"], [route])
        stage = observation["stages"][0]
        self.assertEqual(stage, {"call": "b", "required": ["Know"], "direct": [], "missing": ["Know"], "via_records": [route]})
        text, _ = page(self.tmp, "record-page", "--model", model, "--check", out,
                       "--check-exit", code, "--check-stderr", err, "--no-mermaid")
        for part in ("registers [Notes]", "書くもの・登録先・確定", "記録経由で必要な根拠源に届く(直接とは別)",
                     "Notes 経由: Know (登録者が直接読む); 登録 r (p1)", "この段では記録経由(直接には読まない)"):
            self.assertIn(part, html.unescape(text))
        mutated, stderr, exit_code = relca(self.tmp, "record-mutate", "mutate", model,
            "--kinds", "original_access_removed,evidence_missing")
        mutation = json.loads(mutated.read_text())
        record_sites = [m for row in mutation["mutations"] for m in row["mutants"] if m["site"].get("record") == "Notes"]
        self.assertEqual(len(record_sites), 2)
        self.assertTrue(all(m["site"]["source"] is None for m in record_sites))
        text, _ = page(self.tmp, "record-mutate-page", "--model", model, "--check", out,
            "--check-exit", code, "--check-stderr", err, "--mutate", mutated, "--mutate-exit", exit_code,
            "--mutate-stderr", stderr, "--no-mermaid")
        self.assertIn("記録の読取りの削除: Notes", html.unescape(text))

    def test_record_routes_escape_names_and_distinguish_empty_absent_and_invalid(self):
        reader = object.__new__(review_page.Page)
        self.assertEqual("", reader._record_routes_html({}))
        self.assertIn("確定した記録経由なし", reader._record_routes_html({"via_records": []}))
        for value in (None, {}, [None], [{"record": "Notes", "sources": [], "registrars": []}],
                      [{"record": "Notes", "registrars": [{"call": "r", "people": []}],
                        "sources": [{"source": "Know", "route": "unknown"}]}]):
            with self.subTest(value=value):
                self.assertIn("読めない", reader._record_routes_html({"via_records": value}))
        text = reader._record_routes_html({"via_records": [{"record": "<Notes>",
            "registrars": [{"call": "<r>", "people": []}],
            "sources": [{"source": "<Know>", "route": "relayed"}]}]})
        self.assertIn("人の帰属なし", text)
        self.assertIn("中継された源", text)
        for name in ("Notes", "r", "Know"):
            self.assertIn("&lt;%s&gt;" % name, text)
            self.assertNotIn("<%s>" % name, text)

    def test_stage_unknown_via_is_not_displayed_as_empty_access_and_names_are_escaped(self):
        reader = object.__new__(review_page.Page)
        text = reader._observation_stages_html({"status": "derived", "stages": [{
            "call": "<send>", "capability": "Office.send", "people": ["<person>"],
            "unattributed": False, "direct": ["Invoice"], "via_outputs": None,
            "via_outputs_status": "not_derived", "via_outputs_reason": "merge_control"}]})
        self.assertIn("未導出 (merge_control)", text)
        self.assertIn("Invoice", text)
        self.assertIn("&lt;send&gt;", text)
        self.assertIn("&lt;person&gt;", text)
        self.assertNotIn("<send>", text)
        self.assertIn("観測に数えない", text)

    def test_responsibility_stage_unknown_inapplicable_and_legacy_are_distinct(self):
        reader = object.__new__(review_page.Page)
        self.assertIn("段ごとの記録なし", reader._responsibility_stages_html({"status": "present"}))
        for status, reason, label in [("not_derived", "no_source_identified", "未導出"),
                                      ("not_applicable", "maintenance", "対象外")]:
            text = reader._responsibility_stages_html({"stages_status": status, "stages_reason": reason, "stages": None})
            self.assertIn(label, text)
            self.assertIn(reason, text)
            self.assertNotIn("必要な根拠源を直接読む", text)
        self.assertIn("読めない", reader._responsibility_stages_html({"stages_status": "derived", "stages": None}))

    def test_responsibility_stage_partial_direct_does_not_claim_outputs_only(self):
        reader = object.__new__(review_page.Page)
        text = reader._responsibility_stages_html({"stages_status": "derived", "stages": [
            {"call": "v", "required": ["Invoice", "PO"], "direct": ["PO"], "missing": ["Invoice"]}]})
        self.assertIn("この段で直接読まない根拠源がある", text)
        self.assertIn("Invoice", text)
        self.assertNotIn("この段では生成物だけ", text)

    @classmethod
    def setUpClass(cls):
        (ROOT / ".tmp").mkdir(exist_ok=True)
        cls._tmp = tempfile.TemporaryDirectory(prefix="relca-review-", dir=ROOT / ".tmp")
        t = cls.tmp = cls._tmp.name
        cls.r = {}
        for model in ("as-is", "to-be-a", "to-be-p", "to-be-c"):
            for cmd in ("check", "explain"):
                cls.r[model + "-" + cmd] = relca(t, model + "." + cmd, cmd, INV / (model + ".relca"))
        cls.r["as-is-simulate"] = relca(t, "as-is.simulate", "simulate", INV / "as-is.relca")
        cls.r["as-is-mutate"] = relca(t, "as-is.mutate", "mutate", INV / "as-is.relca")
        cls.r["compare-exhaustive"] = relca(t, "compare", "compare", INV / "configurations.relca",
                                            "--plans", "AsIs,ToBeP,ToBeA", "--mode", "exhaustive")

    @classmethod
    def tearDownClass(cls):
        cls._tmp.cleanup()

    # ---------------------------------------------------------- 実出力

    def test_same_version_examples_have_hand_times_and_displayed_evidence(self):
        for filename, expected, code, done, rejected in [
                ("same-version.relca", "satisfied", 0, 3, 2),
                ("same-version-reused.relca", "violated", 1, 4, 3)]:
            with self.subTest(model=filename):
                model = ROOT / "examples" / filename
                out, err, actual_code = relca(self.tmp, filename + "-check", "check", model)
                value = json.loads(out.read_text())
                self.assertEqual((actual_code, value["verdict"]), (code, expected))
                times = {t["end"]: (t["elapsed"], t["work"]) for t in value["terminals"]}
                self.assertEqual(times, {"done": ({"min": done, "max": done},) * 2,
                                         "rejected": ({"min": rejected, "max": rejected},) * 2})
                self.assertEqual(next(p["verdict"] for p in value["properties"]
                                      if p["id"] == "contract V #1 same_version"), expected)
        model = ROOT / "examples/same-version-reused.relca"
        out, err, code = relca(self.tmp, "version-explain", "explain", model)
        value = json.loads(out.read_text())
        cx = next(c for c in value["explanation"]["counterexamples"] if c["id"].endswith(" same_version"))
        self.assertEqual(cx["status"], "verified")
        self.assertEqual(cx["steps"][-1]["time"], 4)
        self.assertEqual(cx["version"], {"binding": "p", "capability": "Approver.approve", "parameter": "d",
            "version": "d_dup", "examined": {"binding": "k", "capability": "Checker.check", "parameter": "d", "version": "d"}})
        text, _ = page(self.tmp, "version-page", "--model", model, "--check", out,
                       "--check-exit", code, "--check-stderr", err, "--no-mermaid")
        for part in ("same_version Checker.check(d), Approver.approve(d)", "モデルの版の証跡", "d_dup", "開始前に最後に確認した版"):
            self.assertIn(part, html.unescape(text))
        mutated, _, code = relca(self.tmp, "version-mutated", "mutate", ROOT / "examples/same-version.relca", "--kinds", "summary_reused")
        mutation = json.loads(mutated.read_text())
        row = next(r for r in mutation["mutations"] if r["kind"] == "summary_reused")
        self.assertEqual(code, 0)
        self.assertEqual(len(row["mutants"]), 1)
        self.assertEqual(row["mutants"][0]["result"], "detected")
        self.assertTrue(any(c["direction"] == "worsened" and c["key"] == "contract V #1 same_version"
                            for c in row["mutants"][0]["assigned_changes"]))

    def test_same_version_page_refuses_missing_or_forged_evidence_and_allows_null(self):
        model = ROOT / "examples/same-version-reused.relca"
        out, err, code = relca(self.tmp, "version-validation", "explain", model)
        reader = review_page.Page(review_page.parse_args(["--model", str(model), "--check", str(out),
            "--check-exit", str(code), "--check-stderr", str(err), "--no-mermaid", "-o", str(Path(self.tmp)/"version.html")]))
        original = next(c for c in json.loads(out.read_text())["explanation"]["counterexamples"] if c["id"].endswith(" same_version"))
        self.assertIn("d_dup", reader._version_html(original))
        for fault in ("missing", "type", "extra", "examined", "same", "binding", "order"):
            with self.subTest(fault=fault):
                cx = json.loads(json.dumps(original))
                if fault == "missing":
                    del cx["version"]
                elif fault == "type":
                    cx["version"] = "unknown"
                elif fault == "extra":
                    cx["version"]["unrecognized"] = True
                elif fault == "examined":
                    cx["version"]["examined"] = {}
                elif fault == "same":
                    cx["version"]["examined"]["version"] = "d_dup"
                elif fault == "binding":
                    cx["version"]["binding"] = "wrong"
                else:
                    cx["steps"] = [s for s in cx["steps"] if not (s["kind"] == "finish" and s.get("binding") == "k")]
                with self.assertRaises(ValueError):
                    reader._version_html(cx)
                rendered = reader.guarded("版の証跡", lambda: reader._version_html(cx))
                self.assertIn("この節を生成できなかった", rendered)
                self.assertEqual(reader.unverified[-1][:3], ("review_page", "版の証跡", "unsupported"))
        cx = json.loads(json.dumps(original))
        cx["version"]["examined"] = None
        self.assertIn("確認なし", reader._version_html(cx))

    def test_main_agency_limit_is_not_a_viewpoint_limit(self):
        model=Path(self.tmp)/"main-limit.relca"
        calls=" ".join("j%d=Judge.check(s);" % i for i in range(1,13))
        condition=" || ".join("j%d==pass" % i for i in range(1,13))
        model.write_text('''model MainLimit version "0.2" {
          type V=enum{pass,fail}
          type Ack=enum{done}
          actor Doc {source document}
          actor Intake {capacity 1; capability start()->V {reads [Doc]; duration fixed 1min; result nondet{pass}}}
          actor Judge {capacity 1; capability check(x:V)->V {duration fixed 1min; result nondet{pass,fail}}}
          actor Ledger {capacity 1; capability post(x:V)->Ack {commits; duration fixed 1min; result nondet{done}}}
          process Flow {s=Intake.start(); %s if %s {} else {end rejected}; p=Ledger.post(j12); end done}
          cut C {include [Doc,Intake,Judge,Ledger]}
          bounds{cases 1;live_tasks 2;clock 100min}
          verify safety no_resource_overcommit
        }''' % (calls,condition),encoding="utf-8")
        out,err,code=relca(self.tmp,"main-limit","check",model)
        value=json.loads(out.read_text())
        self.assertEqual(value["agency"]["status"],"not_derived")
        self.assertEqual(value["agency"]["limit"]["counter"],"work")
        self.assertIn("W-AGY-4",err.read_text())
        reader=review_page.Page(review_page.parse_args(["--model",str(model),"--check",str(out),
            "--check-exit",str(code),"--check-stderr",str(err),"--no-mermaid","-o",str(Path(self.tmp)/"main-limit.html")]))
        self.assertEqual(reader.viewpoint_limit_rows,[])
        self.assertNotIn("Agency viewpoint の計算上限",reader.bounds_table())

    def test_agency_scale_and_viewpoint_limits_are_displayed_without_changing_verdict(self):
        model = Path(self.tmp) / "viewpoint-scale.relca"
        calls = " ".join("j%d = Judge.check(s);" % i for i in range(1, 12))
        condition = " && ".join("j%d == pass" % i for i in range(1, 12))
        model.write_text('''model ViewpointScale version "0.2" {
            type V = enum { pass, fail }
            type Ack = enum { done }
            actor Doc { source document }
            actor Intake { capacity 1; capability start() -> V { reads [Doc]; duration fixed 1min; result nondet { pass } } }
            actor Judge { capacity 1; capability check(x:V) -> V { duration fixed 1min; result nondet { pass, fail } } }
            actor Ledger { capacity 1; capability post(x:V) -> Ack { commits; duration fixed 1min; result nondet { done } } }
            process Flow { s = Intake.start(); %s if %s {} else { end rejected }; p = Ledger.post(j11); end done }
            cut C { include [Doc, Intake, Judge, Ledger] }
            bounds { cases 1; live_tasks 2; clock 30min }
            verify safety no_resource_overcommit
        }''' % (calls, condition), encoding="utf-8")
        low, err, code = relca(self.tmp, "viewpoint-low", "check", model)
        self.assertEqual(code, 0)
        value = json.loads(low.read_text())
        limited = value["agency"]["viewpoints"]["framing"]
        self.assertEqual(limited["reason"], "derivation_limit")
        self.assertIn("W-AGY-5", err.read_text())
        args = ["--model", str(model), "--check", str(low), "--check-exit", str(code),
                "--check-stderr", str(err), "--no-mermaid"]
        reader = review_page.Page(review_page.parse_args(args + ["-o", str(Path(self.tmp) / "limit-reader.html")]))
        bounds = reader.bounds_table()
        self.assertIn("x1", bounds)
        for token in ("framing", "work", "1000000", limited["limit"]["phase"]):
            self.assertIn(token, bounds)
        text, data = page(self.tmp, "viewpoint-low-page", *args)
        self.assertEqual(data["runs"][0]["verdict"], "satisfied")
        self.assertTrue(any(row[1].endswith("/framing") for row in data["not_derived"]))
        self.assertIn("W-AGY-5", text)
        high, err, code = relca(self.tmp, "viewpoint-high", "check", model, "--agency-limit-scale", "100")
        self.assertEqual(code, 0)
        self.assertEqual(json.loads(high.read_text())["agency"]["viewpoints"]["framing"]["status"], "derived")
        high_reader = review_page.Page(review_page.parse_args(["--model", str(model), "--check", str(high),
                                                              "--check-exit", "0", "--no-mermaid", "-o", str(Path(self.tmp) / "high-reader.html")]))
        self.assertEqual(high_reader.agency_scale_rows, [("check", "x100")])
        self.assertIn("x100", high_reader.bounds_table())
        _, data = page(self.tmp, "viewpoint-high-page", "--model", model, "--check", high,
                       "--check-exit", code, "--check-stderr", err, "--no-mermaid")
        self.assertFalse(any(row[1].endswith("/framing") for row in data["not_derived"]))
        for name, change, target in [
            ("bad-scale", lambda d: d["ledger"]["execution"]["limits"]["agency_limit_scale"].__setitem__("value", 0), "agency_limit_scale"),
            ("bad-limit", lambda d: d["agency"]["viewpoints"]["framing"]["limit"].__setitem__("requested", 0), "/limit"),
        ]:
            bad = rewrite(low, Path(self.tmp) / (name + ".json"), change)
            _, data = page(self.tmp, name + "-page", "--model", model, "--check", bad,
                           "--check-exit", 0, "--no-mermaid")
            self.assertTrue(any(target in row[1] and row[2] == "unsupported" for row in data["unverified"]))
        legacy = rewrite(high, Path(self.tmp) / "scale-unrecorded.json",
                         lambda d: d["ledger"]["execution"]["limits"].pop("agency_limit_scale"))
        reader = review_page.Page(review_page.parse_args(["--model", str(model), "--check", str(legacy),
                                                        "--check-exit", "0", "--no-mermaid", "-o", str(Path(self.tmp) / "legacy-reader.html")]))
        self.assertEqual(reader.agency_scale_rows, [("check", "未記録")])
        unknown = rewrite(low, Path(self.tmp) / "invalid-ir.json", lambda d: d["agency"]["viewpoints"].__setitem__(
            "framing", {"status": "not_derived", "reason": "invalid_ir", "complete": False,
                        "unchecked": ["type_domain_authority", "practical_framing"]}))
        reader = review_page.Page(review_page.parse_args(["--model", str(model), "--check", str(unknown),
                                                        "--check-exit", "0", "--no-mermaid", "-o", str(Path(self.tmp) / "invalid-reader.html")]))
        self.assertIn(("check", "agency/viewpoints/framing", "invalid_ir"), reader.not_derived)
        self.assertEqual(reader.viewpoint_limit_rows, [])
        invalid, _, code = relca(self.tmp, "viewpoint-no-analysis", "check", model, "--agency-limit-scale", "0")
        self.assertEqual(code, 3)
        reader = review_page.Page(review_page.parse_args(["--model", str(model), "--check", str(invalid),
                                                        "--check-exit", "3", "--no-mermaid", "-o", str(Path(self.tmp) / "no-analysis-reader.html")]))
        self.assertEqual(reader.agency_scale_rows, [("check", "未解析(no_analysis)")])

    def test_terminal_and_page_count_fifteen_not_derived_items(self):
        out, err, code = self.r["as-is-check"]
        target = Path(self.tmp) / "not-derived-count.html"
        argv = ["--model", str(INV / "as-is.relca"), "--check", str(out),
                "--check-exit", str(code), "--check-stderr", str(err),
                "--no-mermaid", "-o", str(target)]
        p = review_page.Page(review_page.parse_args(argv))
        p.unverified = []
        p.not_derived = [("check", "agency/viewpoints/observation/%d" % i, "derivation_limit")
                         for i in range(15)]
        p.basis_unknown = []
        terminal = io.StringIO()
        # この試験では Agency 由来の集計だけを注入する。数値の未確認は NumericalPageTest で検査。
        with patch.object(p, "_numerical_html", return_value=""), \
                patch.object(review_page, "Page", return_value=p), redirect_stdout(terminal):
            self.assertEqual(review_page.main(argv), 0)
        self.assertIn("確かめられていないもの 15 件", terminal.getvalue())
        text = target.read_text(encoding="utf-8")
        self.assertIn("確かめられていないもの 15 件", section(text, "unverified"))
        self.assertIn("導出されなかったもの・根拠が不明なもの 15 件", text)
        self.assertIn("(15 件)", section(text, "conclusion"))
        embedded = re.search(r'id="review-data">(.*?)</script>', text, re.S)
        assert embedded is not None
        data = json.loads(embedded.group(1))
        self.assertEqual(data["unverified_count"], 15)
        self.assertEqual(data["unverified"], [])  # Existing category data is preserved.

    def test_unverified_count_deduplicates_items_but_keeps_distinct_origins(self):
        p = object.__new__(review_page.Page)
        p.unverified = [("check", "P", "unsupported", "reason")] * 2
        p.not_derived = [("check", "agency/foo", "derivation_limit")] * 2 + [
            ("compare First", "agency/foo", "derivation_limit")]
        p.basis_unknown = [("check", "agency/bar")] * 2
        self.assertEqual(p.unverified_count(), 4)

    def test_exit_codes_match_commands_json(self):
        expected = {c["id"]: c["expected_exit"] for c in json.loads((INV / "commands.json").read_text())}
        for key, (_, _, code) in self.r.items():
            if key in expected:
                self.assertEqual(code, expected[key], key)
        # commands.json に無い組も exit と verdict の対応を確かめる
        for key, (out, _, code) in self.r.items():
            v = json.loads(Path(out).read_text())["verdict"]
            self.assertEqual(review_page.EXIT_OF[v], code, key)

    def test_as_is_satisfied_shows_model_for_business_review(self):
        out, err, code = self.r["as-is-check"]
        text, data = page(self.tmp, "as-is", "--model", INV / "as-is.relca", "--check", out,
                          "--check-exit", code, "--check-stderr", err)
        self.assertEqual(data["runs"][0]["verdict"], "satisfied")
        # 判定そのものに確かめられていないものは無いが、冒頭の欄は空ではない(要求: 自己申告の未検査・W-* 警告・回していないコマンドも冒頭に出す)。
        self.assertEqual([u for u in data["unverified"] if u[1] == "結果全体"], [])
        self.assertFalse(data["empty"])
        self.assertNotIn("検出されたものは無い", text)
        for cmd in ("simulate", "compare", "mutate", "explain", "--assumptions"):
            self.assertTrue(any(x.startswith(cmd) for x in data["not_run"]), cmd)
        self.assertIn("retired_capability", data["self_unchecked"])  # cut.unchecked
        self.assertIn("fallback_path", data["self_unchecked"])  # agency.unchecked
        self.assertTrue(any(w.startswith("W-LIN-2") for w in data["relca_warnings"]))
        self.assertTrue(any(n[1].endswith("proposal") for n in data["not_derived"]))
        self.assertTrue(data["basis_unknown"])
        self.assertIn("contest_runtime", data["unread_keys"])
        self.assertIn(["check", "contract InvoiceControls #3 at_most_once"], data["mechanism_unmodeled"])
        unv = section(text, "unverified")
        self.assertIn("ADR 0056", unv)
        # 上限は冒頭と第 1 段の両方
        head = text[: text.index('<section id="unverified">')]
        for part in (re.sub(r"<[^>]+>", " ", head), section(text, "model")):
            self.assertRegex(part, r"check\s+1\s+2\s+60\s+切断なし")
        self.assertFalse([a for a in data["alerts"] if "食い違う" in a[1]])
        model = section(text, "model")
        # どの順で: source の process 順に並ぶ
        order = [model.index(x) for x in ("Clerk.enter()", "Clerk.match_po(entry)", "Clerk.match_receipt(entry)",
                                          "Approver.approve(entry,poV,receiptV)", "Ledger.post(poV,receiptV,approvalV)")]
        self.assertEqual(order, sorted(order))
        # 誰が: 宣言した実行者と既定の匿名実行者を区別する
        self.assertIn("c1, c2(宣言)", model)
        self.assertIn("匿名×1(既定)", model)
        # 誰の承認で
        self.assertIn("ApprovalGrant", model)
        self.assertIn("全経路で止める", model)
        # Cut で外した範囲と失うもの
        self.assertIn("Payment", model)
        self.assertIn("payment errors", model)
        # 冒頭: sha256・版・exit
        self.assertIn(review_page.sha256_file(INV / "as-is.relca"), text)
        self.assertIn("relca のバージョン 0.0.0", text)
        self.assertIn("未実施", section(text, "mutation"))
        self.assertIn(review_page.MERMAID_URL, text)
        self.assertIn("mermaid@10.9.1", review_page.MERMAID_URL)

    def test_to_be_a_violated_with_reason_and_unverified_first(self):
        out, err, code = self.r["to-be-a-explain"]
        self.assertEqual(code, 1)
        text, data = page(self.tmp, "to-be-a", "--model", INV / "to-be-a.relca", "--check", out,
                          "--check-exit", code)
        concl = section(text, "conclusion")
        self.assertIn("probability reached #1", concl)
        self.assertIn("231/125000", concl)
        self.assertIn("&lt;= 0.001", concl)  # source の宣言(上限)
        self.assertIn("確率の性質なので", concl)
        # vacuous の separation は確かめられていないものに出て、破れる表より前に置かれる
        self.assertIn(["explain", "contract InvoiceControls #5 separation", "vacuous", "空虚(前提が起きない)"],
                      data["unverified"])
        self.assertLess(concl.index("確かめられていない性質"), concl.index("破れる性質"))
        self.assertIn("Extractor.extract()", section(text, "model"))

    def test_simulate_unsupported_is_listed_as_unverified(self):
        out, err, code = self.r["as-is-simulate"]
        self.assertEqual(code, 4)
        cout, cerr, ccode = self.r["as-is-check"]
        text, data = page(self.tmp, "sim", "--model", INV / "as-is.relca", "--check", cout, "--check-exit", ccode,
                          "--simulate", out, "--simulate-exit", code, "--simulate-stderr", err)
        rows = [u for u in data["unverified"] if u[0] == "simulate" and u[1] == "結果全体"]
        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0][2], "unsupported")
        self.assertIn("unresolved nondeterminism", rows[0][3])
        self.assertTrue(any("U-SEM-0" in a[1] for a in data["alerts"]))
        self.assertIn("全スケジュールの安全性検証ではない", section(text, "conclusion"))
        self.assertIn("unsupported", section(text, "unverified"))

    def test_compare_lists_plans_and_their_flows(self):
        out, err, code = self.r["compare-exhaustive"]
        cout, _, ccode = self.r["as-is-check"]
        text, data = page(self.tmp, "cmp", "--model", INV / "as-is.relca", "--check", cout, "--check-exit", ccode,
                          "--compare", out, "--compare-exit", code, "--compare-model", INV / "configurations.relca")
        concl = section(text, "conclusion")
        for plan in ("AsIs", "ToBeP", "ToBeA"):
            self.assertIn("案 %s: process" % plan, concl)
        self.assertIn("ToBeAFlow", concl)
        self.assertIn("Extractor.extract()", concl)
        self.assertEqual([r["verdict"] for r in data["runs"]], ["satisfied", "violated"])

    def test_mutate_table_and_ungenerated_kinds(self):
        out, err, code = self.r["as-is-mutate"]
        cout, _, ccode = self.r["as-is-check"]
        text, data = page(self.tmp, "mut", "--model", INV / "as-is.relca", "--check", cout, "--check-exit", ccode,
                          "--mutate", out, "--mutate-exit", code)
        mut = section(text, "mutation")
        self.assertIn("Evidence の欠落", mut)
        self.assertIn("検出率・網羅率", mut)
        self.assertNotIn("未実施", mut)
        statuses = {u[1]: u[2] for u in data["unverified"] if u[0] == "mutate"}
        self.assertEqual(statuses.get("定義更新中の旧版書込み"), "unsupported")
        self.assertEqual(statuses.get("同じ Command の再送"), "not_applicable")  # 判定不能へ読み替えない

    def test_to_be_c_invalid(self):
        out, err, code = self.r["to-be-c-check"]
        self.assertEqual(code, 3)
        text, data = page(self.tmp, "c", "--model", INV / "to-be-c.relca", "--check", out, "--check-exit", code,
                          "--check-stderr", err)
        rows = [u for u in data["unverified"] if u[1] == "結果全体"]
        self.assertEqual(rows[0][2], "invalid")
        self.assertIn("latent", rows[0][3])

    def test_counterexample_has_dsl_location(self):
        broken = Path(self.tmp) / "min-broken.relca"
        src = MINIMAL.read_text(encoding="utf-8")
        guard = "    if approvalV == pass {} else { end rejected }\n"
        self.assertIn(guard, src)
        broken.write_text(src.replace(guard, ""), encoding="utf-8")
        out, err, code = relca(self.tmp, "min-broken", "explain", broken)
        self.assertEqual(code, 1)
        text, data = page(self.tmp, "cx", "--model", broken, "--check", out, "--check-exit", code)
        concl = section(text, "conclusion")
        line = [i for i, l in enumerate(broken.read_text().split("\n"), 1) if "posted = Ledger.post" in l][0]
        self.assertRegex(concl, r"反例:\s+contract ExpenseControls #1 invariant")
        self.assertRegex(concl, r"finish\s+違反")
        self.assertIn("%d:5" % line, concl)
        # 壊す前の最小例は satisfied
        ok, _, okcode = relca(self.tmp, "min", "check", MINIMAL)
        self.assertEqual(okcode, 0)

    # ---------------------------------------------------------- わざと壊した入力

    def test_verdict_rewritten_to_unsupported_is_unverified(self):
        src, _, _ = self.r["as-is-check"]
        bad = rewrite(src, Path(self.tmp) / "unsup.json", lambda d: d.__setitem__("verdict", "unsupported"))
        _, data = page(self.tmp, "unsup", "--model", INV / "as-is.relca", "--check", bad, "--check-exit", 4)
        self.assertIn("unsupported", [u[2] for u in data["unverified"] if u[1] == "結果全体"])
        # exit を元の 0 のまま渡すと食い違いの警告も出る
        _, data0 = page(self.tmp, "unsup0", "--model", INV / "as-is.relca", "--check", bad, "--check-exit", 0)
        self.assertTrue(any("食い違う" in a[1] for a in data0["alerts"]))

    def test_forged_satisfied_with_nonzero_exit_warns(self):
        src, _, code = self.r["to-be-a-check"]
        self.assertEqual(code, 1)
        bad = rewrite(src, Path(self.tmp) / "forged.json", lambda d: d.__setitem__("verdict", "satisfied"))
        text, data = page(self.tmp, "forged", "--model", INV / "to-be-a.relca", "--check", bad, "--check-exit", code)
        self.assertTrue(any("食い違う" in a[1] and "合格として扱わない" in a[1] for a in data["alerts"]))
        self.assertNotEqual(data["runs"][0]["verdict"], "satisfied")
        self.assertTrue(any(u[1] == "exit" for u in data["unverified"]))
        self.assertTrue(any("より軽い" in a[1] for a in data["alerts"]))  # 性質に violated が残る

    def test_forged_satisfied_with_zero_exit_but_unverified_property(self):
        src, _, _ = self.r["as-is-check"]

        def f(d):
            d["properties"][1]["verdict"] = "inconclusive"
        bad = rewrite(src, Path(self.tmp) / "prop.json", f)
        _, data = page(self.tmp, "prop", "--model", INV / "as-is.relca", "--check", bad, "--check-exit", 0)
        self.assertTrue(any("より軽い" in a[1] for a in data["alerts"]))
        self.assertIn("probability reached #1", [u[1] for u in data["unverified"]])
        self.assertNotEqual(data["runs"][0]["verdict"], "satisfied")

    def test_unknown_verdict_is_treated_as_unsupported(self):
        src, _, _ = self.r["as-is-check"]
        bad = rewrite(src, Path(self.tmp) / "passed.json", lambda d: d.__setitem__("verdict", "passed"))
        _, data = page(self.tmp, "passed", "--model", INV / "as-is.relca", "--check", bad, "--check-exit", 0)
        self.assertEqual(data["runs"][0]["verdict"], "unsupported")
        self.assertTrue(any("未知の verdict" in a[1] for a in data["alerts"]))

    def test_unknown_keys_and_wrong_types_do_not_crash(self):
        src, _, _ = self.r["as-is-explain"]

        def f(d):
            d["brand_new_key"] = {"x": [1, 2]}
            d["actors"] = "oops"
            d["properties"] = [1, "x", {"id": "weird", "verdict": "maybe", "extra": True}]
            d["terminals"] = {"not": "a list"}
            d["explanation"] = ["wrong"]
            d["agency"] = None
            d["metrics"] = 7
            d["responsibilities"] = [None]
        bad = rewrite(src, Path(self.tmp) / "types.json", f)
        text, data = page(self.tmp, "types", "--model", INV / "as-is.relca", "--check", bad, "--check-exit", 0)
        weird = [u for u in data["unverified"] if u[1] == "weird"]
        self.assertEqual(len(weird), 1)
        self.assertEqual(weird[0][2], "unsupported")
        self.assertIn("未知の verdict", weird[0][3])
        self.assertIn("Clerk.enter()", section(text, "model"))  # source から流れは出る

    def test_missing_exit_warns(self):
        src, _, _ = self.r["as-is-check"]
        text, data = page(self.tmp, "noexit", "--model", INV / "as-is.relca", "--check", src)
        self.assertTrue(any("exit が渡されていない" in a[1] for a in data["alerts"]))
        self.assertIn(["check", "exit", "exit 未確認"], [u[:3] for u in data["unverified"]])
        self.assertNotEqual(data["runs"][0]["verdict"], "satisfied")  # 合格として表示しない

    def test_json_from_other_model_warns(self):
        src, _, code = self.r["as-is-check"]
        _, data = page(self.tmp, "sha", "--model", INV / "to-be-a.relca", "--check", src, "--check-exit", code)
        self.assertTrue(any("sha256" in a[1] and "別のモデル" in a[1] for a in data["alerts"]))

    def test_undeclared_cut_warns_even_when_exit_is_zero(self):
        src = MINIMAL.read_text(encoding="utf-8")
        start = src.index("  cut ExpenseCut")
        end = src.index("  }\n", start) + 4
        nocut = Path(self.tmp) / "nocut.relca"
        nocut.write_text(src[:start] + src[end:], encoding="utf-8")
        out, err, code = relca(self.tmp, "nocut", "check", nocut)
        self.assertEqual(code, 0)  # relca 自体は satisfied を返す
        _, data = page(self.tmp, "nocut", "--model", nocut, "--check", out, "--check-exit", code)
        self.assertTrue(any("W-CUT-0" in a[1] for a in data["alerts"]))

    def test_unreadable_json_is_unsupported(self):
        bad = Path(self.tmp) / "garbage.json"
        bad.write_text("{not json", encoding="utf-8")
        text, data = page(self.tmp, "garbage", "--model", INV / "as-is.relca", "--check", bad, "--check-exit", 0)
        self.assertEqual(data["runs"][0]["verdict"], "unsupported")
        self.assertTrue(any("JSON を読めない" in u[3] for u in data["unverified"]))
        self.assertIn("Clerk.enter()", section(text, "model"))

    def test_no_external_resources_except_pinned_mermaid(self):
        src, _, code = self.r["as-is-check"]
        text, _ = page(self.tmp, "ext", "--model", INV / "as-is.relca", "--check", src, "--check-exit", code)
        urls = set(re.findall(r'(?:src|href)="(https?://[^"]+)"', text))
        self.assertEqual(urls, {review_page.MERMAID_URL})
        self.assertIn('integrity="%s"' % review_page.MERMAID_SRI, text)
        off, _ = page(self.tmp, "off", "--model", INV / "as-is.relca", "--check", src, "--check-exit", code,
                      "--no-mermaid")
        self.assertNotIn("https://", off)
        self.assertIn("Clerk.enter()", section(off, "model"))  # 図が無くても表で読める

    # ---------------------------------------------------------- 注入と report の内側の改ざん

    def test_html_and_mermaid_injection_is_escaped(self):
        evil = '</script><script>alert(1)</script>"\'<!--x'
        src, _, _ = self.r["as-is-check"]

        def f(d):
            d["model"]["name"] = evil
            d["verdict"] = evil
            d["properties"][0]["id"] = evil
        bad_dir = Path(self.tmp) / ('dir"\'<script>alert(3)<!--')  # path に / は入れられない
        bad_dir.mkdir(exist_ok=True)
        bad = rewrite(src, bad_dir / "inj.json", f)
        model_src = (INV / "as-is.relca").read_text(encoding="utf-8")
        hook = "    posted = Ledger.post(poV,receiptV,approvalV)\n"
        self.assertIn(hook, model_src)
        model = bad_dir / "m.relca"
        model.write_text(model_src.replace(
            hook, '    zz = Clerk.recheck("</pre><script>alert(2)</script>\\"<!--")\n' + hook), encoding="utf-8")
        text, data = page(self.tmp, "inj", "--model", model, "--check", bad, "--check-exit", 0)
        # 自分の script(埋め込み JSON・Mermaid 本体・初期化)以外の script が無い
        self.assertEqual(text.count("<script"), 3)
        self.assertNotIn("<!--", text)
        self.assertNotIn("alert(1)</script>", text)
        self.assertNotIn("</pre><script>", text)
        self.assertEqual(data["runs"][0]["raw_verdict"], evil)  # JSON は壊れずに往復する
        self.assertEqual(data["runs"][0]["verdict"], "unsupported")
        for block in re.findall(r'<pre class="mermaid">(.*?)</pre>', text, re.S):
            raw = html.unescape(block)
            self.assertNotIn("<script", raw)
            self.assertNotIn("<!--", raw)
        self.assertIn("#60;script", html.unescape(text))  # label の < は数値 entity

    def test_compare_plan_report_tampering_is_scanned(self):
        src, _, code = self.r["compare-exhaustive"]

        def f(d):
            names = [pl["configuration"] for pl in d["plans"]]
            a = d["plans"][names.index("ToBeA")]["report"]
            a["verdict"] = "satisfied"  # 性質に violated が残る
            asis = d["plans"][names.index("AsIs")]["report"]
            asis["properties"][0]["verdict"] = "inconclusive"
            asis["bounds"]["truncated"] = True
        bad = rewrite(src, Path(self.tmp) / "cmp-tamper.json", f)
        cout, _, ccode = self.r["as-is-check"]
        _, data = page(self.tmp, "cmp-tamper", "--model", INV / "as-is.relca", "--check", cout, "--check-exit", ccode,
                       "--compare", bad, "--compare-exit", code, "--compare-model", INV / "configurations.relca")
        unv = [u[:3] for u in data["unverified"]]
        self.assertIn(["compare 案 ToBeA", "全体の判定", "violated"], unv)
        self.assertIn(["compare 案 AsIs", "safety no_resource_overcommit", "inconclusive"], unv)
        self.assertIn(["compare 案 AsIs", "bounds", "inconclusive"], unv)
        self.assertTrue(any(a[1].startswith("compare 案 ToBeA") and "より軽い" in a[1] for a in data["alerts"]))

    def test_compare_top_verdict_lighter_than_plans(self):
        src, _, _ = self.r["compare-exhaustive"]
        bad = rewrite(src, Path(self.tmp) / "cmp-top.json", lambda d: d.__setitem__("verdict", "satisfied"))
        cout, _, ccode = self.r["as-is-check"]
        _, data = page(self.tmp, "cmp-top", "--model", INV / "as-is.relca", "--check", cout, "--check-exit", ccode,
                       "--compare", bad, "--compare-exit", 0)
        self.assertIn(["compare", "全体の判定", "violated"], [u[:3] for u in data["unverified"]])

    def test_top_level_vacuous_is_unverified(self):
        src, _, _ = self.r["as-is-check"]
        bad = rewrite(src, Path(self.tmp) / "vac.json", lambda d: d.__setitem__("verdict", "vacuous"))
        _, data = page(self.tmp, "vac", "--model", INV / "as-is.relca", "--check", bad, "--check-exit", 5)
        self.assertIn(["check", "結果全体", "vacuous"], [u[:3] for u in data["unverified"]])

    def test_truncated_is_unverified(self):
        src, _, _ = self.r["as-is-check"]
        bad = rewrite(src, Path(self.tmp) / "trunc.json", lambda d: d["bounds"].__setitem__("truncated", True))
        text, data = page(self.tmp, "trunc", "--model", INV / "as-is.relca", "--check", bad, "--check-exit", 0)
        self.assertIn(["check", "bounds", "inconclusive"], [u[:3] for u in data["unverified"]])
        self.assertIn("切断", section(text, "model"))

    def test_mismatch_is_unverified(self):
        src, _, code = self.r["compare-exhaustive"]
        bad = rewrite(src, Path(self.tmp) / "mm.json", lambda d: d.__setitem__("mismatch", {"reason": "cut differs"}))
        cout, _, ccode = self.r["as-is-check"]
        _, data = page(self.tmp, "mm", "--model", INV / "as-is.relca", "--check", cout, "--check-exit", ccode,
                       "--compare", bad, "--compare-exit", code)
        self.assertIn(["compare", "mismatch", "mismatch"], [u[:3] for u in data["unverified"]])

    def test_parse_failure_is_not_reported_as_undeclared_cut(self):
        src = MINIMAL.read_text(encoding="utf-8")
        self.assertIn("  cut ExpenseCut", src)
        broken = Path(self.tmp) / "comment.relca"
        broken.write_text(src.replace("  bounds", "  // note\n  bounds", 1), encoding="utf-8")
        out, err, code = relca(self.tmp, "comment", "check", broken)
        self.assertEqual(code, 4)
        report = json.loads(Path(out).read_text())
        self.assertEqual(report["cut"], "unresolved")
        self.assertFalse(any(d["code"] == "W-CUT-0" for d in report["diagnostics"]))
        _, data = page(self.tmp, "comment", "--model", broken, "--check", out, "--check-exit", code)
        cut = [u for u in data["unverified"] if u[1] == "Cut"]
        self.assertEqual(len(cut), 1)
        self.assertIn("parse に失敗", cut[0][3])
        self.assertIn("diagnostics に W-CUT-0 は無い", cut[0][3])
        self.assertEqual(cut[0][2], "unsupported")  # U-SYN-0
        self.assertFalse(any("Cut が宣言されていない" in a[1] for a in data["alerts"]))

    def test_cut_status_reads_report_fields_for_lower_syn_errors(self):
        for name in ("rec-s01-latent-case.broken-unsupported", "rec-s02-latent-actor.broken-unsupported",
                     "rec-s08-nested-field.broken-unsupported", "rec-s10-field-write.broken-field",
                     "rec-s12-none.broken-non-eq"):
            with self.subTest(name=name):
                model = ROOT / "fixtures" / name / "model.relca"
                out, err, code = relca(self.tmp, name, "check", model)
                report = json.loads(out.read_text())
                self.assertEqual(code, 4)
                self.assertIsNotNone(report["model"]["name"])
                self.assertEqual(report["cut"], "undeclared")
                self.assertIn("U-SYN-0", [d["code"] for d in report["diagnostics"]])
                self.assertIn("W-CUT-0", [d["code"] for d in report["diagnostics"]])
                _, data = page(self.tmp, name, "--model", model, "--check", out,
                               "--check-exit", code, "--check-stderr", err)
                cut = [u for u in data["unverified"] if u[1] == "Cut"]
                self.assertEqual([u[2] for u in cut], ["undeclared"])
                self.assertTrue(any("W-CUT-0" in a[1] and "Cut が宣言されていない" in a[1]
                                    for a in data["alerts"]))
                self.assertFalse(any("parse に失敗" in a[1] for a in data["alerts"]))
                self.assertNotIn("W-CUT-0 は出さない", cut[0][3])

    def test_cut_parse_failures_use_metadata_and_actual_warning_inventory(self):
        source = MINIMAL.read_text(encoding="utf-8")
        start = source.index("  cut ExpenseCut")
        end = source.index("  }\n", start) + 4
        for declared in (False, True):
            base = source if declared else source[:start] + source[end:]
            for fault, expected_exit in (("  // note\n  bounds", 4), ("  bounds }", 3)):
                with self.subTest(declared=declared, fault=fault):
                    model = Path(self.tmp) / "cut-parse.relca"
                    model.write_text(base.replace("  bounds", fault, 1), encoding="utf-8")
                    out, err, code = relca(self.tmp, "cut-parse", "check", model)
                    report = json.loads(out.read_text())
                    self.assertEqual(code, expected_exit)
                    self.assertEqual(report["cut"], "unresolved")
                    self.assertIsNone(report["model"]["name"])
                    self.assertNotIn("W-CUT-0", [d["code"] for d in report["diagnostics"]])
                    _, data = page(self.tmp, "cut-parse", "--model", model, "--check", out,
                                   "--check-exit", code, "--check-stderr", err)
                    cut = [u for u in data["unverified"] if u[1] == "Cut"]
                    self.assertEqual([u[2] for u in cut], [report["verdict"]])
                    self.assertIn("parse に失敗", cut[0][3])
                    self.assertFalse(any("Cut が宣言されていない" in a[1] for a in data["alerts"]))

    def test_cut_warning_and_parsed_metadata_are_not_inferred_from_syn_codes(self):
        model = ROOT / "fixtures" / "rec-s01-latent-case.broken-unsupported" / "model.relca"
        out, _, _ = relca(self.tmp, "cut-fields", "check", model)
        for cut_value, name, warning, error_code, verdict, expected_status, parse_failed in (
                ("undeclared", "Records", True, "E-SYN-0", "invalid", "undeclared", False),
                ("unresolved", "Records", True, "U-SYN-0", "unsupported", "unresolved", False),
                ("undeclared", "Records", False, "U-SYN-0", "unsupported", "undeclared", False),
                ("unresolved", None, True, "E-NAME-0", "invalid", "invalid", True)):
            with self.subTest(cut=cut_value, name=name, warning=warning, code=error_code):
                report = json.loads(out.read_text())
                report["cut"] = cut_value
                report["model"]["name"] = name
                report["verdict"] = verdict
                report["diagnostics"][0]["code"] = error_code
                if not warning:
                    report["diagnostics"] = [d for d in report["diagnostics"] if d["code"] != "W-CUT-0"]
                changed = Path(self.tmp) / "cut-fields-changed.json"
                changed.write_text(json.dumps(report), encoding="utf-8")
                _, data = page(self.tmp, "cut-fields", "--model", model, "--check", changed,
                               "--check-exit", review_page.EXIT_OF[verdict])
                cut = [u for u in data["unverified"] if u[1] == "Cut"]
                self.assertEqual([u[2] for u in cut], [expected_status])
                self.assertEqual(any("parse に失敗" in a[1] for a in data["alerts"]), parse_failed)
                self.assertEqual(any("W-CUT-0" in a[1] for a in data["alerts"]), warning)

    def test_syntax_error_parse_failure_is_invalid(self):
        src = MINIMAL.read_text(encoding="utf-8")
        hook = "  people [e1, m1]\n"
        self.assertIn(hook, src)
        broken = Path(self.tmp) / "esyn.relca"
        broken.write_text(src.replace(hook, hook + "  kernel rv { 0.9: pass; 0.1: fail }\n"), encoding="utf-8")
        out, err, code = relca(self.tmp, "esyn", "check", broken)
        self.assertEqual(code, 3)
        codes = [d["code"] for d in json.loads(Path(out).read_text())["diagnostics"]]
        self.assertIn("E-SYN-0", codes)
        _, data = page(self.tmp, "esyn", "--model", broken, "--check", out, "--check-exit", code)
        cut = [u for u in data["unverified"] if u[1] == "Cut"]
        self.assertEqual([c[2] for c in cut], ["invalid"])
        self.assertIn("E-SYN-0", cut[0][3])

    def test_ai_assumptions_are_ai_claims_not_confirmations(self):
        src, _, code = self.r["as-is-check"]
        spec = Path(self.tmp) / "ai.json"
        spec.write_text(json.dumps({"assumptions": [
            {"target": "Clerk.enter.duration", "value": "6min", "basis": "説明に無いので仮置き", "status": "placeholder"},
            {"target": "kernel.human_entry", "value": "0.99/0.94", "basis": "実測と書いたが出典なし", "status": "measured"},
            {"target": "Clerk.match_po.duration", "value": "3min", "status": "measured", "source": "2026-10 の作業記録"},
            {"target": "Approver.approve.duration", "value": "2min", "status": "confirmed", "confirmed_by": "a1"},
            {"target": "Nowhere.x", "value": 1, "status": "confirmed"},
            {"target": "Clerk.enter.duration", "value": "7min", "status": "placeholder"},
        ]}, ensure_ascii=False), encoding="utf-8")
        text, data = page(self.tmp, "ai", "--model", INV / "as-is.relca", "--check", src, "--check-exit", code,
                          "--assumptions", spec)
        st = [(a["target"], a["status"]) for a in data["ai_assumptions"]]
        self.assertEqual(st, [("Clerk.enter.duration", "placeholder"), ("kernel.human_entry", "placeholder"),
                              ("Clerk.match_po.duration", "measured"), ("Approver.approve.duration", "confirmed"),
                              ("Nowhere.x", "placeholder"), ("Clerk.enter.duration", "placeholder")])
        self.assertTrue(any("Nowhere.x" in a[1] and "対応しない" in a[1] for a in data["alerts"]))
        self.assertTrue(any("Clerk.enter.duration" in a[1] and "重複" in a[1] for a in data["alerts"]))
        # AI の申告は確認済み・実測として見せない(緑や decl のバッジにしない)
        self.assertNotIn("業務の人が確認", text)
        labels = re.findall(r'<span class="badge (\w+)">(AI の[^<]*)</span>', text)
        self.assertTrue(labels)
        self.assertEqual({cls for cls, _ in labels}, {"warn"})
        self.assertIn("AI の申告: 実測(出典 2026-10 の作業記録)。人が未確認", text)
        self.assertIn("AI の申告: 確認済み(confirmed_by a1)。人が未確認", text)
        # 人が確かめる必要のある AI 申告は冒頭に集める
        self.assertEqual(data["ai_claims"], ["Clerk.match_po.duration", "Approver.approve.duration"])
        top = section(text, "unverified")
        self.assertIn("人が確かめる必要のある AI 申告 2 件", top)
        self.assertIn("AI が仮に置いた値 4 件", top)
        # 出所の申告なし(所要時間・kernel 以外も)
        for t in ("Invoice.difficulty", "Ledger.post.duration", "Extractor.capacity", "metrics.window",
                  "verify.probability#1", "contract.InvoiceControls#6", "kernel.po_match"):
            self.assertIn(t, data["unprovenanced"])
        self.assertNotIn("Clerk.enter.duration", data["unprovenanced"])
        self.assertIn("出所の申告なし 15 件", top)  # 宣言 19 件 - 申告のある 4 件
        model = section(text, "model")
        self.assertIn("出所の申告なし", model)
        self.assertIn("human_entry", model)  # 確率の表は最初から開いている(details に入れない)
        m = re.search(r'<section id="model">(.*?)</section>', text, re.S).group(1)
        self.assertNotIn("<details><summary>全部を開く", m)

    # ---------------------------------------------------------- 「無い」の正例と負例(合成入力)

    def _clean_inputs(self, name, model_text, assumptions):
        d = Path(self.tmp) / name
        d.mkdir(exist_ok=True)
        model = d / "m.relca"
        model.write_text(model_text, encoding="utf-8")
        sha = review_page.sha256_file(model)
        ledger = {"producer": {"version": "0.0.0"},
                  "execution": {"selection": {"effective": {"process": {"value": "P"}}}}}
        base = {"verdict": "satisfied", "mode": "exhaustive", "model": {"name": "Clean", "sha256": sha},
                "ledger": ledger, "cut": {"name": "C", "question": "q", "include": [], "exclude": [], "defer": []},
                "bounds": {"cases": 1, "live_tasks": 1, "clock": 1, "truncated": False},
                "properties": [{"id": "reachable end.done", "verdict": "satisfied"}], "diagnostics": []}
        main = dict(base, actors=[], assumptions=[], terminals=[], unit="min", explanation={})
        sim = dict(base, mode="simulation", simulation={"status": "complete"}, terminals=[], unit="min")
        plan = dict(base, actors=[], terminals=[])
        cmp_ = {"verdict": "satisfied", "mode": "compare", "analysis": "exhaustive", "ledger": ledger,
                "plans": [{"configuration": "A", "report": plan}], "tables": {}, "mismatch": None,
                "assumptions": [], "diagnostics": []}
        mut = {"verdict": "satisfied", "mode": "mutation", "set": "bpr-default", "ledger": ledger,
               "original": base, "original_verdict": "satisfied", "run": {}, "mutations": [],
               "assumptions": [], "diagnostics": []}
        paths = {}
        for k, v in (("main", main), ("sim", sim), ("cmp", cmp_), ("mut", mut), ("ai", assumptions)):
            paths[k] = d / (k + ".json")
            paths[k].write_text(json.dumps(v, ensure_ascii=False), encoding="utf-8")
        return page(self.tmp, name, "--model", model, "--check", paths["main"], "--check-exit", 0,
                    "--simulate", paths["sim"], "--simulate-exit", 0, "--compare", paths["cmp"], "--compare-exit", 0,
                    "--compare-model", model, "--mutate", paths["mut"], "--mutate-exit", 0,
                    "--assumptions", paths["ai"])

    CLEAN = 'model Clean version "0.2" {\n  process P {\n    end done\n  }\n}\n'
    WITH_WINDOW = 'model Clean version "0.2" {\n  metrics { window 10min; currency "JPY" }\n  process P {\n    end done\n  }\n}\n'

    def test_top_box_empty_only_when_everything_is_empty(self):
        text, data = self._clean_inputs("clean", self.CLEAN, [])
        self.assertEqual((data["alerts"], data["unverified"], data["not_run"]), ([], [], []))
        self.assertTrue(data["empty"])
        self.assertIn("この入力の範囲で検出されたものは無い", section(text, "unverified"))

    def test_top_box_counts_ai_placeholder(self):
        text, data = self._clean_inputs("ph", self.WITH_WINDOW,
                                        [{"target": "metrics.window", "value": "10min", "status": "placeholder"}])
        self.assertEqual((data["alerts"], data["unverified"], data["unprovenanced"]), ([], [], []))
        self.assertFalse(data["empty"])
        self.assertNotIn("検出されたものは無い", text)

    def test_top_box_counts_unprovenanced_declarations(self):
        text, data = self._clean_inputs("np", self.WITH_WINDOW, [])
        self.assertEqual((data["alerts"], data["unverified"], data["ai_assumptions"]), ([], [], []))
        self.assertEqual(data["unprovenanced"], ["metrics.window"])
        self.assertFalse(data["empty"])
        self.assertNotIn("検出されたものは無い", text)

    # ---------------------------------------------------------- N5: cases と全体の判定

    def test_satisfied_property_with_violated_case_is_contradiction(self):
        src, _, code = self.r["as-is-check"]

        def f(d):
            d["properties"][0]["cases"] = [{"case_id": 1, "verdict": "satisfied"}, {"case_id": 2, "verdict": "violated"}]
        bad = rewrite(src, Path(self.tmp) / "cases.json", f)
        _, data = page(self.tmp, "cases", "--model", INV / "as-is.relca", "--check", bad, "--check-exit", code)
        self.assertTrue(any("全体と案件ごとの判定が矛盾" in a[1] for a in data["alerts"]))
        self.assertIn(["check", "safety no_resource_overcommit の全体判定", "violated"],
                      [u[:3] for u in data["unverified"]])

    def test_missing_loses_and_all_input_diagnostics(self):
        src, _, code = self.r["as-is-check"]
        bad = rewrite(src, Path(self.tmp) / "loses.json", lambda d: d["cut"]["exclude"][0].__setitem__("loses", None))
        sim, _, scode = self.r["as-is-simulate"]
        text, data = page(self.tmp, "loses", "--model", INV / "as-is.relca", "--check", bad, "--check-exit", code,
                          "--simulate", sim, "--simulate-exit", scode)
        self.assertTrue(any("loses" in a[1] for a in data["alerts"]))
        diag = section(text, "model")
        self.assertRegex(diag, r"simulate\s+error\s+U-SEM-0")
        self.assertRegex(diag, r"simulate\s+warning\s+W-LIN-2")


EC = HERE / "testdata" / "ec-plain"


def table_after(html_text, heading):
    """heading を含む見出しの直後の表を、行ごとの td の list にする。"""
    i = html_text.index(heading)
    j = html_text.index("</table>", i)
    body = html_text[i:j]
    return [re.findall(r"<td>(.*?)</td>", tr, re.S) for tr in re.findall(r"<tr>(.*?)</tr>", body, re.S)
            if "<td>" in tr]


def plain(cell):
    return html.unescape(re.sub(r"<[^>]+>", " ", cell)).strip()


class DiagnosticVocabularyTest(unittest.TestCase):
    def test_cause_and_role_dictionaries_match_the_adr_tables(self):
        adr = ROOT / 'docs' / 'adr' / '0081-diagnostic-location-and-detail.md'
        if not adr.is_file():
            self.skipTest('配布版には設計記録がない')
        tables = adr.read_text(encoding='utf-8')
        codes = re.compile(r'`([a-z][a-z0-9_]*)`')
        causes, roles, reasons = set(), set(), set()
        table_count = 0
        for table in re.finditer(r'^\| reason \| cause \| kind \| blocker [^\n]+\n\|---[^\n]+\n((?:\|[^\n]+\n)+)',
                                 tables, re.M):
            table_count += 1
            for row in table[1].splitlines():
                reason, cause, _, blockers = row.strip('|').split('|')
                reasons.update(codes.findall(reason))
                causes.update(codes.findall(cause))
                # 役割欄に出る DSL の leave/on_limit と位置の wire 鍵は役割ではない。
                roles.update(set(codes.findall(blockers)) - {'leave', 'on_limit', 'span', 'location'})
        self.assertEqual(table_count, 5, '診断の語彙表の形式が変わった')
        self.assertEqual(causes, set(review_page.DIAGNOSTIC_CAUSES))
        self.assertEqual(roles, set(review_page.DIAGNOSTIC_ROLES))
        comparisons = (ROOT / 'crates/relca-test-support/src/diagnostic.rs').read_text(encoding='utf-8')
        admitted = re.search(r'pub const NEW_REASONS: &\[&str\] = &\[(.*?)\];', comparisons, re.S)
        self.assertIsNotNone(admitted)
        self.assertLessEqual(set(re.findall(r'"([a-z_]+)"', admitted[1])), reasons)


@unittest.skipUnless(RELCA.exists() or 'RELCA_BIN' in os.environ, 'relca の binary が無い')
class DiagnosticPageTest(unittest.TestCase):
    fixtures = HERE / 'testdata' / 'diagnostics'

    def capture(self, name, expected_exit, *opts, edit=None):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        tmp = directory.name
        model = self.fixtures / name
        if edit is not None:
            source = model.read_bytes()
            model = Path(tmp) / name
            model.write_bytes(edit(source))
        out, err, code = relca(tmp, 'diagnostics', 'check', model, *opts)
        self.assertEqual(code, expected_exit)
        return tmp, model, out, err, code

    def render(self, captured, change=None, stderr=None, extra=()):
        tmp, model, out, err, code = captured
        if change is not None:
            out = rewrite(out, Path(tmp) / 'changed.json', change)
        with redirect_stdout(io.StringIO()):
            return page(tmp, 'diagnostics', '--model', model, '--check', out, '--check-exit', code,
                        '--check-stderr', stderr or err, '--no-mermaid', *extra)

    def diagnostic_row(self, text, code, message, origin='check'):
        rows = table_after(text, '<h3>1.6 relca の診断')
        found = [r for r in rows if tuple(map(plain, r[:4])) in (
            (origin, 'error', code, message), (origin, 'warning', code, message))]
        self.assertEqual(len(found), 1, (origin, code, message))
        return found[0]

    def details(self, text, code, message, location, origin='check'):
        row = self.diagnostic_row(text, code, message, origin)
        self.assertEqual(plain(row[4]), location)
        link = re.search(r'href="#([^"]+)"', row[6])
        assert link is not None, row
        heading = '<h4 id="' + link[1] + '">'
        start = text.index(heading)
        end = re.search(r'<h[234]\b|</section>', text[start + len(heading):])
        assert end is not None
        return text[start:start + len(heading) + end.start()]

    def assert_details(self, detail, kind_text, cause, see, blockers):
        rows = {plain(k): plain(v) for k, v in table_after(detail, '<h4')}
        self.assertEqual(rows['読み方'], kind_text)
        self.assertEqual(rows['原因'], cause)
        self.assertEqual(rows['利用ガイドの節'], see)
        self.assertEqual([tuple(map(plain, r)) for r in table_after(detail, '<h5>関係する箇所')],
                         [r + ('—',) for r in blockers])

    def test_same_version_uses_kind_even_with_same_cause(self):
        for mixed in (True, False):
            with self.subTest(mixed=mixed):
                edit = None if mixed else lambda s: s.replace(b'    e = A.make() response_loss none\n', b'')
                captured = self.capture('mixed.relca', 4, edit=edit)
                text, data = self.render(captured)
                # contract V { + space は15字。same_version 節はそこから36字。
                line = 17 if mixed else 16
                detail = self.details(text, 'U-SEM-0', 'same_version_feature_combination',
                                      f'{line}:16–{line}:52')
                blockers = [('操作結果でない値を渡す呼出し', 'a', '10:5–10:25', '—'),
                            ('操作結果でない値を渡す呼出し', 'b', '11:5–11:26', '—')]
                if mixed:
                    blockers.append(('応答', 'e', '12:5–12:36', '—'))
                self.assert_details(detail,
                                    'この機能の組合せは、この分析では扱えない。局所の書き直しで直るとは限らない。' if mixed else
                                    '書き方で直る。示された文を見直す。',
                                    'non_result_argument', '確かめた版と使う版を揃える', blockers)
                self.assertEqual(data['unverified_count'], 2)

    def test_candidates_and_excluded_writers_keep_their_own_exit(self):
        captured = self.capture('two_exits.relca', 4, '--configuration', 'Loop')
        text, data = self.render(captured)
        message = 'contract Basis #1 evidence_basis: evidence_basis_conditional_snapshot'
        detail = self.details(text, 'U-SEM-0', message, '54:20–54:57')
        body = 'rework・1 回目・本体・leave 1 番目'
        limit = 'rework・1 回目・上限到達後・leave 2 番目'
        # Actor は字下げ2字、呼出しと leave は各文の字下げ・if の接頭辞を数える。
        self.assert_details(detail, 'この性質だけ判定できていない。', '—', '根拠源を求める契約', [
            ('候補の根拠源', 'Doc', '5:3–5:32', limit),
            ('候補の根拠源', 'Policy', '6:3–6:35', body),
            ('静的な系譜で数えなかった辺', 'b1 -> r', '32:7–32:24', body),
            ('静的な系譜で数えなかった辺', 'b1 -> r3', '32:7–32:24', limit),
            ('静的な系譜で数えなかった辺', 'x1 -> r', '36:25–36:44', body),
            ('繰り返しの出口', '—', '38:9–38:14', body),
            ('静的な系譜で数えなかった辺', 'x2 -> r3', '42:22–42:41', limit),
            ('繰り返しの出口', '—', '44:7–44:12', limit),
            ('対象の呼出し', 'u', '46:5–46:20', body),
            ('対象の呼出し', 'u', '46:5–46:20', limit),
        ])
        self.assertEqual(data['unverified_count'], 6)
        self.assertIn('確かめられていないもの 6 件。', section(text, 'unverified'))
        bare = self.render(captured, lambda o: [d.update(detail=None) for d in o['diagnostics']])[1]
        self.assertEqual(data['unverified'], bare['unverified'])
        self.assertEqual(data['relca_warnings'], bare['relca_warnings'])
        stderr = Path(captured[0]) / 'supplement.err'
        # blocker の名前に「: U-」を含む未知の語彙が増えても診断に数えない。
        stderr.write_text(captured[3].read_text() + '  blocked by: role name: U-future at 1:1\n')
        with_extra = self.render(captured, stderr=stderr)[1]
        self.assertEqual(data['alerts'], with_extra['alerts'])

    def test_location_crlf_tab_unicode_multiline_and_eof(self):
        unknown = 'cut includes an undeclared Actor'
        cases = [
            (lambda s: s.replace(b'\n', b'\r\n'), 3, 'E-NAME-0', unknown, '14:35–14:42', '[Unknown]'),
            (lambda s: s.replace(b'  cut', b'\tcut'), 3, 'E-NAME-0', unknown, '14:34–14:41', '[Unknown]'),
            (lambda s: s.replace('確認'.encode(), '確認😀é'.encode()),
             3, 'E-NAME-0', unknown, '14:38–14:45', '[Unknown]'),
            (lambda _: 'model M version あ'.encode(),
             4, 'U-SYN-0', 'unsupported syntax: あ', '1:17–1:18', 'あ'),
            (lambda _: b'model M version "0.2" {\r\n',
             3, 'E-SYN-0', 'expected model item or closing brace', '2:1', ''),
            (lambda _: b'model M version "0.2" {',
             3, 'E-SYN-0', 'expected model item or closing brace', '1:24', '{'),
            (lambda s: s.replace(b'  }\n  process', b'    capability spare() -> V {\n'
                                 b'      duration fixed 1min; result nondet { ok }\n    }\n  }\n  process', 1)
             .replace(b'[Unknown]', b'[A]'),
             0, 'W-UNREACH-0', 'process P: A.spare not_called', '7:5–9:6', 'capability spare'),
        ]
        # Unknown の前は字下げ2 + `cut C { `8 + `question `9 + `"確認"; `6 + `include [`9 = 34 字。
        # タブで −1、絵文字・e・結合文字で +3。`model M version ` は16字、あは1字。
        # EOF は LF の直後か、23 字の直後。spare は字下げ4字から9行目の } の直後まで。
        for edit, status, code, msg, location, source in cases:
            with self.subTest(location=location):
                text, data = self.render(self.capture('location.relca', status, edit=edit))
                row = self.diagnostic_row(text, code, msg)
                self.assertEqual(plain(row[4]), location)
                self.assertIn(source, plain(row[5]))
                self.assertFalse(any('span と location' in r[3] for r in data['unverified']))

    def test_null_missing_and_disagreeing_locations_are_distinct(self):
        captured = self.capture('location.relca', 3, edit=lambda s: s.replace(b'\n', b'\r\n'))
        message = 'cut includes an undeclared Actor'
        for mode, expected in [('null', '位置なし'), ('legacy', '14:35–14:42 (旧形式: span から換算)'),
                               ('disagree', '15:3–15:9')]:
            with self.subTest(mode=mode):
                def change(out):
                    d = next(d for d in out['diagnostics'] if d['code'] == 'E-NAME-0')
                    if mode == 'null':
                        d.update(location=None, span=None)
                    elif mode == 'legacy':
                        d.pop('location')
                    else:
                        d.update(location=dict(line=15, column=3, end_line=15, end_column=9))

                text, data = self.render(captured, change)
                row = self.diagnostic_row(text, 'E-NAME-0', message)
                self.assertEqual(plain(row[4]), expected)
                self.assertEqual(data['unverified_count'], 3 if mode == 'disagree' else 2)
                if mode == 'disagree':
                    self.assertIn('bounds {', plain(row[5]))
                    self.assertTrue(any('span と location' in r[3] and '14:35–14:42' in r[3]
                                        and '15:3–15:9' in r[3] for r in data['unverified']))

    def test_unknown_kind_role_and_fields_show_raw_values_and_count(self):
        captured = self.capture('two_exits.relca', 4, '--configuration', 'Loop')

        def change(out):
            detail = out['diagnostics'][-1]['detail']
            detail.update(kind='future<kind>', cause='future<cause>', added={'x': '<future>'})
            detail['blockers'][0].update(role='future<role>')
            detail['blockers'][0]['exits'][0]['added'] = 42

        text, data = self.render(captured, change)
        detail = self.details(text, 'U-SEM-0',
                              'contract Basis #1 evidence_basis: evidence_basis_conditional_snapshot', '54:20–54:57')
        rows = {plain(k): plain(v) for k, v in table_after(detail, '<h4')}
        self.assertEqual(rows['読み方'], 'future<kind>')
        self.assertEqual(rows['原因'], 'future<cause>')
        first = table_after(detail, '<h5>関係する箇所')[0]
        self.assertEqual(tuple(map(plain, first[:3])), ('future<role>', 'Doc', '5:3–5:32'))
        self.assertEqual([tuple(map(plain, r)) for r in table_after(detail, '<h5>未解釈の欄')],
                         [('detail.added', '{"x": "<future>"}'), ('detail.blockers[0].exits[0].added', '42')])
        self.assertEqual(data['unverified_count'], 11)
        self.assertIn('確かめられていないもの 11 件。', section(text, 'unverified'))
        self.assertNotIn('<future>', detail)

    def test_new_refusal_cause_stays_on_its_diagnostic(self):
        captured = self.capture('mixed.relca', 4)

        def change(out):
            # 未対応の理由語・原因を模した入力。後続の CLI を実行した結果ではない。
            diagnostic = out['diagnostics'][-1]
            diagnostic['message'] = 'loop_feature_combination'
            diagnostic['detail'].update(reason='loop_feature_combination', cause='value_returning_if')

        text, _ = self.render(captured, change)
        detail = self.details(text, 'U-SEM-0', 'loop_feature_combination', '17:16–17:52')
        rows = {plain(k): plain(v) for k, v in table_after(detail, '<h4')}
        self.assertEqual(rows['理由語'], 'loop_feature_combination')
        self.assertEqual(rows['原因'], 'value_returning_if')

    def test_repeat_rewrite_points_to_guide_and_rewritten_model_is_accepted(self):
        captured = self.capture('repeat_value_if.relca', 4)
        text, data = self.render(captured)
        # 字下げ6字の直後が束縛名 v、外側は字下げ4字と長さ6の repeat。
        detail = self.details(text, 'U-SEM-0', 'unsupported syntax: loop_feature_combination', '15:7–15:8')
        self.assert_details(detail, '書き方で直る。示された文を見直す。', 'value_returning_if', '繰り返し', [
            ('繰り返しの導入箇所', '—', '12:5–12:11', '—'),
            ('併用した機能・文', '—', '15:7–15:8', '—'),
        ])
        self.assertIn('直し方は利用ガイドの「繰り返し」を参照。', detail)
        self.assertIn('繰り返しの中に値を返す if がある。', detail)
        self.assertEqual(data['unverified_count'], 3)

        def rewrite_if(source):
            before = b'      v = if a == ok { yield a } else { yield b }\n      u = A.use(v)\n'
            self.assertIn(before, source)
            return source.replace(before, b'      if a == ok { u1 = A.use(a) } else { u2 = A.use(b) }\n')

        fixed = self.capture('repeat_value_if.relca', 0, edit=rewrite_if)
        self.assertNotIn('value_returning_if', self.render(fixed)[0])

    def test_shared_record_keeps_rounds_on_the_call_not_the_declaration(self):
        captured = self.capture('repeat_shared_record.relca', 4)
        text, data = self.render(captured)
        # 15 行目は字下げ6字の後に a = A.save() が12字。
        detail = self.details(text, 'U-SEM-0', 'registration_feature_combination', '8:5–8:105')
        self.assertIn('案件をまたぐ共有記録の登録・読取りは扱えない。', detail)
        self.assertEqual([tuple(map(plain, r)) for r in table_after(detail, '<h5>関係する箇所')], [
            ('記録 Actor の宣言', 'Notes', '5:3–5:17', '—', '—'),
            ('登録する操作の宣言', 'A.save', '8:5–8:105', '—', '—'),
            ('操作の呼出し', 'a', '15:7–15:19', '—', '1、2 回目'),
        ])
        self.assertEqual(data['unverified_count'], 2)

    def test_rounds_and_exits_are_independent_cells(self):
        captured = self.capture('two_exits.relca', 4, '--configuration', 'Loop')

        def add_rounds(out):
            # 両鍵の同居を模した入力。元の実物ではこの行に exits だけがある。
            out['diagnostics'][-1]['detail']['blockers'][0]['rounds'] = [1]

        text, data = self.render(captured, add_rounds)
        detail = self.details(text, 'U-SEM-0',
                              'contract Basis #1 evidence_basis: evidence_basis_conditional_snapshot', '54:20–54:57')
        doc = table_after(detail, '<h5>関係する箇所')[0]
        self.assertEqual(tuple(map(plain, doc)), ('候補の根拠源', 'Doc', '5:3–5:32',
                                                  'rework・1 回目・上限到達後・leave 2 番目', '1 回目'))
        self.assertEqual(data['unverified_count'], 6)

    def test_command_refusals_without_root_position(self):
        merge = self.capture('merge_configurations.relca', 0, '--configuration', 'X')
        for role, opts, message, cause in [
            ('compare', ['--plans', 'X,Y', '--mode', 'exhaustive'], 'merge comparison is not connected',
             'compare では合流を含む案を比較できない。'),
            ('diff', ['--from', 'X', '--to', 'Y'], 'merge diff projection is not connected',
             'diff では合流を含む構成の差分を出せない。'),
        ]:
            with self.subTest(role=role):
                out, err, code = relca(merge[0], role, role, merge[1], *opts)
                self.assertEqual(code, 4)
                text, _ = self.render(merge, extra=[f'--{role}', out, f'--{role}-stderr', err,
                                                   f'--{role}-exit', code, f'--{role}-model', merge[1]])
                detail = self.details(text, 'U-SEM-0', 'merge_feature_combination: ' + message, '位置なし', role)
                self.assertIn(cause, detail)
                # 17 行目の束縛名 v は字下げ4字に続く1字。
                row = table_after(detail, '<h5>関係する箇所')[0]
                self.assertEqual(tuple(map(plain, row)), ('合流値の定義', 'v', '17:5–17:6', '—', '—'))
                if role == 'diff':
                    self.diagnostic_row(text, 'W-UNREACH-0', 'process Q: A.use2 not_called', 'diff to Y')

        wait = self.capture('repeat_wait.relca', 0)
        out, err, code = relca(wait[0], 'mutate', 'mutate', wait[1], '--set', 'bpr-default')
        self.assertEqual(code, 4)
        text, _ = self.render(wait, extra=['--mutate', out, '--mutate-stderr', err, '--mutate-exit', code])
        detail = self.details(text, 'U-SEM-0',
                              'loop_feature_combination: repeat mutation with wait until is not connected',
                              '位置なし', 'mutate')
        self.assertIn('mutate では条件待ちを含む繰り返しを扱えない。', detail)
        self.assertIn('関係する箇所なし', detail)


class ResponseRetryPageTest(unittest.TestCase):
    """注文・決済モデルの再送と応答喪失。explain 出力の必要な欄を抜いた固定 JSON を使い、relca は回さない。"""

    def setUp(self):
        (ROOT / ".tmp").mkdir(exist_ok=True)
        self._tmp = tempfile.TemporaryDirectory(prefix="relca-review-r5-", dir=ROOT / ".tmp")
        self.tmp = self._tmp.name
        self.base = json.loads((EC / "explain.json").read_text(encoding="utf-8"))

    def tearDown(self):
        self._tmp.cleanup()

    def build(self, name, fn=None, model=None, extra=()):
        d = json.loads(json.dumps(self.base))
        if fn:
            fn(d)
        p = Path(self.tmp) / (name + ".json")
        p.write_text(json.dumps(d, ensure_ascii=False), encoding="utf-8")
        return page(self.tmp, name, "--model", model or EC / "model.relca", "--check", p, "--check-exit", 1,
                    "--no-mermaid", *extra)

    def step(self, d, index):
        return [s for s in d["explanation"]["counterexamples"][0]["steps"] if s["index"] == index][0]

    def steps_by_index(self, data):
        return {s["index"]: s for s in data["response_steps"]}

    # ---------------------------------------------------------- 正例(注文・決済モデルの固定 JSON)

    def test_counterexample_steps_show_attempt_receipt_actual_commit_and_retry_location(self):
        text, data = self.build("ec")
        self.assertEqual(data["unreadable"], [])
        st = self.steps_by_index(data)
        self.assertEqual(st[35]["sentence"], "pay の 1 回目: 実際は charged で commit 済み、応答は unknown、業務への結果は未公開")
        self.assertEqual(st[48]["sentence"], "pay の 2 回目: 実際は charged で commit 済み、応答は delivered、業務への結果は charged")
        rows = {plain(r[0]): [plain(c) for c in r] for r in table_after(text, "再送と応答の段")}
        self.assertEqual(sorted(rows), ["27", "28", "35", "40", "41", "48"])  # commit を新しい段に増やさない
        # 列: # 時刻 案件 command 試行 事象 応答 観測した結果 実際の結果 commit retry位置 読み方
        r35 = rows["35"]
        self.assertEqual(r35[3:6], ["pay", "1", "finish"])
        self.assertIn("unknown", r35[6])
        self.assertIn("未受信", r35[7])
        self.assertNotIn("charged", r35[7])  # 実際の結果を観測結果の列へ移さない
        self.assertEqual(r35[8], "charged")
        self.assertIn("commit あり", r35[9])
        self.assertIn("81:58", r35[10])
        self.assertIn("retry max_attempts 2", r35[10])
        r48 = rows["48"]
        self.assertIn("delivered", r48[6])
        self.assertEqual((r48[7], r48[8]), ("charged", "charged"))
        self.assertIn("commit なし", rows["27"][9])
        heads = re.findall(r"<th>(.*?)</th>", text[text.index("再送と応答の段"):])[:12]
        self.assertIn("呼出し元が観測した結果", heads)
        self.assertIn("サーバ側の実際の結果(分析者用。呼出し元には見えない)", heads)
        concl = section(text, "conclusion")
        self.assertIn("一つの例で、原因の全部の説明や発生率ではない", concl)
        self.assertIn("未受信(応答 unknown)", concl)  # 反例の主表でも unknown の結果を「—」にしない

    def test_retry_declaration_is_a_target_and_matches_json(self):
        spec = Path(self.tmp) / "ai.json"
        spec.write_text(json.dumps([{"target": "retry.pay", "value": "2 / 0min", "status": "placeholder"}]),
                        encoding="utf-8")
        text, data = self.build("decl", extra=("--assumptions", spec))
        targets = {d[0]: d for d in data["declarations"]}
        for t in ("retry.pay", "response_loss.pay", "Psp.charge_plain.effect_scope", "Psp.charge_keyed.idempotency"):
            self.assertIn(t, targets)
        self.assertEqual(targets["retry.pay"][3], 81)
        self.assertFalse([a for a in data["alerts"] if "対応しない" in a[1]])  # friction F6 の偽の警告が消える
        self.assertNotIn("retry.pay", data["unprovenanced"])
        rd = data["response_decls"][0]
        self.assertEqual((rd["retry"]["max_attempts"], rd["retry"]["backoff"], rd["retry"]["exhausted"]),
                         (2, "0min", "payment_failed"))
        self.assertEqual(rd["response_loss"]["mode"], "nondet")
        rows = [[plain(c) for c in r] for r in table_after(text, "再送と応答喪失の宣言")]
        self.assertEqual(len(rows), 1)
        self.assertIn("試行の上限 2 回(初回を含む。再送は最大 1 回)", rows[0][2])
        self.assertIn("end payment_failed", rows[0][2])
        self.assertIn("AI の仮置き", rows[0][2])
        self.assertIn("effect_scope modeled", rows[0][4])
        self.assertIn("response.pay = max_attempts=2; backoff=0; loss=Nondet", rows[0][5])
        self.assertIn("max_attempts 一致、backoff 一致、loss 一致", rows[0][5])

    def test_json_retry_value_disagreeing_with_source_is_an_error(self):
        def f(d):
            for a in d["assumptions"]:
                if a["field"] == "response.pay":
                    a["value"] = "max_attempts=3; backoff=5; loss=Nondet"
        _, data = self.build("mismatch", f)
        errs = [a[1] for a in data["alerts"] if a[0] == "error" and "食い違う" in a[1]]
        self.assertTrue(any("max_attempts" in e for e in errs))
        self.assertTrue(any("backoff" in e for e in errs))

    def test_receipts_are_a_separate_set_not_a_run(self):
        text, _ = self.build("rcpt")
        concl = section(text, "conclusion")
        self.assertLess(concl.index("再送と応答の段"), concl.index("応答の観測の集合"))
        self.assertIn("行の並びは実行の順ではなく", concl)
        self.assertIn("一つの実行で全部の行が起きるわけではない", concl)
        self.assertIn("同じ試行の unknown の行が後で delivered になったことを表さない", concl)
        rows = [[plain(c) for c in r] for r in table_after(text, "応答の観測の集合")]
        self.assertEqual([(r[2], r[4].split("(")[0]) for r in rows],
                         [("1", "delivered"), ("1", "unknown"), ("2", "delivered"), ("2", "unknown")])
        self.assertEqual([r[6] for r in rows if r[4].startswith("unknown")], ["null(未受信)", "null(未受信)"])

    def test_case_outcomes_separate_unknown_from_end_and_truncation(self):
        text, data = self.build("co")
        self.assertEqual(data["case_outcomes"], [{
            "case_id": 1, "ends": ["out_of_stock", "stock_refunded", "refund_rejected", "cancel_refunded", "shipped"],
            "receipt_unknown": 1, "cut_off": 0, "receipt_resolution": "inconclusive", "truncated": False}])
        concl = section(text, "conclusion")
        self.assertIn("completed_at: null(未完了。0 ではない)", concl)
        self.assertIn("observed_until: 4 〜 4", concl)
        self.assertIn("計算上の切断(bounds.truncated・cut_off)とは別の理由の未確認", concl)
        self.assertIn("この表は End に届いた終端だけ", concl)

    def test_agency_response_control_is_not_resolution(self):
        text, data = self.build("agy")
        top = section(text, "unverified")
        self.assertIn("警告が消えたのは問題が解決したからではなく、導出しなかったからである", top)
        self.assertIn("W-AGY-0, W-AGY-2, W-LIN-2, W-RESP-2", top)
        self.assertIn("response_control", data["self_unchecked"])
        self.assertIn("general_status_query", data["self_unchecked"])  # response_analysis.unchecked
        self.assertIn("F/U の観点)を代わりに行っていない", section(text, "conclusion"))

    # ---------------------------------------------------------- 境界と壊した入力

    def test_no_counterexample_still_shows_outcomes(self):
        text, data = self.build("nocx", lambda d: d["explanation"].__setitem__("counterexamples", []))
        self.assertEqual(data["response_steps"], [])
        self.assertNotIn("再送と応答の段", text)
        self.assertIn("案件ごとの終わり方", text)
        self.assertEqual(data["unreadable"], [])

    def test_other_cases_are_kept_apart(self):
        def f(d):
            s = json.loads(json.dumps(self.step(d, 48)))
            s.update(index=60, time=9, case_id=2, attempt=1, binding="@response:2:1")
            for e in s["events"]:
                e["case_id"] = 2
            d["explanation"]["counterexamples"][0]["steps"].append(s)
            co = d["case_outcomes"]
            co.append({"case_id": 2, "arrival": 5, "terminals": [
                {"kind": "end", "end": "shipped", "completed_at": {"min": 9, "max": 9}, "elapsed": {"min": 4, "max": 4},
                 "work": {"min": 4, "max": 4}, "result_at": None, "probability": {"min": "0", "max": "1"}}]})
            co.append({"case_id": 3, "arrival": 7, "terminals": [dict(co[0]["terminals"][0])]})
            rr = [p for p in d["properties"] if p["id"] == "receipt_resolution"][0]
            # 位置と case_id がどれも一致しない並び(位置で対応させる実装を落とす)
            rr["cases"] = [{"case_id": 2, "arrival": 5, "verdict": "satisfied"},
                           {"case_id": 3, "arrival": 7, "verdict": "inconclusive"},
                           {"case_id": 1, "arrival": 0, "verdict": "inconclusive"}]
        text, data = self.build("mixed", f)
        st = self.steps_by_index(data)
        self.assertTrue(st[35]["sentence"].startswith("案件 1 の pay の 1 回目: 実際は charged で commit 済み、応答は unknown"))
        self.assertTrue(st[60]["sentence"].startswith("案件 2 の pay の 1 回目: 実際は charged で commit 済み、応答は delivered"))
        cv = {c["case_id"]: c for c in data["case_outcomes"]}
        self.assertEqual((cv[1]["receipt_resolution"], cv[1]["receipt_unknown"]), ("inconclusive", 1))
        self.assertEqual((cv[2]["receipt_resolution"], cv[2]["ends"], cv[2]["receipt_unknown"]), ("satisfied", ["shipped"], 0))
        self.assertEqual((cv[3]["receipt_resolution"], cv[3]["ends"], cv[3]["receipt_unknown"]), ("inconclusive", [], 1))
        rows = {plain(r[0]): [plain(c) for c in r] for r in table_after(text, "案件ごとの終わり方")}
        self.assertEqual(rows["3"][1], "0 件")
        self.assertIn("「失敗した」ではなく「End に届いたものが無い」", rows["3"][5])
        self.assertIn("未確認", rows["3"][5])
        self.assertIn("応答不明・切断で止まった終端は無い", rows["2"][5])

    def test_missing_fields_are_reported_not_filled(self):
        def f(d):
            del self.step(d, 35)["attempt"]
            del self.step(d, 48)["events"]
            del d["case_outcomes"][0]["terminals"][0]["observed_until"]
            d["assumptions"] = [a for a in d["assumptions"] if a["field"] != "response.pay"]
            del d["response_receipts"][0]["attempt"]
        text, data = self.build("missing", f)
        st = self.steps_by_index(data)
        self.assertIsNone(st[35]["attempt"])  # 1 を補わない
        self.assertIn("(試行番号の記録なし)", st[35]["sentence"])
        self.assertIn("commit は読めない", st[48]["sentence"])
        un = {(u[0], u[1]) for u in data["unreadable"]}
        for key in (("反例 contract PayControls #2 at_most_once の step 35", "attempt"),
                    ("反例 contract PayControls #2 at_most_once の step 48", "events"),
                    ("case_outcomes 案件 1", "observed_until"),
                    ("JSON", "assumptions の response.pay"),
                    ("response_receipts[0]", "attempt")):
            self.assertIn(key, un)
        r35 = {plain(r[0]): [plain(c) for c in r] for r in table_after(text, "再送と応答の段")}["35"]
        self.assertEqual(r35[4], "記録なし")
        self.assertIn("再送・応答の欄で読めなかった情報", section(text, "unverified"))

    def test_wrong_types_are_reported(self):
        def f(d):
            self.step(d, 35)["attempt"] = "1"
            self.step(d, 40)["command"] = 7
            self.step(d, 48)["events"] = "commit"
            d["case_outcomes"][0]["terminals"][1]["elapsed"] = "2"
            d["response_receipts"][1]["attempt"] = True
            d["response_analysis"] = ["complete"]
            self.step(d, 41)["retry_location"] = "81:58"
        text, data = self.build("types", f)
        un = {(u[0], u[1]) for u in data["unreadable"]}
        for key in (("反例 contract PayControls #2 at_most_once の step 35", "attempt"),
                    ("反例 contract PayControls #2 at_most_once の step 40", "command"),
                    ("反例 contract PayControls #2 at_most_once の step 48", "events"),
                    ("反例 contract PayControls #2 at_most_once の step 41", "retry_location"),
                    ("case_outcomes 案件 1", "elapsed"),
                    ("response_receipts[1]", "attempt"),
                    ("response_analysis", "response_analysis")):
            self.assertIn(key, un)
        st = self.steps_by_index(data)
        self.assertIsNone(st[35]["attempt"])
        self.assertIn("読めない: 1", text)

    def test_unknown_receipt_words_are_unreadable(self):
        def f(d):
            self.step(d, 35)["observed_receipt"] = "lost"
            d["response_receipts"][0]["receipt"] = "maybe"
            d["case_outcomes"][0]["terminals"][0]["kind"] = "receipt_lost"
        text, data = self.build("words", f)
        why = [u[2] for u in data["unreadable"]]
        for w in ("lost", "maybe", "receipt_lost"):
            self.assertIn("未知の語 %s" % w, why)
        st = self.steps_by_index(data)
        self.assertIn("応答の状態は読めない", st[35]["sentence"])
        self.assertNotIn("未受信", plain(table_after(text, "再送と応答の段")[2][7]))  # 未知の語を unknown と読まない
        self.assertEqual(data["case_outcomes"][0]["receipt_unknown"], 0)  # 未知の語を receipt_unknown に数えない

    def test_actual_result_forged_into_observed_is_rejected(self):
        _, data = self.build("forged", lambda d: self.step(d, 35).__setitem__("result", "charged"))
        self.assertTrue(any(a[0] == "error" and "取り違えた" in a[1] for a in data["alerts"]))
        self.assertTrue(any(u[1] == "応答と観測結果" for u in data["unverified"]))

    def test_injection_through_new_fields_is_escaped(self):
        evil = '</td></table><script>alert(1)</script><img src=x onerror=alert(2)><!--'

        def f(d):
            s = self.step(d, 35)
            s["command"] = evil
            s["actual_result"] = evil
            s["events"][0]["kind"] = evil
            self.step(d, 48)["observed_receipt"] = evil
            d["response_receipts"][0]["capability"] = evil
            d["case_outcomes"][0]["terminals"][1]["end"] = evil
            for a in d["assumptions"]:
                if a["field"] == "response.pay":
                    a["value"] = evil
        src = (EC / "model.relca").read_text(encoding="utf-8")
        hook = "response_loss nondet retry"
        self.assertIn(hook, src)
        model = Path(self.tmp) / "evil.relca"
        model.write_text(src.replace(hook, 'response_loss probability 0.1 basis "</code><script>alert(3)</script><!--" retry'),
                         encoding="utf-8")
        text, data = self.build("inj", f, model=model)
        self.assertEqual(text.count("<script"), 1)  # 埋め込み JSON だけ(--no-mermaid)
        self.assertNotIn("<img", text)
        self.assertNotIn("<!--", text)
        self.assertNotIn("</table><script>", text)
        rd = data["response_decls"][0]["response_loss"]
        self.assertEqual((rd["mode"], rd["p"], rd["basis"]), ("probability", "0.1", "</code><script>alert(3)</script><!--"))
        self.assertIn(html.escape(evil, quote=True), text)


@unittest.skipUnless(RELCA.exists(), "relca の binary が無い: %s" % RELCA)
class RepetitionPageTest(unittest.TestCase):
    """ガイドの全文と実際の check/explain 出力。期待値は testdata/repetition/README.md の手計算。"""

    @classmethod
    def setUpClass(cls):
        (ROOT / ".tmp").mkdir(exist_ok=True)
        cls._tmp = tempfile.TemporaryDirectory(prefix="relca-review-loops-", dir=ROOT / ".tmp")
        cls.tmp = cls._tmp.name
        cls.models = {n: HERE / "testdata" / "repetition" / (n + ".relca") for n in ("rework", "poll")}
        cls.models["via"] = ROOT / "fixtures" / "act-via" / "model.relca"
        cls.r = {}
        for n, model in cls.models.items():
            for cmd in ("check", "explain"):
                cls.r[n, cmd] = relca(cls.tmp, n + "." + cmd, cmd, model)

    @classmethod
    def tearDownClass(cls):
        cls._tmp.cleanup()

    def build(self, name, cmd="check", model=None, output=None, code=None, assumptions=None):
        out, err, rc = self.r[name, cmd]
        argv = ["--model", model or self.models[name], "--check", output or out,
                "--check-exit", rc if code is None else code, "--check-stderr", err]
        if assumptions:
            argv += ["--assumptions", assumptions]
        return page(self.tmp, name + "-page-" + cmd, *argv)

    @staticmethod
    def graph(text):
        match = re.search(r'<pre class="mermaid">(.*?)</pre>', text, re.S)
        assert match is not None
        return html.unescape(match.group(1))

    def test_inputs_are_exact_guide_examples(self):
        guide = (ROOT / "docs" / "guide" / "dsl.md").read_text()
        for n in ("rework", "poll"):
            self.assertIn("```relca\n" + self.models[n].read_text() + "```", guide)

    def test_rework_loop_graph_and_table_keep_controls_and_following_end(self):
        text, _ = self.build("rework")
        flow = section(text, "model")
        for word in ("repeat rework", "at_most 3", "gap 0min(既定)", "body", "on_limit",
                     "Office.inspect()", "Office.correct()", "Office.escalate()", "again", "leave",
                     "end escalated", "end done"):
            self.assertIn(word, html.unescape(flow))
        self.assertNotIn("未解釈", flow)
        graph = self.graph(text)
        self.assertIn('"次の回へ', graph)
        self.assertIn('"最後の回なら on_limit', graph)
        self.assertIn('"leave', graph)
        # A real backward edge, not just the word again on a disconnected node.
        edges = re.findall(r'n(\d+) -->\|"次の回へ[^"\n]*"\| n(\d+)', graph)
        self.assertTrue(edges, graph)
        self.assertTrue(all(int(a) > int(b) for a, b in edges))

    def test_poll_gap_is_only_on_next_round_edge(self):
        text, _ = self.build("poll")
        graph = self.graph(text)
        self.assertIn("repeat poll", graph)
        self.assertIn("gap fixed 5min", graph)
        self.assertRegex(graph, r'-->\|"次の回へ[^"\n]*gap fixed 5min[^"\n]*"\|')
        self.assertRegex(graph, r'-->\|"最後の回なら on_limit[^"\n]*gap なし[^"\n]*"\|')
        self.assertIn("end done", graph)
        self.assertNotIn("終端の宣言なし", graph)

    def test_real_rework_repetition_tables(self):
        for cmd in ("check", "explain"):
            with self.subTest(cmd=cmd):
                out, _, code = self.r["rework", cmd]
                self.assertEqual(code, 1)
                raw = json.loads(Path(out).read_text())["repetition"]
                case = raw["cases"][0]
                self.assertEqual(case["rounds"], {"min": 1, "max": 3})
                self.assertEqual(case["on_limit"], {"min": 0, "max": 1})
                self.assertEqual(case["on_limit_probability"], {"min": "1/8", "max": "1/8"})
                self.assertEqual([(r["rounds"], r["probability"]) for r in case["distribution"]],
                                 [(0, {"min": "0", "max": "0"}), (1, {"min": "1/2", "max": "1/2"}),
                                  (2, {"min": "1/4", "max": "1/4"}),
                                  (3, {"min": "1/4", "max": "1/4"})])
                text, data = self.build("rework", cmd)
                self.assertNotIn("repetition", data["unread_keys"])
                rows = [[plain(c) for c in r] for r in table_after(text, "繰り返しの回数と上限後の処理")]
                self.assertEqual(rows[0][1:5], ["rework", "1 〜 3", "0 〜 1", "1/8 〜 1/8"])
                dist = [[plain(c) for c in r] for r in table_after(text, "body 開始回数の分布")]
                self.assertEqual([r[1:4] for r in dist], [["rework", "0", "0 〜 0"], ["rework", "1", "1/2 〜 1/2"],
                                 ["rework", "2", "1/4 〜 1/4"], ["rework", "3", "1/4 〜 1/4"]])
                sites = [[plain(c) for c in r] for r in table_after(text, "繰り返しの site と元位置")]
                self.assertEqual({tuple(r[1:5]) for r in sites},
                                 {("rework", c, str(n), "body") for c in ("r", "c") for n in (1, 2, 3)}
                                 | {("rework", "e", "null", "on_limit")})
                self.assertTrue(all(re.fullmatch(r"\d+:\d+", r[5]) for r in sites))

    def test_real_poll_repetition_tables(self):
        for cmd in ("check", "explain"):
            self.assertEqual(self.r["poll", cmd][2], 0)
            text, _ = self.build("poll", cmd)
            rows = [[plain(c) for c in r] for r in table_after(text, "繰り返しの回数と上限後の処理")]
            self.assertEqual(rows[0][1:5], ["poll", "2 〜 2", "0 〜 0", "0 〜 0"])
            self.assertIn("complete", section(text, "conclusion"))

    def test_nondeterministic_repetition_does_not_invent_probabilities(self):
        model = Path(self.tmp) / "rework-nondet.relca"
        model.write_text(self.models["rework"].read_text().replace(
            '  kernel Inspection { * -> { 0.5: pass; 0.5: fail } }\n', "").replace(
            "outcomes kernel Inspection", "result nondet { pass, fail }"))
        out, _, code = relca(self.tmp, "rework-nondet", "check", model)
        self.assertEqual(code, 1)
        raw = json.loads(Path(out).read_text())["repetition"]["cases"][0]
        self.assertIsNone(raw["on_limit_probability"])
        self.assertTrue(all(f["probability"] is None for f in raw["distribution"]))
        text, _ = self.build("rework", model=model, output=out, code=code)
        self.assertEqual(plain(table_after(text, "繰り返しの回数と上限後の処理")[0][4]), "null(算出なし)")
        self.assertTrue(all(plain(r[3]) == "null(算出なし)" for r in table_after(text, "body 開始回数の分布")))

    def test_actual_simulation_shows_observed_counts_not_exhaustive_probabilities(self):
        model = Path(self.tmp) / "poll-sim.relca"
        model.write_text(self.models["poll"].read_text().replace(
            "  verify reachable", "  simulation replications 8 seed 0 confidence 0.95\n  verify reachable"))
        out, _, code = relca(self.tmp, "poll-sim", "simulate", model)
        self.assertEqual(code, 0)
        raw = json.loads(Path(out).read_text())["repetition"]
        self.assertEqual(raw["coverage"], "sampled")
        case = raw["cases"][0]
        self.assertEqual((case["samples"], case["excluded_samples"], case["observed_on_limit"]), (8, 0, 0))
        check, _, rc = relca(self.tmp, "poll-sim-check", "check", model)
        text, _ = page(self.tmp, "simulation", "--model", model, "--check", check, "--check-exit", rc,
                       "--simulate", out, "--simulate-exit", code, "--no-mermaid")
        tables = text[text.index("<h4>simulate</h4>"):]
        row = [plain(c) for c in table_after(tables, "繰り返しの回数と上限後の処理")[0]]
        self.assertEqual(row[1:], ["poll", "2 〜 2", "0 〜 0", "null(算出なし)", "8", "0", "0"])
        dist = [[plain(c) for c in r] for r in table_after(tables, "body 開始回数の分布")]
        self.assertIn(["1", "poll", "2", "null(算出なし)", "8"], dist)

    def test_on_limit_leave_and_body_end_keep_their_distinct_edges(self):
        model = Path(self.tmp) / "poll-controls.relca"
        model.write_text(self.models["poll"].read_text().replace("{ leave }", "{ end ready }").replace(
            "on_limit { end timed_out }", "on_limit { leave }"))
        out, _, code = relca(self.tmp, "poll-controls", "check", model)
        self.assertEqual(code, 1)  # end.done is unreachable; this is not the Poll contract.
        text, _ = self.build("poll", model=model, output=out, code=code)
        graph = self.graph(text)
        ready = re.search(r'(n\d+)\(\["end ready"\]\)', graph)
        limit = re.search(r'(n\d+)\(\["on_limit poll"\]\)', graph)
        assert ready and limit
        self.assertNotRegex(graph, re.escape(ready[1]) + r" -->")
        self.assertRegex(graph, re.escape(limit[1]) + r' -->\|"leave')
        self.assertNotIn("未解釈", section(text, "model"))

    def test_accepted_unreachable_statement_after_end_is_not_called_rejected(self):
        model = Path(self.tmp) / "poll-after-end.relca"
        model.write_text(self.models["poll"].read_text().replace("{ leave }", "{ end done; wait 1min }"))
        out, _, code = relca(self.tmp, "poll-after-end", "check", model)
        self.assertEqual(code, 0)  # Relca rejects statements after again/leave, not after end.
        text, _ = self.build("poll", model=model, output=out, code=code)
        self.assertNotIn("Relca が受け付けない形", section(text, "model"))
        self.assertNotIn('"wait 1min"', self.graph(text))

    def test_repetition_type_errors_unknown_coverage_and_injection_fail_closed(self):
        original, _, code = self.r["rework", "check"]
        variants = {
            "object": lambda d: d.__setitem__("repetition", []),
            "cases": lambda d: d["repetition"].__setitem__("cases", {}),
            "coverage": lambda d: d["repetition"].__setitem__("coverage", "future"),
            "probability": lambda d: d["repetition"]["cases"][0].__setitem__("on_limit_probability", False),
            "phase": lambda d: d["repetition"]["sites"][0]["origin"].__setitem__("phase", "future"),
            "missing": lambda d: d["repetition"]["cases"][0].pop("rounds"),
        }
        for n, change in variants.items():
            with self.subTest(shape=n):
                output = rewrite(original, Path(self.tmp) / (n + "-output.json"), change)
                text, data = self.build("rework", output=output, code=code)
                self.assertTrue(any(u[1].startswith("repetition.") and u[2] == "unsupported"
                                    for u in data["unverified"]))
                self.assertTrue(data["unreadable"])
                self.assertFalse(any("節「繰り返しの出力」を生成できなかった" in a[1] for a in data["alerts"]))
        evil = '</td><script>alert(1)</script><img src=x onerror=alert(2)>'
        output = rewrite(original, Path(self.tmp) / "loop-injection.json", lambda d: (
            d["repetition"].__setitem__("basis", evil),
            d["repetition"]["sites"][0]["origin"].__setitem__("call_site", evil)))
        text, _ = page(self.tmp, "loop-injection", "--model", self.models["rework"],
                       "--check", output, "--check-exit", code, "--no-mermaid")
        self.assertEqual(text.count("<script"), 1)  # 埋め込み JSON だけ。
        self.assertNotIn("<img", text)
        self.assertIn(html.escape(evil, quote=True), text)

    def test_loop_and_via_model_assumption_targets(self):
        for n, expected in (("poll", {"poll.at_most": "繰り返しの上限回数", "poll.gap": "繰り返しの間隔"}),
                            ("rework", {"rework.at_most": "繰り返しの上限回数"}),
                            ("via", {"Extract.via_model": "経由するモデルの版"})):
            with self.subTest(model=n):
                spec = Path(self.tmp) / (n + "-assumptions.json")
                spec.write_text(json.dumps([{"target": t, "status": "confirmed", "confirmed_by": "業務担当"}
                                            for t in expected], ensure_ascii=False))
                text, data = self.build(n, assumptions=spec)
                decls = {d[0]: d for d in data["declarations"]}
                for t, kind in expected.items():
                    self.assertIn(t, decls)
                    self.assertEqual(decls[t][1], kind)
                    self.assertNotIn(t, data["unprovenanced"])
                    self.assertIn(t, data["ai_claims"])
                self.assertFalse([a for a in data["alerts"] if "対応しない" in a[1]])
                self.assertIn("人が未確認", text)
                if n == "rework":
                    self.assertNotIn("rework.gap", decls)  # 省略時既定を宣言にしない。
                if n == "via":
                    self.assertEqual(decls["Extract.via_model"][2], 'via model "doc@v3"')

    def test_not_derived_is_counted_once_per_report_in_terminal_and_page(self):
        model = Path(self.tmp) / "poll-cut.relca"
        model.write_text(self.models["poll"].read_text().replace("clock 20min", "clock 6min"))
        out, err, code = relca(self.tmp, "poll-cut", "check", model)
        self.assertEqual(code, 2)
        raw = json.loads(Path(out).read_text())
        self.assertEqual((raw["repetition"]["coverage"], raw["repetition"]["cases"]), ("not_derived", []))
        terminal = io.StringIO()
        with redirect_stdout(terminal):
            text, data = self.build("poll", model=model, output=out, code=code)
        rows = [u for u in data["unverified"] if u[1] == "repetition.coverage"]
        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0][2], "not_derived")
        self.assertIn("repetition.coverage", section(text, "unverified"))
        count = data["unverified_count"]
        self.assertIn("確かめられていないもの %d 件" % count, terminal.getvalue())
        self.assertIn("確かめられていないもの %d 件" % count, section(text, "unverified"))
        self.assertIn("(%d 件)" % count, section(text, "conclusion"))
        self.assertIn("算出できていない。0回・0件ではない", section(text, "conclusion"))
        self.assertTrue(any("repetition/unchecked" in x for v in data["self_unchecked"].values() for x in v))

    def test_rejected_repeat_forms_are_explicit_not_uninterpreted(self):
        base = self.models["poll"].read_text()
        variants = {
            "over": base.replace("poll at_most 3", "poll over items at_most 3"),
            "nested": base.replace("s = Registry.read()", "repeat inner at_most 1 { wait 1min; again } on_limit { leave }\n s = Registry.read()"),
            "limit-again": base.replace("on_limit { end timed_out }", "on_limit { again }"),
            "fallthrough": base.replace("if s == ready { leave } else { again }", "if s == ready { leave } else {}"),
            "too-many": base.replace("at_most 3", "at_most 33"),
            "zero-limit": base.replace("at_most 3", "at_most 0"),
            "no-limit": base.replace("on_limit { end timed_out }", ""),
            "join-any": base.replace("s = Registry.read()", "fork f { s = Registry.read() }; join any f where result == ready tie by [s] remaining drain if none -> timed_out"),
            "zero-time": base.replace("gap fixed 5min", "gap fixed 0min").replace("duration fixed 1min", "duration fixed 0min"),
            "after-control": base.replace("{ again }", "{ again; wait 1min }"),
            "response-loss": base.replace("s = Registry.read()", "s = Registry.read() response_loss none"),
        }
        for n, source in variants.items():
            with self.subTest(shape=n):
                model = Path(self.tmp) / (n + ".relca")
                model.write_text(source)
                out, _, code = relca(self.tmp, n, "check", model)
                self.assertIn(code, (3, 4))
                text, _ = self.build("poll", model=model, output=out, code=code)
                self.assertIn("Relca が受け付けない形", section(text, "model"))
                self.assertNotIn("未解釈", section(text, "model"))

    def test_not_derived_in_plan_and_mutate_original_keeps_each_origin(self):
        original = json.loads(Path(self.r["poll", "check"][0]).read_text())
        # Transport-only controls: these envelopes are hand-built, not produced by the CLI.
        # These envelopes exercise the page's existing all-report scanner, not new CLI support.
        original["repetition"]["coverage"] = "not_derived"
        original["repetition"]["cases"] = []
        original["repetition"]["unchecked"] = ["iteration_distribution(analysis_incomplete)"]
        compare = Path(self.tmp) / "loop-compare.json"
        compare.write_text(json.dumps({"verdict": "satisfied", "plans": [
            {"configuration": n, "report": original} for n in ("A", "B")]}))
        mutate = Path(self.tmp) / "loop-mutate.json"
        mutate.write_text(json.dumps({"verdict": "satisfied", "original": original}))
        out, _, code = self.r["poll", "check"]
        text, data = page(self.tmp, "nested-reports", "--model", self.models["poll"], "--check", out,
                         "--check-exit", code, "--compare", compare, "--compare-exit", 0,
                         "--compare-model", self.models["poll"], "--mutate", mutate, "--mutate-exit", 0,
                         "--no-mermaid")
        self.assertEqual({u[0] for u in data["unverified"] if u[1] == "repetition.coverage"},
                         {"compare 案 A", "compare 案 B", "mutate original"})
        for origin in ("compare 案 A", "compare 案 B", "mutate original"):
            self.assertIn("<h4>%s</h4>" % origin, text)


@unittest.skipUnless(RELCA.exists(), "relca の binary が必要")
class MergePageTest(unittest.TestCase):
    def setUp(self):
        self.tmpdir = tempfile.TemporaryDirectory(prefix="merge-page-", dir=ROOT / ".tmp")
        self.tmp = self.tmpdir.name
        self.model = ROOT / "crates/relca-core/tests/merge_if/models/M1.relca"

    def tearDown(self):
        self.tmpdir.cleanup()

    def test_real_merge_and_verified_event_are_not_tasks_or_hidden_steps(self):
        check, _, code = relca(self.tmp, "merge-check", "check", self.model)
        explain, _, xcode = relca(self.tmp, "merge-explain", "explain", self.model)
        self.assertEqual((code, xcode), (1, 1))
        text, data = page(self.tmp, "merge", "--model", self.model, "--check", explain,
                          "--check-exit", xcode)
        model = section(text, "model")
        self.assertIn("d2 = if r == fail", model)
        self.assertIn("yield f", model)
        self.assertIn("yield d", model)
        self.assertIn("値の複写(Task なし)", model)
        self.assertNotIn("未解釈", model)
        self.assertIn("d2 ← f(Task なし)", html.unescape(text))
        self.assertIn("merged", text)
        self.assertIn("repaired", text)
        self.assertTrue(any("merge_control" in str(row) for row in data["not_derived"]))
        raw = json.loads(check.read_text())
        self.assertEqual({t["binding"] for t in raw["metrics"]["cases"][0]["tasks"]}, {"d", "r", "f", "s"})

    def test_rejected_merge_is_not_drawn_as_an_accepted_flow(self):
        base = self.model.read_text()
        merge = "d2 = if r == fail { f = Clerk.fix(d); yield f } else { yield d }"
        variants = {
            "fallthrough": (base.replace("yield f", ""), "merge_fallthrough"),
            "no-else": (base.replace("else { yield d }", ""), None),
            "all-end": (base.replace(merge, "d2 = if r == fail { end done_fixed } else { end done_as_is }"), "merge_no_continuing_branch"),
            "after-yield": (base.replace("yield f", "yield f; wait 1min"), "merge_unreachable_after_yield"),
            "outside": (base.replace(merge, "yield d"), "merge_yield_outside"),
            "undefined": (base.replace("yield f", "yield missing"), "undefined merge source"),
            "qualified-latent": (base.replace("actor Clerk { capacity 1", "actor Clerk { capacity 1; latent secret:D=draft")
                                 .replace("yield f", "yield Clerk.secret"), "latent"),
        }
        for name, (body, reason) in variants.items():
            with self.subTest(shape=name):
                source = Path(self.tmp) / (name + ".relca")
                source.write_text(body)
                check, _, code = relca(self.tmp, name, "check", source)
                self.assertEqual(code, 3)
                text, _ = page(self.tmp, name, "--model", source, "--check", check, "--check-exit", code)
                model = section(text, "model")
                self.assertIn("Relca が受け付けない形", model)
                if reason:
                    self.assertIn(reason, model)
                self.assertNotIn("未解釈", model)

    def test_nested_merge_keeps_selected_sources_and_yield_can_still_be_a_name(self):
        source = ROOT / "crates/relca-core/tests/merge_if/models/M2.relca"
        src = review_page.Source(source.read_text())
        steps = review_page.flatten_steps(review_page.parse_process(src, src.items_of("process")[0][1]))
        self.assertEqual({s["binding"] for s in steps if s["kind"] == "merge"}, {"d2", "g"})
        self.assertEqual({(s["binding"], s["source"]) for s in steps if s["kind"] == "yield"},
                         {("g", "f"), ("g", "d"), ("d2", "g")})
        named = Path(self.tmp) / "yield-name.relca"
        named.write_text(self.model.read_text().replace("d = Clerk.write()", "yield = Clerk.write(); d = Clerk.write()"))
        src = review_page.Source(named.read_text())
        steps = review_page.flatten_steps(review_page.parse_process(src, src.items_of("process")[0][1]))
        self.assertTrue(any(s["kind"] == "call" and s["binding"] == "yield" for s in steps))


@unittest.skipUnless(RELCA.exists() or "RELCA_BIN" in os.environ, "relca の binary が無い")
class EvidenceBasisPageTest(unittest.TestCase):
    def setUp(self):
        (ROOT / ".tmp").mkdir(exist_ok=True)
        self.directory = tempfile.TemporaryDirectory(prefix="evidence-review-", dir=ROOT / ".tmp")
        self.tmp = self.directory.name
        self.addCleanup(self.directory.cleanup)
        self.model = ROOT / "examples/evidence-basis-missing.relca"
        self.out, self.err, self.code = relca(self.tmp, "missing", "explain", self.model)
        self.assertEqual(self.code, 1)

    def build(self, name, output=None):
        return page(self.tmp, name, "--model", self.model, "--check", output or self.out,
                    "--check-exit", self.code, "--check-stderr", self.err, "--no-mermaid")

    def basis_page(self, name, model, configuration, code, command="explain", fault=None):
        out, err, actual = relca(self.tmp, name, command, model, "--configuration", configuration)
        self.assertEqual(actual, code, msg=err.read_text())
        if fault:
            def change(report):
                fault(report["explanation"]["counterexamples"][0]["evidence"])
            out = rewrite(out, Path(self.tmp) / (name + "-fault.json"), change)
        return page(self.tmp, name + "-page", "--model", model, "--check", out,
                    "--check-exit", code, "--check-stderr", err, "--no-mermaid")

    def conditional_model(self):
        # The x06 regression: Doc may reach Notes, Extra cannot, Policy does.
        model = Path(self.tmp) / "conditional.relca"
        model.write_text('''model ConditionalEvidence version "0.2" {
  type V = enum { pass, fail }
  kernel Review { * -> { 0.5: pass; 0.5: fail } }
  observed case { brief: optional<V> }
  actor Doc { source document }
  actor Policy { source document }
  actor Extra { source document }
  actor Notes {}
  actor Clerk { capacity 1
    capability intake() -> V { reads [Doc]; writes brief; duration fixed 1min; result nondet { pass } }
    capability patch() -> V { writes brief; duration fixed 1min; result nondet { pass } }
    capability draft() -> V { reads [brief]; duration fixed 1min; result nondet { pass } }
    capability pol() -> V { reads [Policy]; duration fixed 1min; result nondet { pass } }
    capability review() -> V { duration fixed 1min; outcomes kernel Review }
  }
  actor Boss { capacity 1
    capability register(d: V, e: V) -> V { registers [Notes]; duration fixed 1min; result nondet { pass } }
    capability use() -> V { reads [Notes]; duration fixed 1min; result nondet { pass } }
  }
  cut C { include [Doc, Policy, Extra, Notes, Clerk, Boss] }
  process Work { i = Clerk.intake(); g = Clerk.review(); if g == pass { p = Clerk.patch() } else {}; d = Clerk.draft(); e = Clerk.pol(); s = Boss.register(d, e); u = Boss.use(); end done }
  configuration Hand { cut C; process Work }
  bounds { cases 1; live_tasks 2; clock 60min }
  contract Basis { evidence_basis Boss.use [Policy, Doc, Extra] }
}
''', encoding="utf-8")
        return model

    def test_conditional_sources_are_not_missing_or_counted_sources(self):
        text, _ = self.basis_page("conditional", self.conditional_model(), "Hand", 1)
        rows = list(map(lambda r: tuple(map(plain, r)), table_after(text, "<h5>モデルの根拠源の証跡")))
        for row in (("照合に数えた源", "Policy"), ("不足した源", "Extra"),
                    ("届きうるが確かめられない源", "Doc")):
            self.assertIn(row, rows)

    def loop_model(self, number):
        return ROOT / "crates/relca-core/tests/bounded_repetition/models" / f"eb-alias-e{number}.relca"

    def test_exit_table_keeps_number_phase_and_round(self):
        model = self.loop_model(5)
        cases = [("limit", model, ["rework", "2 回目", "上限到達後の処理 (on_limit)", "2 番目の leave"]),
                 ("two-loops", self.loop_model(6), ["rework", "2 回目", "本体", "番号の記録なし"])]
        body = Path(self.tmp) / "body.relca"
        body.write_text(model.read_text().replace("reads [Doc]; writes brief", "writes brief"))
        cases.append(("body", body, ["rework", "1 回目", "本体", "1 番目の leave"]))
        for name, model, row in cases:
            with self.subTest(name=name):
                text, _ = self.basis_page(name, model, "Loop", 1)
                exits = [list(map(plain, r)) for r in table_after(text, "<h5>反例で通った繰り返しの出口")]
                expected = [row] + ([["note", "1 回目", "本体", "番号の記録なし"]] if name == "two-loops" else [])
                self.assertEqual(exits, expected)
        text, _ = self.basis_page("limit-hand", self.loop_model(5), "Hand", 1)
        self.assertIn(["不足した源", "Doc"], [list(map(plain, r)) for r in table_after(text, "<h5>モデルの根拠源の証跡")])
        self.assertNotIn("<h5>反例で通った繰り返しの出口", text)

    def test_counterexample_inside_repeat_names_original_call(self):
        model = Path(self.tmp) / "inside.relca"
        model.write_text(self.loop_model(5).read_text().replace("evidence_basis Boss.use", "evidence_basis Clerk.review"))
        text, _ = self.basis_page("inside", model, "Loop", 1)
        rows = [list(map(plain, r)) for r in table_after(text, "<h5>モデルの根拠源の証跡")]
        self.assertIn(["繰り返し内の呼出し", "rework / 1 回目 / 本体 / 元の呼出し d"], rows)

    def test_conditional_only_does_not_invent_a_counterexample_or_reason(self):
        for command in ("check", "explain"):
            text, info = self.basis_page("unsupported-" + command, self.loop_model(4), "Loop", 4, command)
            self.assertNotIn("<h5>モデルの根拠源の証跡", text)
            rows = [list(map(plain, r)) for r in table_after(text, "<h4>確かめられていない性質")]
            row, = [r for r in rows if r[0] == "contract Basis #1 evidence_basis"]
            self.assertIn("unsupported", row[1])
            issue, = [u for u in info["unverified"] if u[:3] == [command, row[0], "unsupported"]]
            if command == "check":
                self.assertIn("届きうるが確かめられない源がある", row[-1])
                self.assertIn("evidence_basis_conditional_snapshot", issue[-1])
            else:
                self.assertIn("理由の記録なし", row[-1])
                self.assertIn("check --configuration Loop", row[-1])
                self.assertNotIn("evidence_basis_conditional_snapshot", issue[-1])

    def test_bad_optional_evidence_does_not_render_as_a_witness(self):
        conditional = self.conditional_model()
        for name, model, cfg, fault in (
            ("overlap", conditional, "Hand", lambda e: e.update(conditional_sources=["Extra"])),
            ("unrequested", conditional, "Hand", lambda e: e.update(conditional_sources=["Doc", "Other"])),
            ("wrong-conditional", conditional, "Hand", lambda e: e.update(conditional_sources="Doc")),
            ("wrong-site", self.loop_model(5), "Loop", lambda e: e["repetition_exits"][0].update(leave_site=True)),
            ("wrong-phase", self.loop_model(5), "Loop", lambda e: e["repetition_exits"][0].update(phase="else")),
            ("unknown-exit-key", self.loop_model(5), "Loop", lambda e: e["repetition_exits"][0].update(extra=1)),
        ):
            with self.subTest(name=name):
                text, info = self.basis_page(name, model, cfg, 1, fault=fault)
                self.assertNotIn("<h5>モデルの根拠源の証跡", text)
                self.assertTrue(any(u[:2] == ["review_page", "根拠源の証跡"] for u in info["unverified"]))

    def test_positive_example_keeps_hand_times_and_both_read_mutations(self):
        model = ROOT / "examples/evidence-basis.relca"
        out, _, code = relca(self.tmp, "positive", "check", model)
        self.assertEqual(code, 0)
        value = json.loads(out.read_text())
        row = next(p for p in value["properties"] if p["id"] == "contract Basis #1 evidence_basis")
        self.assertEqual(row, {"id": "contract Basis #1 evidence_basis", "verdict": "satisfied"})
        self.assertEqual(len(value["terminals"]), 1)
        self.assertEqual(value["terminals"][0]["elapsed"], {"min": 2, "max": 2})
        self.assertEqual(value["terminals"][0]["work"], {"min": 2, "max": 2})
        out, _, code = relca(self.tmp, "positive-mutate", "mutate", model, "--kinds", "evidence_missing")
        self.assertEqual(code, 0)
        value = json.loads(out.read_text())
        mutants = next(k for k in value["mutations"] if k["kind"] == "evidence_missing")["mutants"]
        self.assertEqual(len(mutants), 2)
        self.assertEqual({m["result"] for m in mutants}, {"detected"})

    def test_real_counterexample_has_source_clause_and_evidence(self):
        raw = json.loads(self.out.read_text())
        trace = raw["explanation"]["counterexamples"][0]
        self.assertEqual(trace["replay"]["status"], "verified")
        self.assertEqual(trace["evidence"]["missing"], ["Invoice"])
        text, data = self.build("valid")
        self.assertIn("evidence_basis Clerk.match_po [Invoice,PurchaseOrder]", text)
        self.assertIn("モデルの根拠源の証跡", text)
        self.assertIn("要求した源", text)
        self.assertIn("照合に数えた源", text)
        self.assertIn("不足した源", text)
        self.assertIn("実際の資料アクセスを観測したログではない", text)
        self.assertFalse(any(u[0] == "review_page" and u[1] == "根拠源の証跡" for u in data["unverified"]))

    def test_direct_clause_remains_contextual_source_text(self):
        source = self.model.read_text().replace("evidence_basis Clerk.match_po [Invoice,PurchaseOrder]",
                                               "evidence_basis direct Clerk.match_po [Invoice]")
        model = Path(self.tmp) / "direct.relca"
        model.write_text(source)
        out, err, code = relca(self.tmp, "direct", "explain", model)
        self.assertEqual(code, 1)
        text, _ = page(self.tmp, "direct-page", "--model", model, "--check", out,
                       "--check-exit", code, "--check-stderr", err, "--no-mermaid")
        self.assertIn("evidence_basis direct Clerk.match_po [Invoice]", text)
        self.assertIn("モデルの根拠源の証跡", text)

    def test_missing_malformed_and_inconsistent_evidence_is_unverified(self):
        def evidence(value):
            return value["explanation"]["counterexamples"][0]["evidence"]

        mutations = {
            "absent": lambda v: v["explanation"]["counterexamples"][0].pop("evidence"),
            "wrong-type": lambda v: evidence(v).__setitem__("sources", None),
            "wrong-missing": lambda v: evidence(v).__setitem__("missing", []),
            "unordered": lambda v: evidence(v).__setitem__("required", ["PurchaseOrder", "Invoice"]),
            "unknown-key": lambda v: evidence(v).__setitem__("assumed", True),
            "wrong-call": lambda v: evidence(v).__setitem__("actor", "Other"),
        }
        for name, change in mutations.items():
            with self.subTest(fault=name):
                altered = rewrite(self.out, Path(self.tmp) / (name + ".json"), change)
                text, data = self.build(name, altered)
                self.assertIn("この節を生成できなかった", text)
                self.assertTrue(any(u[0] == "review_page" and u[1] == "根拠源の証跡" for u in data["unverified"]))


@unittest.skipUnless(RELCA.exists(), "relca の binary が無い")
class ContractScopePageTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="relca-scope-page-", dir=ROOT / ".tmp")
        self.addCleanup(self.temp.cleanup)
        self.tmp = self.temp.name
        self.model = ROOT / "examples/configuration-contracts.relca"
        self.out, self.err, self.code = relca(self.tmp, "old", "check", self.model, "--configuration", "Old")
        self.assertEqual(self.code, 0)

    def build(self, name, out=None):
        return page(self.tmp, name, "--model", self.model, "--check", out or self.out,
                    "--check-exit", self.code, "--no-mermaid")

    def test_real_scope_is_displayed_once_and_counted_without_folding_it(self):
        text, data = self.build("old-page")
        rows = [r for r in data["unverified"] if r[1] == "contract Controls #1 authority_basis"]
        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0][2], "not_applicable")
        self.assertIn("Oldには承認画面がない", text)
        self.assertIn("applies_to [New]", text)
        self.assertIn("not_applicable / verdict なし", text)
        self.assertNotIn("contract_scope", data["unread_keys"])
        self.assertFalse(any("全体と案件ごとの判定が矛盾" in str(a) for a in data["alerts"]))

    def test_applied_source_header_remains_visible_without_an_excluded_row(self):
        out, _, code = relca(self.tmp, "new", "check", self.model, "--configuration", "New")
        self.assertEqual(code, 0)
        text, data = self.build("new-page", out)
        self.assertIn("applies_to [New]", text)
        self.assertIn("Oldには承認画面がない", text)
        self.assertFalse(any(r[1] == "contract Controls #1 authority_basis" for r in data["unverified"]))

    def test_mutate_root_scope_is_a_copy_not_an_extra_unverified_contract(self):
        out, _, code = relca(self.tmp, "mutate", "mutate", self.model, "--configuration", "Old", "--kinds", "authority_expired")
        self.assertEqual(code, 5)
        text, data = page(self.tmp, "mutate-page", "--model", self.model, "--check", self.out,
                          "--check-exit", self.code, "--mutate", out,
                          "--mutate-exit", code, "--no-mermaid")
        rows = [r for r in data["unverified"] if r[1] == "contract Controls #1 authority_basis" and r[0].startswith("mutate")]
        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0][0], "mutate original")
        self.assertIn("not_applicable", text)

    def test_compare_non_applicable_cell_is_not_an_unsupported_verdict(self):
        out, err, code = relca(self.tmp, "compare", "compare", self.model,
                               "--plans", "Old,New", "--mode", "exhaustive")
        self.assertEqual(code, 0)
        text, data = page(self.tmp, "compare-page", "--model", self.model, "--check", self.out,
                          "--check-exit", 0, "--compare", out, "--compare-exit", code,
                          "--compare-stderr", err, "--compare-model", self.model, "--no-mermaid")
        self.assertIn("非適用(not_applicable、verdict なし)", text)
        self.assertTrue(any(r[0] == "compare 案 Old" and r[2] == "not_applicable" for r in data["unverified"]))

    def test_malformed_scope_rows_are_unverified_not_silently_omitted(self):
        changes = {
            "missing": lambda r: r.pop("reason"),
            "status": lambda r: r.__setitem__("status", "satisfied"),
            "mask": lambda r: r.__setitem__("masks_call", "false"),
            "duplicate": lambda r: r.__setitem__("applies_to", ["New", "New"]),
        }
        for name, change in changes.items():
            with self.subTest(name=name):
                out = rewrite(self.out, Path(self.tmp) / (name + ".json"), lambda v: change(v["contract_scope"][0]))
                _, data = self.build(name, out)
                self.assertTrue(any(r[1] == "contract_scope" and r[2] == "unsupported" for r in data["unverified"]))


@unittest.skipUnless(RELCA.exists(), "relca の binary が無い")
class IntentPageTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="relca-intent-page-", dir=ROOT / ".tmp")
        self.addCleanup(self.temp.cleanup)
        self.tmp = self.temp.name
        self.source = (ROOT / "examples/capability-replacement.relca").read_text()

    def build(self, name, source):
        model = Path(self.tmp) / (name + ".relca")
        model.write_text(source)
        out, err, code = relca(self.tmp, name + "-check", "check", model, "--configuration", "B")
        self.assertEqual(code, 0)
        compare, compare_err, compare_code = relca(self.tmp, name + "-compare", "compare", model,
                                                  "--plans", "A,B,C", "--mode", "exhaustive")
        self.assertEqual(compare_code, 0)
        return page(self.tmp, name, "--model", model, "--check", out, "--check-exit", code,
                    "--check-stderr", err, "--compare-model", model, "--compare", compare,
                    "--compare-exit", compare_code, "--compare-stderr", compare_err, "--no-mermaid")

    def test_pair_replace_source_and_compare_unchecked_are_not_a_diff_claim(self):
        text, data = self.build("pairs", self.source)
        self.assertIn('id="intent-declarations"', text)
        self.assertEqual(text.count("intent from A to B {"), 1)
        self.assertEqual(text.count("intent from C to B {"), 1)
        self.assertIn("replace Clerk.check with Bot.check", text)
        self.assertIn("Robot の保守契約", text)
        self.assertIn("差分との照合ではない", text)
        self.assertIn('id="intent-unchecked"', text)
        self.assertIn("intent", data["self_unchecked"])
        self.assertNotIn("intent", data["unread_keys"])

    def test_empty_default_and_pair_declarations_are_displayed_without_merging(self):
        bare = re.sub(r"  intent from [AC] to B \{[^}]*\}\n", "", self.source)
        for name, source, declared in [
            ("none", bare, False),
            ("default", bare.replace("configuration B { cut K; process PB }",
                                      "configuration B { cut K; process PB; intent { } }"), True),
            ("empty-pair", bare.replace("  bounds", "  intent from A to B { }\n  bounds"), True),
        ]:
            with self.subTest(name=name):
                text, data = self.build(name, source)
                self.assertEqual('id="intent-declarations"' in text, declared)
                self.assertEqual("intent" in data["self_unchecked"], declared)
                if name == "default":
                    self.assertIn("B の既定", text)
                    self.assertIn("intent { }", text)
                elif name == "empty-pair":
                    self.assertIn("intent from A to B { }", text)

    def test_intent_reason_is_escaped_as_source_not_html(self):
        text, _ = self.build("escape", self.source.replace("照合を自動化", "<確認>&"))
        self.assertIn("&lt;確認&gt;&amp;", text)
        self.assertNotIn("<確認>", text)


@unittest.skipUnless(RELCA.is_file(), "relca がない。cargo build -p relca-cli が必要")
class EstimatePageTest(unittest.TestCase):
    def render(self, outcome="outcomes kernel K", pass_p="0.5", fail_p="0.5", minimum=1,
               change=None, supplied_exit=None, censored=False, decimals=2):
        with tempfile.TemporaryDirectory() as tmp, redirect_stdout(io.StringIO()):
            model = Path(tmp) / "input.relca"
            kernel = f"kernel K {{ * -> {{ {pass_p}: pass; {fail_p}: fail }} }}" if "kernel" in outcome else ""
            model.write_text(f'''model M version "0.2" {{
              type T = enum {{ pass, fail }}
              people [p]
              {kernel}
              actor A {{ executors [p]; capability run() -> T {{ duration fixed 1min; {outcome} }} }}
              process P {{ r = A.run(); end done }}
              cut C {{ include [A] }}
              bounds {{ cases 1; live_tasks 1; clock 10min }}
              verify reachable end.done
            }}''', encoding="utf-8")
            log = Path(tmp) / "events.jsonl"
            cases = [{"case_id": str(i), "from_case_start": True, "through_case_end": True} for i in range(4)]
            lines = [{"kind": "header", "schema": "relca-log/v1", "clock": "log", "time_unit": "min",
                      "window": {"from": 0, "to": 1}, "cases": cases,
                      "actor_map": [{"source_id": "a", "actor": "A"}],
                      "executor_map": [{"source_id": "p", "person": "p"}]}]
            for i, result in enumerate(("pass", "fail", "pass", "fail")):
                for instant, phase in enumerate(("start", "finish")):
                    if censored and i >= 2 and phase == "finish":
                        continue
                    lines.append({"kind": "event", "event_id": f"{i}-{phase}", "case_id": str(i), "task_id": "t",
                                  "operation": "A.run", "lifecycle": phase, "cancel_phase": None,
                                  "time": {"earliest": instant, "latest": instant}, "after": [],
                                  "actor_id": "a", "executor_id": "p", "input_version": None, "output_version": None,
                                  "result": result if instant else None})
            log.write_text("".join(json.dumps(line) + "\n" for line in lines), encoding="utf-8")
            spec = Path(tmp) / "spec.json"
            spec.write_text(json.dumps({"schema": "relca-estimate-spec/v1",
                                        "model_sha256": review_page.sha256_file(model),
                                        "log_sha256": review_page.sha256_file(log), "slots": "all",
                                        "confidence": "1079/2048", "decimals": decimals, "max_support": 4,
                                        "min_samples": minimum}), encoding="utf-8")
            report, _, check_exit = relca(tmp, "check", "check", model)
            self.assertEqual(check_exit, 0)
            estimate, err, exit_code = relca(tmp, "estimate", "estimate", model, "--log", log, "--spec", spec)
            expected_exit = 4 if "nondet" in outcome else 2 if minimum > 4 or censored or decimals == 0 else 0
            self.assertEqual(exit_code, expected_exit, err.read_text())
            if change:
                estimate = rewrite(estimate, Path(tmp) / "changed.json", change)
            return page(tmp, "estimate", "--model", model, "--check", report, "--check-exit", check_exit,
                        "--estimate", estimate, "--estimate-exit", exit_code if supplied_exit is None else supplied_exit,
                        "--no-mermaid")

    def test_matching_candidates_supply_provenance(self):
        text, data = self.render()
        self.assertNotIn("A.run.duration", data["unprovenanced"])
        self.assertNotIn("kernel.K", data["unprovenanced"])
        self.assertIn("relca がログから導出(n=4", text)
        self.assertIn("ログの窓 0〜1 分", text)
        self.assertIn("ログの真正性と代表性は人が未確認", text)
        self.assertIn("[1/8, 7/8]", text)

    def test_revise3_partial_candidate_is_not_provenance_even_when_equal(self):
        text, data = self.render(censored=True)
        self.assertIn("kernel.K", data["unprovenanced"])
        self.assertIn("ログからの候補あり(一部未確定: 既知 2 件 / 4 件、打切り 2 件)", text)
        for label in ("n=4", "unknown=2", "censored=2", "coverage=partial"):
            self.assertIn(label, text)

    def test_revise3_exact_only_candidate_keeps_other_provenance(self):
        text, data = self.render(decimals=0)
        self.assertIn("厳密値のみ（DSL の桁不足）", text)
        self.assertIn("dsl_precision_insufficient", text)
        self.assertIn("kernel.K", data["unprovenanced"])
        self.assertNotIn("A.run.duration", data["unprovenanced"])

    def test_outside_interval_warns_without_provenance_claim(self):
        text, data = self.render(pass_p="0.99", fail_p="0.01")
        self.assertIn("relca がログから導出", text)
        self.assertIn("宣言値はログから導いた区間の外", text)
        self.assertIn("kernel.K", data["unprovenanced"])
        self.assertNotIn("A.run.duration", data["unprovenanced"])

    def test_hash_and_exit_mismatch_are_not_used(self):
        for kwargs, warning in [({"change": lambda d: d["model"].update(sha256="0" * 64)}, "sha256 が違う"),
                                ({"supplied_exit": 2}, "verdict と exit を照合できない")]:
            with self.subTest(warning=warning):
                text, data = self.render(**kwargs)
                self.assertIn(warning, text)
                self.assertNotIn("relca がログから導出", text)
                self.assertIn("A.run.duration", data["unprovenanced"])
                self.assertIn("kernel.K", data["unprovenanced"])

    def test_absent_values_have_a_separate_table(self):
        text, data = self.render(outcome="result nondet { pass, fail }", minimum=5)
        rows = [tuple(map(plain, row[:3])) for row in table_after(text, "ログから候補を出せなかった欄")]
        self.assertEqual(rows, [("duration.A.run", "withheld", "insufficient_samples"),
                                ("nondet.A.run", "not_derivable", "nondeterministic")])
        self.assertIn("A.run.duration", data["unprovenanced"])
        self.assertNotIn("relca がログから導出", text)


if __name__ == "__main__":
    unittest.main(verbosity=2)
