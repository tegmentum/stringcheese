"""Head-to-head: StringCheese (via wasm) vs. Python's Jaro / Jaro-Winkler libs.

# Contestants

Jaro:

* **StringCheese** — ``jaro_similarity`` from the wasm component.
* **python-Levenshtein.jaro** — C extension.
* **jellyfish.jaro_similarity** — Rust extension.

Jaro–Winkler:

* **StringCheese** — ``jaro_winkler_similarity`` (classic: prefix 4,
  scaling 0.1, no boost threshold).
* **python-Levenshtein.jaro_winkler** — same defaults as the classic
  Winkler paper.
* **jellyfish.jaro_winkler_similarity** — same defaults.

rapidfuzz **is not** included in this file. ``rapidfuzz.distance.Jaro``
and ``rapidfuzz.distance.JaroWinkler`` exist and would slot in cleanly;
the ecosystem-standard-pair for Jaro-family similarity in Python is
python-Levenshtein + jellyfish, and adding rapidfuzz is a followup.

# Variant identity

All three libraries here compute **classic** Jaro–Winkler with prefix 4
and scaling 0.1. StringCheese's ``JaroWinkler::classic()`` matches those
defaults; ``docs/design/`` and ``component/README.md`` are clear that
only the classic variant is exposed at the WIT boundary. If a future
non-classic tuning goes into the WIT interface, that pairing needs a
separate bench file to keep the head-to-head axis clean.
"""

from __future__ import annotations

import Levenshtein
import jellyfish
import pytest

from _inputs import LENGTHS, REGIMES, build_pair

_SALTS = (0xD1, 0xD2, 0xD3)


@pytest.fixture(scope="module")
def pairs() -> dict[tuple[int, str], tuple[bytes, bytes]]:
    return {(n, k): build_pair(n, k, _SALTS) for n in LENGTHS for k in REGIMES}


# --------------------------------------------------------------------------- #
# Jaro
# --------------------------------------------------------------------------- #


@pytest.mark.parametrize("n", LENGTHS, ids=[f"len{n:04d}" for n in LENGTHS])
@pytest.mark.parametrize("kind", REGIMES)
def test_stringcheese_jaro(benchmark, stringcheese, pairs, n, kind):
    benchmark.group = f"jaro/{kind}/len{n:04d}"
    a, b = pairs[(n, kind)]
    benchmark(stringcheese.jaro_similarity, a, b)


@pytest.mark.parametrize("n", LENGTHS, ids=[f"len{n:04d}" for n in LENGTHS])
@pytest.mark.parametrize("kind", REGIMES)
def test_python_levenshtein_jaro(benchmark, pairs, n, kind):
    benchmark.group = f"jaro/{kind}/len{n:04d}"
    a_b, b_b = pairs[(n, kind)]
    a, b = a_b.decode("ascii"), b_b.decode("ascii")
    benchmark(Levenshtein.jaro, a, b)


@pytest.mark.parametrize("n", LENGTHS, ids=[f"len{n:04d}" for n in LENGTHS])
@pytest.mark.parametrize("kind", REGIMES)
def test_jellyfish_jaro(benchmark, pairs, n, kind):
    benchmark.group = f"jaro/{kind}/len{n:04d}"
    a_b, b_b = pairs[(n, kind)]
    a, b = a_b.decode("ascii"), b_b.decode("ascii")
    benchmark(jellyfish.jaro_similarity, a, b)


# --------------------------------------------------------------------------- #
# Jaro–Winkler
# --------------------------------------------------------------------------- #


@pytest.mark.parametrize("n", LENGTHS, ids=[f"len{n:04d}" for n in LENGTHS])
@pytest.mark.parametrize("kind", REGIMES)
def test_stringcheese_jaro_winkler(benchmark, stringcheese, pairs, n, kind):
    benchmark.group = f"jaro_winkler/{kind}/len{n:04d}"
    a, b = pairs[(n, kind)]
    benchmark(stringcheese.jaro_winkler_similarity, a, b)


@pytest.mark.parametrize("n", LENGTHS, ids=[f"len{n:04d}" for n in LENGTHS])
@pytest.mark.parametrize("kind", REGIMES)
def test_python_levenshtein_jaro_winkler(benchmark, pairs, n, kind):
    benchmark.group = f"jaro_winkler/{kind}/len{n:04d}"
    a_b, b_b = pairs[(n, kind)]
    a, b = a_b.decode("ascii"), b_b.decode("ascii")
    benchmark(Levenshtein.jaro_winkler, a, b)


@pytest.mark.parametrize("n", LENGTHS, ids=[f"len{n:04d}" for n in LENGTHS])
@pytest.mark.parametrize("kind", REGIMES)
def test_jellyfish_jaro_winkler(benchmark, pairs, n, kind):
    benchmark.group = f"jaro_winkler/{kind}/len{n:04d}"
    a_b, b_b = pairs[(n, kind)]
    a, b = a_b.decode("ascii"), b_b.decode("ascii")
    benchmark(jellyfish.jaro_winkler_similarity, a, b)
