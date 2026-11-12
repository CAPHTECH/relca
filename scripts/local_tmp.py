"""Select a temporary directory inside the source root, before creating it."""

import os
from pathlib import Path


def local_tmp(root, requested=None):
    root = Path(root).resolve()
    requested = os.environ.get("TMPDIR", ".tmp") if requested is None else requested
    candidate = (root / requested).resolve()
    if not candidate.is_relative_to(root):
        candidate = root / ".tmp"
    # Do not follow an escaping symlink at the fallback either.
    candidate = candidate.resolve()
    if not candidate.is_relative_to(root):
        raise ValueError("temporary directory must remain inside the source root")
    return candidate


if __name__ == "__main__":
    print(local_tmp(Path(__file__).resolve().parent.parent))
