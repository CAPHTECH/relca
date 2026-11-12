#!/usr/bin/env bash
set -euo pipefail
root=$(dirname "$(dirname "$(realpath "$0")")")
export CARGO_NET_OFFLINE=true
TMPDIR=$(python3 -B "$root/scripts/local_tmp.py")
export TMPDIR
mkdir -p "$TMPDIR"
cargo build --manifest-path "$root/Cargo.toml" -p relca-cli
exec python3 -B "$root/scripts/snapshots.py" --binary "$root/target/debug/relca" --update
