#!/usr/bin/env python3
"""Regenerate the ``unigram-byte-fallback-synth`` conformance fixture.

The fixture is a hand-crafted SentencePiece Unigram vocabulary with
``byte_fallback: true`` and 20 test cases specifically designed to
exercise the Unigram byte-fallback code path in ``UnigramTokenizer``.

This script writes two files, in place:

* ``unigram_byte_fallback_synth.json`` — the fixture the conformance
  runner reads (``checkpoint`` / ``source`` / ``note`` / ``cases``).
* ``vocabs/unigram-byte-fallback-synth/tokenizer.json`` — the
  synthetic Unigram vocab the runner materialises into a runtime
  tokenizer.

The expected-id column in each case is computed by an in-file Viterbi
reimplementation that mirrors ``UnigramTokenizer::encode_piece_ids``
byte for byte, so if the runtime and this script agree the fixture
round-trips; if they diverge, the conformance runner surfaces the
mismatch and the fix goes into whichever side is wrong.

Run from anywhere::

    python3 crates/stringcheese-tokenizer-hf/tests/conformance/gen_unigram_byte_fallback_synth.py
"""

import json
import os
from pathlib import Path

HERE = Path(__file__).resolve().parent

# Vocab shape mirrors the Llama-2 convention: <unk>/<s>/</s> at ids
# 0..2, the 256 reserved byte-fallback tokens at ids 3..=258, then a
# few word tokens for tests that need to hit vocab-only paths as well.
vocab = [["<unk>", 0.0], ["<s>", 0.0], ["</s>", 0.0]]
for b in range(256):
    vocab.append([f"<0x{b:02X}>", -3.0])
BYTE_BASE = 3
words = ["hi", "hello", "world", "the", "quick", "fox"]
for w in words:
    vocab.append([w, -1.0])

tokenizer_json = {
    "version": "1.0",
    "truncation": None,
    "padding": None,
    "added_tokens": [],
    "normalizer": None,
    "pre_tokenizer": None,
    "post_processor": None,
    "decoder": None,
    "model": {
        "type": "Unigram",
        "vocab": vocab,
        "unk_id": 0,
        "byte_fallback": True,
    },
}


def encode(text: str) -> list[int]:
    """Same Viterbi as ``UnigramTokenizer::encode_piece_ids``.

    A separate implementation kept small enough that a divergence is
    obvious on inspection. Byte-fallback fires when a position is
    unreachable via vocab-only paths, scored with the same
    ``unk_penalty`` (10.0) the Rust runtime uses so vocab-only paths
    always win when they exist.
    """
    if not text:
        return []
    chars = list(text)
    n = len(chars)
    NEG = float("-inf")
    best_score = [NEG] * (n + 1)
    best_prev: list = [(0, None)] * (n + 1)
    best_score[0] = 0.0
    UNK_PENALTY = 10.0
    lookup: dict[str, tuple[int, float]] = {}
    for i, (s, sc) in enumerate(vocab):
        lookup.setdefault(s, (i, sc))
    for i in range(1, n + 1):
        for j in range(0, i):
            if best_score[j] == NEG:
                continue
            piece = "".join(chars[j:i])
            entry = lookup.get(piece)
            if entry is not None:
                cand = best_score[j] + entry[1]
                if cand > best_score[i]:
                    best_score[i] = cand
                    best_prev[i] = (j, ("single", entry[0]))
        if best_score[i] == NEG and best_score[i - 1] != NEG:
            bs = chars[i - 1].encode("utf-8")
            byte_ids = [BYTE_BASE + b for b in bs]
            best_score[i] = best_score[i - 1] - UNK_PENALTY
            best_prev[i] = (i - 1, ("bytes", byte_ids))
    if best_score[n] == NEG:
        raise ValueError(f"cannot encode {text!r}")
    ids_rev: list[int] = []
    pos = n
    while pos > 0:
        prev, trans = best_prev[pos]
        kind, payload = trans
        if kind == "single":
            ids_rev.append(payload)
        else:
            ids_rev.extend(reversed(payload))
        pos = prev
    return list(reversed(ids_rev))


inputs = [
    ("empty", ""),
    ("single-word-hi", "hi"),
    ("single-word-hello", "hello"),
    ("two-words-nospace", "hihello"),
    ("ascii-oov-question", "?"),
    ("ascii-oov-mixed", "hi?"),
    ("ascii-oov-only", "??!"),
    ("word-plus-2byte-utf8", "hié"),
    ("2byte-utf8-only", "é"),
    ("3byte-utf8-snowman", "☃"),
    ("4byte-utf8-emoji-smile", "\U0001f600"),
    ("word-plus-4byte-emoji", "hello\U0001f600"),
    ("mixed-word-oov-word", "hi?world"),
    ("repeated-emoji", "\U0001f600\U0001f600\U0001f600"),
    ("cyrillic-2byte", "др"),
    ("chinese-3byte", "你好"),
    ("hindi-3byte", "नमस"),
    ("arabic-2byte", "مر"),
    ("obscure-unicode-hebrew-2byte", "שלום"),
    ("emoji-plus-word-plus-emoji", "\U0001f600hi\U0001f389"),
]
cases = [
    {"input": inp, "expected_ids": encode(inp), "note": note}
    for note, inp in inputs
]

fixture = {
    "checkpoint": "unigram-byte-fallback-synth",
    "source": (
        "hand-crafted synthetic Unigram vocab; expected ids computed by "
        "gen_unigram_byte_fallback_synth.py against the vocab in "
        "tests/conformance/vocabs/unigram-byte-fallback-synth/tokenizer.json"
    ),
    "note": (
        "Synthetic Llama-shape Unigram vocabulary with SentencePiece "
        "byte_fallback enabled; exercises the <0xXX> byte-fallback path "
        "for a mix of ASCII / Latin / CJK / emoji OOV characters. The "
        "vocab is hand-crafted (no upstream reference tool involved) - "
        "its whole purpose is to exercise the byte-fallback code path "
        "in the conformance runner. IDs 3..=258 are the reserved "
        "<0x00>..<0xFF> byte-fallback tokens; ids 259.. are the six "
        "seed word tokens (hi, hello, world, the, quick, fox)."
    ),
    "cases": cases,
}

(HERE / "unigram_byte_fallback_synth.json").write_text(
    json.dumps(fixture, indent=2, ensure_ascii=False) + "\n",
    encoding="utf-8",
)
vocab_dir = HERE / "vocabs" / "unigram-byte-fallback-synth"
vocab_dir.mkdir(parents=True, exist_ok=True)
(vocab_dir / "tokenizer.json").write_text(
    json.dumps(tokenizer_json, indent=2, ensure_ascii=False) + "\n",
    encoding="utf-8",
)
print(f"wrote {HERE / 'unigram_byte_fallback_synth.json'}")
print(f"wrote {vocab_dir / 'tokenizer.json'}")
print(f"cases: {len(cases)}, vocab size: {len(vocab)}")
