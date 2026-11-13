#!/usr/bin/env bash
# 最初に失敗した検査で停止し、同じソースの合格を記録する。
set -euo pipefail
cd "$(dirname "$0")/.."
TMPDIR=$(python3 -B scripts/local_tmp.py)
export TMPDIR
mkdir -p "$TMPDIR"

stamp=target/.check-ok
current=$(python3 -B scripts/source_files.py)
if [[ "${1-}" != "--force" && -f $stamp && "$(cat "$stamp")" == "$current" ]]; then
  echo "check: 前回合格と同じ状態のため省略 (--force で再実行)"
  exit 0
fi
python3 -B -c 'from pathlib import Path; Path("target/.check-ok").unlink(missing_ok=True)'
step() { echo "== $*"; "$@"; }
step cargo fmt --all --check
step cargo clippy --workspace --all-targets --quiet -- -D warnings
if command -v cargo-nextest >/dev/null 2>&1; then
  echo "check: test runner = nextest + cargo test --doc"
  step cargo nextest run --workspace
  step cargo test --doc --workspace --quiet
else
  echo "check: test runner = cargo test (cargo-nextest not found)"
  step cargo test --workspace --quiet
fi
step python3 -B scripts/check-refs.py
step python3 -B scripts/test-phase1-acceptance.py
# 確認用ページの試験は、いま build した relca を使う(古い binary で黙って食い違わない)。
step cargo build --quiet -p relca-cli
step python3 -B scripts/test-review-page.py
mkdir -p target
echo "$current" > "$stamp"
echo "check: 全段合格"
