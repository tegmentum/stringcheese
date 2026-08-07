"""Pythonic adapter around the StringCheese WebAssembly component.

This module is the Python-side face of ``component/rust-host/`` — it
loads ``stringcheese_component_host.wasm`` via ``wasmtime-py`` and
exposes every function the WIT interface declares as a plain Python
method.

Design notes
============

The StringCheese component is built with ``cargo component build
--release`` from ``component/rust-host/``. Because the crate targets
``wasm32-wasip1``, its produced binary imports a small tail of WASI 0.2
interfaces (stdio, filesystem, clocks) even though the algorithm code
never touches any of them; ``wasmtime.component.Linker.add_wasip2()``
supplies those imports at instantiation time.

Every exported WIT function is looked up **once** at ``StringCheese``
construction and cached on the instance as a bound ``wasmtime.Func``.
Callers then invoke the cached ``Func`` per bench iteration — the
per-call cost is what the benchmarks are meant to measure, not the
per-call *lookup* cost.

WIT boundary shapes handled here
--------------------------------

* ``list<u8> × list<u8> → u32`` — every distance metric except Hamming.
* ``list<u8> × list<u8> → f64`` — every similarity metric.
* ``list<u8> × list<u8> → result<u32, string>`` — Hamming. wasmtime-py
  unwraps ``result<T, E>`` to the raw ``T`` on ``ok`` and the raw ``E``
  on ``err`` with no tag, so we ``isinstance``-check to distinguish and
  raise a Python exception on error.
* ``list<u8> × list<u8> × u32 → variant { within(u32), exceeded(u32) }``
  — bounded Levenshtein. wasmtime-py returns a ``Variant`` with ``tag``
  and ``payload`` attributes; we translate to a small dataclass.
* ``list<u8> × list<u8> → option<u32>`` and ``list<u32>`` — search.
  wasmtime-py returns ``None`` for ``none``, ``int`` for ``some``, and
  a plain ``list[int]`` for ``list<u32>``.
* ``string → string`` — phonetic. Plain ``str`` on both sides.

The WIT ``full Damerau`` function is deliberately absent from the
component (see ``component/README.md`` — the underlying kernel needs a
``HashMap`` which pulls in ``getrandom`` on ``wasm32-*``); the adapter
therefore raises :class:`NotImplementedError` from ``damerau_distance``
so callers can distinguish "not exposed at the WIT boundary" from "not
implemented in StringCheese", and so bench files can catch it and skip.
"""

from __future__ import annotations

import os
from dataclasses import dataclass
from pathlib import Path
from typing import Optional

from wasmtime import Engine, Store
from wasmtime.component import Component, Linker


# --------------------------------------------------------------------------- #
# Types
# --------------------------------------------------------------------------- #


@dataclass(frozen=True)
class BoundedDistance:
    """Result of a bounded distance call.

    Mirrors the WIT ``variant bounded-distance { within(u32), exceeded(u32) }``.
    ``within`` carries the exact distance (guaranteed ``<= cutoff``);
    ``exceeded`` carries the cutoff itself and signals the true distance
    is strictly greater.
    """

    kind: str  # "within" or "exceeded"
    value: int

    @property
    def is_within(self) -> bool:
        return self.kind == "within"


class HammingLengthMismatch(ValueError):
    """Raised when Hamming distance is asked for on unequal-length inputs.

    The WIT boundary returns ``result<u32, string>`` for Hamming; the
    underlying Rust kernel's typed ``LengthMismatch`` error is flattened
    to a diagnostic string. This exception preserves that diagnostic
    unchanged for callers that want to log or match on it.
    """


# --------------------------------------------------------------------------- #
# Component discovery
# --------------------------------------------------------------------------- #


# Path from this file to the pre-built component .wasm produced by
# ``cargo component build --release`` inside ``component/rust-host/``.
# Kept as a constant so a caller with an out-of-tree build can pass an
# explicit ``wasm_path`` to :class:`StringCheese` and skip discovery.
_DEFAULT_WASM_RELATIVE = Path(
    "../../component/rust-host/target/wasm32-wasip1/release/"
    "stringcheese_component_host.wasm"
)

_WIT_PACKAGE = "stringcheese:core"
_WIT_VERSION = "0.1.0"


def _default_wasm_path() -> Path:
    """Best-effort discovery of the component .wasm on disk.

    Resolution order:

    1. ``STRINGCHEESE_WASM`` env var, if set.
    2. Path relative to this module, matching the layout the repository
       ships (``bench-adapters/python/`` → ``component/rust-host/…``).

    Discovery does **not** trigger a build; the caller is expected to have
    already run ``cargo component build --release`` inside
    ``component/rust-host/``. If the file is missing the caller will see
    a :class:`FileNotFoundError` when :class:`StringCheese` is constructed.
    """
    env = os.environ.get("STRINGCHEESE_WASM")
    if env:
        return Path(env).resolve()
    here = Path(__file__).resolve().parent
    return (here / _DEFAULT_WASM_RELATIVE).resolve()


# --------------------------------------------------------------------------- #
# The adapter
# --------------------------------------------------------------------------- #


def _iface(name: str) -> str:
    """WIT interface name with version suffix, as wasmtime expects it."""
    return f"{_WIT_PACKAGE}/{name}@{_WIT_VERSION}"


class StringCheese:
    """A loaded StringCheese component instance.

    A single :class:`StringCheese` owns one ``wasmtime.Store`` and one
    ``wasmtime.component.Instance``. Instances are **not** thread-safe —
    wasmtime stores are single-threaded by design. Construct one per
    thread if you need concurrent calls; for the bench harness a single
    instance shared across a pytest process is fine because
    pytest-benchmark runs iterations serially.
    """

    def __init__(self, wasm_path: Optional[str] = None) -> None:
        path = Path(wasm_path) if wasm_path is not None else _default_wasm_path()
        if not path.exists():
            raise FileNotFoundError(
                f"StringCheese component .wasm not found at {path}. "
                "Build it first with `cd component/rust-host && "
                "cargo component build --release`, or set the "
                "STRINGCHEESE_WASM env var to an explicit path."
            )

        self._engine = Engine()
        self._store = Store(self._engine)
        with open(path, "rb") as f:
            self._component = Component(self._engine, f.read())
        self._linker = Linker(self._engine)
        # The component imports a WASI 0.2 tail (stdio, clocks,
        # filesystem) because ``wasm32-wasip1`` builds `std` in. These
        # calls are never reached from the algorithm code, but the
        # linker must still satisfy the imports at instantiation time.
        self._linker.add_wasip2()
        self._instance = self._linker.instantiate(self._store, self._component)

        # Pre-resolve every exported function into a cached ``Func``.
        # Doing the lookup once outside the bench loop is the whole
        # point of the adapter — a lookup-per-call would swamp the
        # per-call algorithm cost we are trying to measure.
        self._levenshtein = self._resolve("distance", "levenshtein")
        self._levenshtein_within = self._resolve("distance", "levenshtein-within")
        self._hamming = self._resolve("distance", "hamming")
        self._osa = self._resolve("distance", "osa")
        self._lcs_distance = self._resolve("distance", "lcs-distance")

        self._jaro = self._resolve("similarity", "jaro")
        self._jaro_winkler = self._resolve("similarity", "jaro-winkler")
        self._dice_bigrams = self._resolve("similarity", "dice-bigrams")
        self._jaccard_bigrams = self._resolve("similarity", "jaccard-bigrams")

        self._find_first = self._resolve("search", "find-first")
        self._find_all = self._resolve("search", "find-all")

        self._soundex = self._resolve("phonetic", "soundex")
        self._nysiis = self._resolve("phonetic", "nysiis")
        self._double_metaphone_primary = self._resolve(
            "phonetic", "double-metaphone-primary"
        )

    # ------------------------------------------------------------------ #
    # Lookup helper
    # ------------------------------------------------------------------ #

    def _resolve(self, interface: str, func: str):
        """Look up a nested WIT export and return the callable ``Func``."""
        iface_idx = self._component.get_export_index(_iface(interface))
        if iface_idx is None:
            raise RuntimeError(
                f"WIT interface `{_iface(interface)}` not present in component"
            )
        func_idx = self._component.get_export_index(func, iface_idx)
        if func_idx is None:
            raise RuntimeError(
                f"WIT function `{func}` not present under `{_iface(interface)}`"
            )
        f = self._instance.get_func(self._store, func_idx)
        if f is None:
            raise RuntimeError(
                f"WIT export `{interface}/{func}` resolved to a non-function item"
            )
        return f

    def _invoke(self, func, *args):
        """Call a cached component ``Func`` and clear its post-return state.

        ``post_return`` is the Component Model's hook for the guest to
        clean up any return-side allocations (e.g. the linear-memory
        buffer backing a returned ``list<u8>``). It must be called
        exactly once between successive invocations of the same
        function, or the guest may hold onto a stale pointer.
        """
        result = func(self._store, *args)
        func.post_return(self._store)
        return result

    # ------------------------------------------------------------------ #
    # Distance
    # ------------------------------------------------------------------ #

    def levenshtein_distance(self, a: bytes, b: bytes) -> int:
        """Unit-cost Levenshtein edit distance (byte-level)."""
        return int(self._invoke(self._levenshtein, a, b))

    def levenshtein_within(self, a: bytes, b: bytes, cutoff: int) -> BoundedDistance:
        """Bounded Levenshtein — early-terminates when distance exceeds ``cutoff``."""
        v = self._invoke(self._levenshtein_within, a, b, cutoff)
        # wasmtime-py returns a Variant with (tag, payload).
        return BoundedDistance(kind=v.tag, value=int(v.payload))

    def hamming_distance(self, a: bytes, b: bytes) -> int:
        """Hamming distance. Raises :class:`HammingLengthMismatch` on unequal lengths."""
        r = self._invoke(self._hamming, a, b)
        # wasmtime-py returns the ok payload on success and the err
        # payload on failure with no wrapper. Ok is u32, err is string.
        if isinstance(r, str):
            raise HammingLengthMismatch(r)
        return int(r)

    def osa_distance(self, a: bytes, b: bytes) -> int:
        """Optimal String Alignment / restricted Damerau distance."""
        return int(self._invoke(self._osa, a, b))

    def lcs_distance(self, a: bytes, b: bytes) -> int:
        """LCS-derived distance: ``|a| + |b| - 2 * lcs(a, b)``."""
        return int(self._invoke(self._lcs_distance, a, b))

    def damerau_distance(self, a: bytes, b: bytes) -> int:
        """Full unrestricted Damerau distance.

        **Not exposed at the WIT boundary** — see ``component/README.md``
        "Deliberately not exposed". The underlying Rust kernel needs a
        ``HashMap``, which pulls in ``getrandom`` on wasm32 targets and
        would require additional host wiring. Raises
        :class:`NotImplementedError` unconditionally so bench files can
        catch it and mark the StringCheese cell as N/A.
        """
        raise NotImplementedError(
            "Full Damerau is not exposed by the StringCheese WIT component; "
            "use `osa_distance` (restricted Damerau) instead."
        )

    # ------------------------------------------------------------------ #
    # Similarity
    # ------------------------------------------------------------------ #

    def jaro_similarity(self, a: bytes, b: bytes) -> float:
        """Jaro similarity in ``[0.0, 1.0]``."""
        return float(self._invoke(self._jaro, a, b))

    def jaro_winkler_similarity(self, a: bytes, b: bytes) -> float:
        """Jaro–Winkler (classic: prefix 4, scaling 0.1) in ``[0.0, 1.0]``."""
        return float(self._invoke(self._jaro_winkler, a, b))

    def dice_bigrams(self, a: bytes, b: bytes) -> float:
        """Dice / Sørensen coefficient over character bigrams."""
        return float(self._invoke(self._dice_bigrams, a, b))

    def jaccard_bigrams(self, a: bytes, b: bytes) -> float:
        """Jaccard similarity over character bigrams."""
        return float(self._invoke(self._jaccard_bigrams, a, b))

    # ------------------------------------------------------------------ #
    # Search
    # ------------------------------------------------------------------ #

    def find_first(self, needle: bytes, haystack: bytes) -> Optional[int]:
        """First occurrence of ``needle`` in ``haystack``, or ``None`` if absent."""
        r = self._invoke(self._find_first, needle, haystack)
        return None if r is None else int(r)

    def find_all(self, needle: bytes, haystack: bytes) -> list[int]:
        """Every occurrence of ``needle`` in ``haystack`` (ascending, may overlap)."""
        return [int(x) for x in self._invoke(self._find_all, needle, haystack)]

    # ------------------------------------------------------------------ #
    # Phonetic
    # ------------------------------------------------------------------ #

    def soundex(self, name: str) -> str:
        """NARA-1918 canonical Soundex encoding."""
        return str(self._invoke(self._soundex, name))

    def nysiis(self, name: str) -> str:
        """NYSIIS encoding, truncated to six characters."""
        return str(self._invoke(self._nysiis, name))

    def double_metaphone_primary(self, name: str) -> str:
        """Double Metaphone primary key (Philips 1999)."""
        return str(self._invoke(self._double_metaphone_primary, name))


__all__ = [
    "BoundedDistance",
    "HammingLengthMismatch",
    "StringCheese",
]
