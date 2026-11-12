#!/usr/bin/env python3
"""設計書の節参照(§N)が実在する見出しを指すか検査する。

規則:
- docs/design*.md の中: 無印の §N は自文書、`v0.1 §N` はその版。外部文献は見ない。
- それ以外のファイル: 版の明記(`v0.2 §21`)を必須にする。無印は誤り。
  設計書の版が上がると旧版の参照は全部落ちるので、番号ずれを黙って残さない。
- `§a〜§b`、`§a・§b`、`§a、§b` の後ろ側は前の参照と同じ文書を指す。

見るのは見出しの実在だけで、参照先の内容が文脈に合うかは見ない。
"""

import re
import sys
from pathlib import Path

from source_files import source_files

ROOT = Path(__file__).resolve().parent.parent
DOCS = sorted((ROOT / "docs").glob("design*.md"))
INDEX = ROOT / "docs/design-sections.txt"

TITLE = re.compile(r"^# .*?v(\d+\.\d+)", re.M)
HEADING = re.compile(r"^#{2,4} (\d+(?:\.\d+)*)[.\s]", re.M)
REF = re.compile(r"§(\d+(?:\.\d+)*)")
VERSION_BEFORE = re.compile(r"v(\d+\.\d+)\s*$")
EXTERNAL_BEFORE = re.compile(r"`[^`]+`\s*$")
CHAIN_BEFORE = re.compile(r"[〜~・、]\s*$")


def load_docs():
    """Use number-only metadata; if private sources exist, verify exact agreement."""
    by_version, sections = {}, {}
    for lineno, line in enumerate(INDEX.read_text(encoding="utf-8").splitlines(), 1):
        match = re.fullmatch(r"v(\d+\.\d+) §(\d+(?:\.\d+)*)", line)
        if not match:
            raise ValueError(f"{INDEX.name}:{lineno}: 版つき節番号以外の内容")
        version, number = match.groups()
        target = by_version.setdefault(version, version)
        numbers = sections.setdefault(target, set())
        if number in numbers:
            raise ValueError(f"{INDEX.name}:{lineno}: 重複した節番号")
        numbers.add(number)
    if not by_version:
        raise ValueError("節番号索引が空")
    for path in DOCS:
        text = path.read_text(encoding="utf-8")
        m = TITLE.search(text)
        if not m:
            raise ValueError(f"{path.relative_to(ROOT)}: 表題に版 (vX.Y) が無い")
        version = m.group(1)
        actual = set(HEADING.findall(text))
        indexed = sections.get(version, set())
        if actual != indexed:
            raise ValueError(
                f"v{version}: 索引と設計の節が不一致 "
                f"(索引に不足={sorted(actual - indexed)}, 索引に余分={sorted(indexed - actual)})"
            )
        sections[path] = actual
    return by_version, sections


def tracked_files():
    return [p for p in source_files(ROOT) if p.suffix in {".md", ".rs", ".toml", ".py", ".sh", ".relca", ".json"}]


def main():
    try:
        by_version, sections = load_docs()
    except (ValueError, OSError) as error:
        print(f"check-refs: {error}", file=sys.stderr)
        return 1
    errors = []
    for path in tracked_files():
        if not path.is_file():
            continue
        rel = path.relative_to(ROOT)
        in_design = path in sections
        for lineno, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            target = None  # 直前の参照先(連鎖用)。"external" は外部
            for m in REF.finditer(line):
                before = line[: m.start()]
                if CHAIN_BEFORE.search(before) and target is not None:
                    pass
                elif EXTERNAL_BEFORE.search(before):
                    target = "external"
                elif v := VERSION_BEFORE.search(before):
                    target = by_version.get(v.group(1))
                    if target is None:
                        errors.append(f"{rel}:{lineno}: v{v.group(1)} の版が索引に無い: {m.group(0)}")
                        target = "external"
                        continue
                elif in_design:
                    target = path
                else:
                    errors.append(f"{rel}:{lineno}: 版の無い節参照 {m.group(0)}(版を明記する: vX.Y {m.group(0)})")
                    target = "external"
                    continue
                if target != "external" and m.group(1) not in sections[target]:
                    errors.append(f"{rel}:{lineno}: {target} に {m.group(0)} が無い")
    for e in errors:
        print(e, file=sys.stderr)
    if errors:
        print(f"check-refs: {len(errors)} 件の不正な節参照", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
