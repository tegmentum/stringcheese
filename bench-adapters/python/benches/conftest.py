"""pytest fixtures shared across the Python bench-adapter files.

The two things every bench file needs are:

* A single, module-scoped :class:`StringCheese` — instantiating the
  wasm component takes ~10 ms of setup (compile + link + instantiate),
  which is not what the bench is measuring. One instance per pytest
  process keeps the setup cost off the measurement.
* Prebuilt input pairs, cached by ``(length, regime)`` so the corpus is
  materialised once and re-used across pytest-benchmark rounds.

These fixtures are pytest-only; direct use of ``StringCheese`` from a
regular Python script does not need them.
"""

from __future__ import annotations

import pytest

import sys
from pathlib import Path

# Make the sibling ``stringcheese_adapter`` module importable without an
# editable install. The adapter subtree ships without a src/-layout
# because the whole thing is one file at ``bench-adapters/python/``.
_HERE = Path(__file__).resolve().parent
_PKG_ROOT = _HERE.parent
if str(_PKG_ROOT) not in sys.path:
    sys.path.insert(0, str(_PKG_ROOT))

from stringcheese_adapter import StringCheese  # noqa: E402


@pytest.fixture(scope="session")
def stringcheese() -> StringCheese:
    """One :class:`StringCheese` per pytest session.

    Session scope, not function scope: the component load / link /
    instantiate takes tens of milliseconds and would dwarf every
    per-call bench measurement if paid per test.
    """
    return StringCheese()
