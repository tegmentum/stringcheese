"""Head-to-head: StringCheese (via wasm) OSA + Damerau vs. Python's libraries.

# Variant identity is load-bearing

There are two commonly-named "Damerau" algorithms and they compute
different distances. Pairing them incorrectly puts two different
algorithms on the same axis and produces numbers that look meaningful
but are not — exactly the failure mode ``docs/DESIGN.md`` warns about
in the "Comparative Library Benchmarking" section, and exactly the
pattern the Rust adapter's ``damerau_vs_strsim.rs`` documents.

## OSA (Optimal String Alignment / "restricted Damerau")

Each substring can be edited at most once, does not satisfy the
triangle inequality.

* **StringCheese** — ``osa_distance``.
* **rapidfuzz** — ``rapidfuzz.distance.OSA.distance``.

jellyfish does not expose OSA (only full Damerau); python-Levenshtein
does not expose either Damerau variant. This group is therefore
StringCheese-vs-rapidfuzz only.

## Full unrestricted Damerau

Substrings can be edited unlimited times; a true metric.

* **StringCheese** — **not exposed at the WIT boundary.** See
  ``component/README.md`` "Deliberately not exposed": the underlying
  kernel needs a ``HashMap``, which pulls in ``getrandom`` on
  ``wasm32-*``. The adapter raises :class:`NotImplementedError` from
  ``damerau_distance``; the pytest test below is marked ``skip`` with a
  reason so the run stays honest about what is and is not measured.
* **jellyfish** — ``jellyfish.damerau_levenshtein_distance``.
* **rapidfuzz** — ``rapidfuzz.distance.DamerauLevenshtein.distance``.

The jellyfish + rapidfuzz cells are still benched even without a
StringCheese counterpart so the ecosystem-baseline numbers are on the
same axis as the OSA head-to-head. A StringCheese Damerau cell will
appear once the underlying kernel gets a wasm-portable hash story.

# Representation caveat

Same as the other files: StringCheese consumes ``bytes``, the two
native libraries consume ``str``. On ASCII input the semantics agree
and the FFI cost is folded in on purpose.
"""

from __future__ import annotations

import jellyfish
import pytest
from rapidfuzz.distance import DamerauLevenshtein as RfDam
from rapidfuzz.distance import OSA as RfOsa

from _inputs import LENGTHS, REGIMES, build_pair

_SALTS = (0xE1, 0xE2, 0xE3)


@pytest.fixture(scope="module")
def pairs() -> dict[tuple[int, str], tuple[bytes, bytes]]:
    return {(n, k): build_pair(n, k, _SALTS) for n in LENGTHS for k in REGIMES}


# --------------------------------------------------------------------------- #
# OSA (restricted Damerau)
# --------------------------------------------------------------------------- #


@pytest.mark.parametrize("n", LENGTHS, ids=[f"len{n:04d}" for n in LENGTHS])
@pytest.mark.parametrize("kind", REGIMES)
def test_stringcheese_osa(benchmark, stringcheese, pairs, n, kind):
    benchmark.group = f"osa/{kind}/len{n:04d}"
    a, b = pairs[(n, kind)]
    benchmark(stringcheese.osa_distance, a, b)


@pytest.mark.parametrize("n", LENGTHS, ids=[f"len{n:04d}" for n in LENGTHS])
@pytest.mark.parametrize("kind", REGIMES)
def test_rapidfuzz_osa(benchmark, pairs, n, kind):
    benchmark.group = f"osa/{kind}/len{n:04d}"
    a_b, b_b = pairs[(n, kind)]
    a, b = a_b.decode("ascii"), b_b.decode("ascii")
    benchmark(RfOsa.distance, a, b)


# --------------------------------------------------------------------------- #
# Full Damerau (unrestricted)
# --------------------------------------------------------------------------- #


@pytest.mark.skip(
    reason=(
        "Full Damerau is not exposed by the StringCheese WIT component — "
        "the underlying kernel needs a HashMap (getrandom on wasm32). "
        "See component/README.md 'Deliberately not exposed'. "
        "Remove this skip once the kernel gets a wasm-portable hash story."
    )
)
@pytest.mark.parametrize("n", LENGTHS, ids=[f"len{n:04d}" for n in LENGTHS])
@pytest.mark.parametrize("kind", REGIMES)
def test_stringcheese_damerau(benchmark, stringcheese, pairs, n, kind):
    benchmark.group = f"damerau/{kind}/len{n:04d}"
    a, b = pairs[(n, kind)]
    benchmark(stringcheese.damerau_distance, a, b)


@pytest.mark.parametrize("n", LENGTHS, ids=[f"len{n:04d}" for n in LENGTHS])
@pytest.mark.parametrize("kind", REGIMES)
def test_jellyfish_damerau(benchmark, pairs, n, kind):
    benchmark.group = f"damerau/{kind}/len{n:04d}"
    a_b, b_b = pairs[(n, kind)]
    a, b = a_b.decode("ascii"), b_b.decode("ascii")
    benchmark(jellyfish.damerau_levenshtein_distance, a, b)


@pytest.mark.parametrize("n", LENGTHS, ids=[f"len{n:04d}" for n in LENGTHS])
@pytest.mark.parametrize("kind", REGIMES)
def test_rapidfuzz_damerau(benchmark, pairs, n, kind):
    benchmark.group = f"damerau/{kind}/len{n:04d}"
    a_b, b_b = pairs[(n, kind)]
    a, b = a_b.decode("ascii"), b_b.decode("ascii")
    benchmark(RfDam.distance, a, b)
