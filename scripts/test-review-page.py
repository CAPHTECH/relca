#!/usr/bin/env python3
"""確認用ページ(skills/relca-model/scripts)の試験を、いま build した relca で回す。

check.sh から呼ぶ。古い binary を使って黙って食い違わないよう、RELCA_BIN を target/debug/relca に固定する。
"""
import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
env = dict(os.environ, RELCA_BIN=str(ROOT / "target" / "debug" / "relca"))
sys.exit(subprocess.run(
    [sys.executable, "-B", "-m", "unittest", "discover",
     "-s", str(ROOT / "skills" / "relca-model" / "scripts"), "-p", "test_review_page.py"],
    env=env, cwd=ROOT).returncode)
