"""Fail-closed checks which remain active under python -O / PYTHONOPTIMIZE."""


def require(condition, detail: object='K5 explicit check failed'):
    if not condition:
        raise AssertionError(detail)
