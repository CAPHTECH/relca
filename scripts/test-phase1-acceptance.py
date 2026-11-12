#!/usr/bin/env python3
"""Check the closed design acceptance mapping, not semantic implementation coverage."""

import re
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
TABLE = ROOT / "docs/guide/acceptance.md"
REFERENCE = re.compile(r"`(dir|test):([^`]+)`")


def section(text, start, end):
    if text.count(start) != 1 or text.count(end) != 1:
        raise ValueError(f"section markers must occur exactly once: {start}")
    return text.split(start, 1)[1].split(end, 1)[0]


# Independent summaries, keyed by the unchanged row IDs. No source headings or
# prose are shipped. Evidence, status rules, row identity and order still close
# over all 39 obligations, rather than treating any nonempty table as sufficient.
LABELS = {
    "01": [
        "有限型と確率の静的拒否", "逐次の所要時間", "並行の所要時間", "容量による直列化",
        "先着採用後のdrain", "取消要求とackの間隔", "同時刻の採用規律", "writer間の衝突",
        "再送での二重登録", "初期誤りと見逃しの積", "共通障害の同時失敗", "再実行間の依存",
        "終了と回復の区別", "排除後にも残る義務", "実行者IDの分離", "非活性制約の分類",
        "計算切断の保留", "標本と解析値の区別", "欠けたログの不明判定", "比較条件の整合",
    ],
    "02": [
        "grain変更による非干渉", "省略欄の報告", "匿名実行者の検査限界", "Cutなしの警告",
        "Actorの包含漏れ", "partyの経路不足", "異なるCutの比較拒否", "intentと実差分の対応",
        "共通根拠の重複防止", "根拠不足の独立性警告", "チームと個人の重複容量",
        "判定と停止経路の区別", "使用ゼロの縮退操作", "資源近似と台帳機構の対照",
        "真値による分岐の拒否", "許可と授権の別検査", "規則改定前の授権", "異議の構造的表示",
        "請求書モデルの数値受入",
    ],
}


def design_labels(version):
    # When private sources are present, keep the source-table cardinality check.
    # Number-only section integrity is checked separately by check-refs.
    number = {"01": "0.1", "02": "0.2"}[version]
    markers = {"01": ("## 18. ", "## 19. "), "02": ("### 22.1 ", "### 22.2 ")}
    for path in (ROOT / "docs").glob("design*.md"):
        if not re.search(rf"^# .*?v{re.escape(number)}(?:\D|$)", path.read_text(), re.M):
            continue
        text = section(path.read_text(), *markers[version])
        rows = re.findall(r"^\| ([^|]+?) \|", text, re.M)[1:]
        if len(rows) != len(LABELS[version]):
            raise ValueError(f"design V{version}: source acceptance row count differs")
    return LABELS[version]


def is_test(source, name):
    # Explicit Rust tests; a helper or a name mentioned only in a comment is not enough.
    explicit = rf"#\[test\]\s*(?:#\[[^\n]+\]\s*)*(?:pub\s+)?fn\s+{re.escape(name)}\s*\("
    if re.search(explicit, source):
        return True
    # conditional_probability.rs's matrix! generates #[test] fn $name from
    # named rows. Check both the generator and the actual invocation, not a
    # bare substring that could accept a made-up function or fixture label.
    generator = re.search(
        r"macro_rules!\s+matrix\s*\{.*?#\[test\]\s*fn\s*\$name\s*\(.*?\n\}",
        source,
        re.S,
    )
    invocation = re.search(r"^matrix!\s*\{(.*?)^\}", source, re.M | re.S)
    return bool(
        generator
        and invocation
        and re.search(rf"^\s*{re.escape(name)}\s*:", invocation[1], re.M)
    )


def validate(text, *, public=False):
    errors = []
    for version, count in [("01", 20), ("02", 19)]:
        labels = design_labels(version)
        if len(labels) != count:
            errors.append(f"design V{version}: expected {count} rows, got {len(labels)}")
        block = section(
            text,
            f"<!-- phase1-acceptance-v{version}:start -->",
            f"<!-- phase1-acceptance-v{version}:end -->",
        )
        rows = []
        for line in block.splitlines():
            if not line.startswith("| V"):
                continue
            cells = [cell.strip() for cell in line.strip("|").split("|")]
            if len(cells) != 4:
                errors.append(f"malformed mapping row: {line}")
                continue
            row_id, label, status, evidence = cells
            rows.append((row_id, label))
            if status not in {"covered", "later", "partial", "missing"}:
                errors.append(f"{row_id}: unknown status {status}")
            refs = REFERENCE.findall(evidence)
            if status == "covered" and not refs:
                errors.append(f"{row_id}: covered row has no fixture or test")
            if status in {"missing", "partial"} and "未対応" not in evidence and "K8" not in evidence:
                errors.append(f"{row_id}: unresolved part needs an explicit limitation")
            if status == "later":
                if public:
                    if "後続へ分けた" not in evidence:
                        errors.append(f"{row_id}: later row needs an explicit deferral")
                elif not re.search(r"v0\.[12].*Phase2", evidence):
                    errors.append(f"{row_id}: later row needs a versioned Phase2 basis")
            for kind, value in refs:
                if kind == "dir":
                    path = ROOT / value
                    if not value.startswith("fixtures/") or not path.is_dir():
                        errors.append(f"{row_id}: missing fixture directory {value}")
                    else:
                        for filename in ["model.relca", "calc.md", "expected.json"]:
                            if not (path / filename).is_file():
                                errors.append(f"{row_id}: missing {value}/{filename}")
                else:
                    path, separator, name = value.partition("::")
                    file = ROOT / path
                    if (
                        not separator
                        or not path.startswith("crates/")
                        or not path.endswith(".rs")
                        or not file.is_file()
                        or not is_test(file.read_text(), name)
                    ):
                        errors.append(f"{row_id}: missing Rust test {value}")
        expected = [(f"V{version}-{n:02d}", label) for n, label in enumerate(labels, 1)]
        if rows != expected:
            errors.append(f"V{version}: row IDs, order or acceptance summaries differ: {rows!r}")
    return errors


class AcceptanceMapping(unittest.TestCase):
    def test_design_rows_and_every_cited_directory_and_test_exist(self):
        self.maxDiff = None
        self.assertEqual(validate(TABLE.read_text(), public=True), [])

    def test_later_public_rows_require_explicit_deferral(self):
        text = TABLE.read_text()
        rows = [line for line in text.splitlines() if "| later |" in line]
        self.assertEqual(len(rows), 3)
        for row in rows:
            changed = text.replace(row, row.replace("後続へ分けた行", "対象の行"), 1)
            self.assertNotEqual(changed, text)
            self.assertTrue(any("explicit deferral" in e for e in validate(changed, public=True)))

    def test_public_wording_does_not_replace_private_versioned_basis(self):
        errors = validate(TABLE.read_text())
        self.assertEqual(len(errors), 3)
        self.assertTrue(all("versioned Phase2 basis" in e for e in errors))

    def test_missing_or_duplicate_design_row_is_rejected(self):
        text = TABLE.read_text()
        row = next(line for line in text.splitlines() if line.startswith("| V01-02 |"))
        for changed in [text.replace(row, "", 1), text.replace(row, row + "\n" + row, 1)]:
            self.assertTrue(any("row IDs" in error for error in validate(changed, public=True)))

    def test_nonexistent_directory_is_rejected(self):
        changed = TABLE.read_text().replace(
            "dir:fixtures/serial-time`", "dir:fixtures/nonexistent-directory`", 1
        )
        self.assertTrue(any("missing fixture directory" in error for error in validate(changed, public=True)))

    def test_nonexistent_test_and_existing_helper_are_rejected(self):
        original = "::live_quality_time_waits_and_quantiles_follow_hand_fractions`"
        for replacement in ["::nonexistent_test`", "::root`"]:
            changed = TABLE.read_text().replace(original, replacement, 1)
            self.assertTrue(any("missing Rust test" in error for error in validate(changed, public=True)))

    def test_matrix_requires_the_real_generator_and_named_invocation(self):
        source = (ROOT / "crates/relca-core/tests/conditional_probability.rs").read_text()
        self.assertTrue(is_test(source, "s_mass_high"))
        self.assertFalse(is_test(source, "SUM"))
        self.assertFalse(is_test(source.replace("#[test] fn $name", "fn $name"), "s_mass_high"))


if __name__ == "__main__":
    unittest.main()
