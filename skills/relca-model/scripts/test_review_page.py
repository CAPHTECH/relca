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
import json
import os
import re
import subprocess
import sys
import tempfile
import unittest
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


@unittest.skipUnless(RELCA.exists(), "relca の binary が無い: %s(cargo build -p relca-cli)" % RELCA)
class ReviewPageTest(unittest.TestCase):
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
        self.assertIn(["compare 案 ToBeA", "verdict の畳み込み", "violated"], unv)
        self.assertIn(["compare 案 AsIs", "safety no_resource_overcommit", "inconclusive"], unv)
        self.assertIn(["compare 案 AsIs", "bounds", "inconclusive"], unv)
        self.assertTrue(any(a[1].startswith("compare 案 ToBeA") and "より軽い" in a[1] for a in data["alerts"]))

    def test_compare_top_verdict_lighter_than_plans(self):
        src, _, _ = self.r["compare-exhaustive"]
        bad = rewrite(src, Path(self.tmp) / "cmp-top.json", lambda d: d.__setitem__("verdict", "satisfied"))
        cout, _, ccode = self.r["as-is-check"]
        _, data = page(self.tmp, "cmp-top", "--model", INV / "as-is.relca", "--check", cout, "--check-exit", ccode,
                       "--compare", bad, "--compare-exit", 0)
        self.assertIn(["compare", "verdict の畳み込み", "violated"], [u[:3] for u in data["unverified"]])

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
        self.assertEqual(json.loads(Path(out).read_text())["cut"], "undeclared")  # relca 自体は undeclared と出す
        _, data = page(self.tmp, "comment", "--model", broken, "--check", out, "--check-exit", code)
        cut = [u for u in data["unverified"] if u[1] == "Cut"]
        self.assertEqual(len(cut), 1)
        self.assertIn("parse に失敗", cut[0][3])
        self.assertEqual(cut[0][2], "unsupported")  # U-SYN-0
        self.assertFalse(any("Cut が宣言されていない" in a[1] for a in data["alerts"]))

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

    # ---------------------------------------------------------- N5: cases の畳み込み

    def test_satisfied_property_with_violated_case_is_contradiction(self):
        src, _, code = self.r["as-is-check"]

        def f(d):
            d["properties"][0]["cases"] = [{"case_id": 1, "verdict": "satisfied"}, {"case_id": 2, "verdict": "violated"}]
        bad = rewrite(src, Path(self.tmp) / "cases.json", f)
        _, data = page(self.tmp, "cases", "--model", INV / "as-is.relca", "--check", bad, "--check-exit", code)
        self.assertTrue(any("畳み込みが矛盾" in a[1] for a in data["alerts"]))
        self.assertIn(["check", "safety no_resource_overcommit の畳み込み", "violated"],
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


if __name__ == "__main__":
    unittest.main(verbosity=2)
