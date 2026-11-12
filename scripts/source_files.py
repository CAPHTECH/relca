"""Source inventory for a checkout or a source-only export (no parent-repo fallback)."""

import hashlib
import os
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
GENERATED = {".git", ".tmp", "target", "__pycache__"}


def source_files(root=ROOT):
    if (root / ".git").exists():
        result = subprocess.run(
            ["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"],
            cwd=root, check=True, capture_output=True,
        )
        return sorted({root / os.fsdecode(p) for p in result.stdout.split(b"\0") if p})
    paths = []
    for parent, dirs, files in os.walk(root):
        dirs[:] = sorted(d for d in dirs if d not in GENERATED)
        paths.extend(Path(parent) / name for name in sorted(files))
    return paths


def fingerprint(root=ROOT):
    digest = hashlib.sha256()
    for path in source_files(root):
        digest.update(os.fsencode(path.relative_to(root)))
        digest.update(b"\0")
        if path.is_file():
            digest.update(path.read_bytes())
            digest.update(str(path.stat().st_mode & 0o777).encode())
        else:
            digest.update(b"missing")
        digest.update(b"\0")
    return digest.hexdigest()


if __name__ == "__main__":
    print(fingerprint())
