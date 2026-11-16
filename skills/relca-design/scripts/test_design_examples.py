#!/usr/bin/env python3
"""relca-design の手順で使った主張を、relca を実際に回して確かめる。

examples/invoice の元のモデルと、skills/relca-design/examples の改訂版を比べる。
期待値は references/invoice-walkthrough.md の手計算で、実行結果から写したものではない。
主な主張には、改訂前(または壊した版)で期待した理由で外れることも対で置く。
修理案の時間・作業・差戻しの値は一致を見るだけで、壊した対は無い。
skip は unittest では exit 0 だが合格ではない。

relca の binary は環境変数 RELCA_BIN、無ければ <repo>/target/debug/relca。
無ければ skip する(skip は合格ではない。先に cargo build -p relca-cli)。

  python3 -B skills/relca-design/scripts/test_design_examples.py
"""

import json
import os
import subprocess
import tempfile
import unittest
from fractions import Fraction
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
RELCA = Path(os.environ.get("RELCA_BIN", ROOT / "target" / "debug" / "relca"))
INV = ROOT / "examples" / "invoice"
EX = HERE.parent / "examples"
STRONG = EX / "as-is-strengthened.relca"
REVIEW = EX / "to-be-a-review.relca"


def run(*args):
    p = subprocess.run([str(RELCA), *map(str, args)], capture_output=True, text=True)
    try:
        out = json.loads(p.stdout)
    except json.JSONDecodeError:
        out = None
    return p.returncode, out, p.stderr


def props(report):
    return {p["id"]: p for p in report["properties"]}


def prob(report):
    (p,) = [p for p in report["properties"] if "probability" in p]
    return Fraction(p["probability"]["min"]), Fraction(p["probability"]["max"])


def mutants(report):
    """(kind, actor, capability, source) -> [(result, reason)]"""
    out = {}
    for m in report["mutations"]:
        for mu in m["mutants"]:
            s = mu["site"]
            key = (m["kind"], s.get("actor"), s.get("capability"), s.get("source"))
            out.setdefault(key, []).append((mu["result"], mu["reason"]))
    return out


def edited(tmp, src, name, old, new):
    text = Path(src).read_text()
    assert text.count(old) == 1, (src, old)
    path = Path(tmp) / name
    path.write_text(text.replace(old, new))
    return path


@unittest.skipUnless(RELCA.exists(), f"relca binary not found: {RELCA}")
class Strengthen(unittest.TestCase):
    """手順2: 呼ばれる操作の見逃しを、業務上必要な契約で塞ぐ。"""

    def test_baseline_gap_then_closed(self):
        code, before, _ = run("mutate", INV / "as-is.relca")
        self.assertEqual(code, 4)
        b = mutants(before)
        self.assertEqual(b[("evidence_missing", "Clerk", "enter", "Invoice")], [("undetected", "no_worsening")])
        self.assertEqual(b[("summary_reused", "Clerk", "match_po", None)], [("undetected", "no_assigned_check")])

        code, check, _ = run("check", STRONG)
        self.assertEqual(code, 0)
        self.assertEqual(check["verdict"], "satisfied")
        self.assertEqual(prob(check), (Fraction(93, 125000),) * 2)
        kinds = [i for i in props(check) if "evidence_basis" in i or "same_version" in i]
        self.assertEqual(len(kinds), 3)

        code, after, _ = run("mutate", STRONG)
        self.assertEqual(code, 4)
        a = mutants(after)
        self.assertEqual(a[("evidence_missing", "Clerk", "enter", "Invoice")], [("detected", None)])
        self.assertEqual(a[("summary_reused", "Clerk", "match_po", None)], [("undetected", "no_worsening")])
        self.assertEqual(a[("summary_reused", "Clerk", "match_receipt", None)], [("undetected", "no_worsening")])
        # 契約を足しても変わらない2箇所。extract は呼ばれない、recheck は異議経路で呼ばれうる
        self.assertEqual(a[("evidence_missing", "Clerk", "recheck", "Invoice")], [("undetected", "no_worsening")])
        self.assertEqual(a[("evidence_missing", "Extractor", "extract", "Invoice")], [("undetected", "no_worsening")])
        starts = {c["capability"]: c["starts"] for c in check["metrics"]["capabilities"]}
        self.assertNotIn("Extractor.extract", starts)
        self.assertEqual((starts["Clerk.recheck"]["min"], starts["Clerk.recheck"]["max"]), ("0", "1"))
        self.assertEqual((starts["Clerk.enter"]["min"], starts["Clerk.enter"]["max"]), ("1", "1"))

    def test_binding_named_like_observed_variable_is_refused(self):
        with tempfile.TemporaryDirectory() as tmp:
            text = STRONG.read_text()
            start = text.index("process AsIsFlow")
            bad = Path(tmp) / "bad.relca"
            bad.write_text(text[:start] + text[start:].replace("en =", "entry =").replace("(en", "(entry"))
            code, _, err = run("check", bad)
        self.assertEqual(code, 4)
        self.assertIn("same_version_feature_combination", err)


@unittest.skipUnless(RELCA.exists(), f"relca binary not found: {RELCA}")
class Repair(unittest.TestCase):
    """手順3-4: 違反と空虚な統制を見つけ、構造を足して直し、余裕を測る。"""

    def test_original_violates_and_control_goes_vacuous(self):
        code, r, _ = run("check", INV / "to-be-a.relca")
        self.assertEqual(code, 1)
        self.assertEqual(prob(r), (Fraction(231, 125000),) * 2)
        self.assertEqual(props(r)["contract InvoiceControls #5 separation"]["verdict"], "vacuous")
        starts = {c["capability"] for c in r["metrics"]["capabilities"]}
        self.assertNotIn("Clerk.enter", starts)

    def test_lower_error_rate_still_vacuous(self):
        # 0.0024 * 0.02 + 0.012 * 0.07 = 0.000888: 確率は満たすが、分離は空虚なまま
        old = "difficult -> { 0.85: high with extraction_correct = true; 0.15: high with extraction_correct = false }"
        new = "difficult -> { 0.93: high with extraction_correct = true; 0.07: high with extraction_correct = false }"
        with tempfile.TemporaryDirectory() as tmp:
            path = edited(tmp, INV / "to-be-a.relca", "ed07.relca", old, new)
            code, r, _ = run("check", path)
        self.assertEqual(code, 5)
        self.assertEqual(prob(r), (Fraction(111, 125000),) * 2)
        self.assertEqual(props(r)["probability reached #1"]["verdict"], "satisfied")
        self.assertEqual(props(r)["contract InvoiceControls #5 separation"]["verdict"], "vacuous")

    def test_repaired_design(self):
        code, r, err = run("check", REVIEW)
        self.assertEqual(code, 0, err)
        self.assertEqual(prob(r), (Fraction(231, 250000),) * 2)
        p = props(r)
        sep = [i for i in p if i.endswith("separation")]
        self.assertEqual([p[i]["verdict"] for i in sep], ["satisfied"])
        ends = {t["end"]: t for t in r["terminals"]}
        self.assertEqual(ends["posted"]["elapsed"], {"min": 11, "max": 11})
        self.assertEqual(ends["posted"]["work"], {"min": 14, "max": 14})
        self.assertEqual(Fraction(ends["returned"]["probability"]["min"]), Fraction(11269, 250000))
        self.assertIn("W-LIN-2: vV Clerk.review_entry", err)
        self.assertIn("W-LIN-2: approvalV Approver.approve", err)

    def test_direct_basis_closes_review_gap(self):
        code, r, _ = run("mutate", REVIEW)
        self.assertEqual(code, 4)
        m = mutants(r)
        self.assertEqual(m[("evidence_missing", "Clerk", "review_entry", "Invoice")], [("detected", None)])
        self.assertEqual(m[("original_access_removed", "Clerk", "review_entry", "Invoice")], [("detected", None)])
        with tempfile.TemporaryDirectory() as tmp:
            weak = edited(tmp, REVIEW, "weak.relca", "    evidence_basis direct Clerk.review_entry [Invoice]\n", "")
            code, r, _ = run("mutate", weak)
        self.assertEqual(code, 4)
        m = mutants(r)
        self.assertEqual(m[("evidence_missing", "Clerk", "review_entry", "Invoice")], [("undetected", "no_worsening")])

    def test_detection_margin(self):
        # 0.001848 * (1 - d) <= 0.001  <=>  d >= 0.4589...
        old = "false -> { 0.5: pass; 0.5: fail }"
        with tempfile.TemporaryDirectory() as tmp:
            for d, keep, code_expected, value in (
                ("0.45", "0.55", 1, Fraction(2541, 2500000)),
                ("0.46", "0.54", 0, Fraction(6237, 6250000)),
            ):
                path = edited(tmp, REVIEW, f"d{d}.relca", old, f"false -> {{ {keep}: pass; {d}: fail }}")
                code, r, _ = run("check", path)
                self.assertEqual(code, code_expected, d)
                self.assertEqual(prob(r), (value,) * 2, d)

    def test_reserved_word_as_capability_name(self):
        with tempfile.TemporaryDirectory() as tmp:
            text = REVIEW.read_text().replace("review_entry", "verify")
            bad = Path(tmp) / "bad.relca"
            bad.write_text(text)
            code, _, err = run("check", bad)
        self.assertEqual(code, 3)
        self.assertIn("E-SYN-0", err)


@unittest.skipUnless(RELCA.exists(), f"relca binary not found: {RELCA}")
class Pitfalls(unittest.TestCase):
    """引っかかりやすい点: 当たらない defer が値を全部取り下げ、原因が W-CUT-2 で分かる。"""

    OPAQUE = (
        "    defer Resubmission reason \"one receipt only\" loses \"ultimate settlement\"\n",
        "    defer Vendor.contest_rate reason \"fixed recheck duration is not measured\" loses \"frequency and effort\"\n",
    )

    def test_opaque_defer_withdraws_and_names_cause(self):
        code, r, err = run("check", INV / "phase1.relca", "--configuration", "AsIs")
        self.assertEqual(code, 0, err)
        lines = [l for l in err.splitlines() if l.startswith("W-CUT-2:")]
        self.assertEqual(len(lines), 2)
        self.assertIn("defer Resubmission ", lines[0])
        self.assertIn("defer Vendor.contest_rate ", lines[1])
        fx = r["cut"]["defer_effects"]
        self.assertIs(fx["process"], True)
        self.assertEqual(
            [(e["target"], e["resolution"], e["withdraws"]) for e in fx["entries"]],
            [("Resubmission", "opaque", "process"), ("Vendor.contest_rate", "opaque", "process")],
        )
        self.assertEqual(r["metrics"]["overall"]["work"]["reason"], "not_configured")
        ends = {t["end"]: t for t in r["terminals"]}
        self.assertEqual(ends["posted"]["elapsed"], {"min": 17, "max": 17})

    def test_without_opaque_defer_values_return(self):
        # 2行を外すと取り下げが無くなり、作業量が出る(値の正しさは見ない。取り下げの有無だけ)
        text = (INV / "phase1.relca").read_text()
        for line in self.OPAQUE:
            self.assertEqual(text.count(line), 1, line)
            text = text.replace(line, "")
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "nodefer.relca"
            path.write_text(text)
            code, r, err = run("check", path, "--configuration", "AsIs")
        self.assertNotIn("W-CUT-2", err)
        self.assertIsNone(r["metrics"]["overall"]["work"]["reason"], err)


if __name__ == "__main__":
    unittest.main()
