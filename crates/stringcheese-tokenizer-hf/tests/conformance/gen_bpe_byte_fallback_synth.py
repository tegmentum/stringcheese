#!/usr/bin/env python3
"""Regenerate the ``bpe-byte-fallback-synth`` conformance fixture.

The fixture is a hand-crafted BPE vocabulary with
``byte_fallback: true`` and 20 test cases specifically designed to
exercise the BPE byte-fallback code path in ``BpeTokenizer``. Real
Llama-2 / Mistral / Qwen `tokenizer.json` blobs on the Hub today ship
as ``model.type == "BPE"`` with this flag set — the mirror-image of
the Unigram-shape SentencePiece fixture that lives alongside this one
under ``gen_unigram_byte_fallback_synth.py``.

This script writes two files, in place:

* ``bpe_byte_fallback_synth.json`` — the fixture the conformance
  runner reads (``checkpoint`` / ``source`` / ``note`` / ``cases``).
* ``vocabs/bpe-byte-fallback-synth/tokenizer.json`` — the synthetic
  BPE vocab the runner materialises into a runtime tokenizer.

The expected-id column in each case is computed by an in-file BPE
encoder that mirrors ``BpeTokenizer::encode_region_bpe`` byte for
byte (seed per character, run the naive O(n^2) merge loop, then
fan out unresolved pieces via the 256 reserved ``<0xXX>`` ids). If
the runtime and this script agree the fixture round-trips; if they
diverge, the conformance runner surfaces the mismatch and the fix
goes into whichever side is wrong.

Run from anywhere::

    python3 crates/stringcheese-tokenizer-hf/tests/conformance/gen_bpe_byte_fallback_synth.py
"""

import json
from pathlib import Path

HERE = Path(__file__).resolve().parent

# Vocab shape mirrors the Llama-2 BPE convention: <unk>/<s>/</s> at
# ids 0..2, the 256 reserved byte-fallback tokens at ids 3..=258, then
# single-character entries for the ASCII letters we build words out of,
# and finally the merged compound pieces those letters compose into.
BYTE_BASE = 3
vocab: dict[str, int] = {"<unk>": 0, "<s>": 1, "</s>": 2}
for b in range(256):
    vocab[f"<0x{b:02X}>"] = BYTE_BASE + b
# Single-character entries (character-BPE seeds). Every letter used in
# a merge must appear here for the seed piece to resolve.
letters = ["h", "i", "e", "l", "o", "w", "r", "d"]
next_id = BYTE_BASE + 256
for c in letters:
    vocab[c] = next_id
    next_id += 1
# Merged compound entries (every intermediate merge target and every
# terminal whole-word target must be present).
extras = ["hi", "ll", "he", "hell", "hello", "wo", "wor", "worl", "world"]
for w in extras:
    vocab[w] = next_id
    next_id += 1

# Merges by priority (lower rank = higher priority; entry index = rank).
# Every intermediate merged surface must be in the vocab so the piece
# lookup succeeds after the merge fires.
merges: list[tuple[str, str]] = [
    ("h", "i"),
    ("l", "l"),
    ("h", "e"),
    ("he", "ll"),
    ("hell", "o"),
    ("w", "o"),
    ("wo", "r"),
    ("wor", "l"),
    ("worl", "d"),
]

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
        "type": "BPE",
        "vocab": vocab,
        "merges": [[l, r] for (l, r) in merges],
        "byte_fallback": True,
    },
}


def encode(text: str) -> list[int]:
    """Same shape as ``BpeTokenizer::encode_region_bpe`` for byte-
    fallback + no pre-tokenizer + no specials.

    We seed per character (byte-fallback semantics), run a naive O(n^2)
    merge loop, then fan out any piece whose bytes are not in the vocab
    via its raw UTF-8 bytes' ``<0xXX>`` ids.
    """
    if not text:
        return []
    # The BPE crate's whitespace-split fallback DROPS whitespace when
    # no pre_tokenizer is configured. Match that so the fixture's
    # expected-id column matches the runtime byte for byte.
    words: list[str] = []
    cursor = 0
    while cursor < len(text):
        while cursor < len(text) and text[cursor].isspace():
            cursor += 1
        if cursor >= len(text):
            break
        start = cursor
        while cursor < len(text) and not text[cursor].isspace():
            cursor += 1
        words.append(text[start:cursor])
    # If the input has no whitespace-separated tokens, treat the whole
    # input as one word (matches the pre-tokenizer's "no words but
    # non-empty" case).
    if not words and text and any(not c.isspace() for c in text):
        words = [text]

    merge_rank = {(l, r): i for i, (l, r) in enumerate(merges)}
    reverse_vocab = {surface: tid for surface, tid in vocab.items()}
    byte_ids = [BYTE_BASE + b for b in range(256)]

    out: list[int] = []
    for word in words:
        # Seed per character with the char's raw UTF-8 bytes.
        pieces: list[bytes] = [c.encode("utf-8") for c in word]
        # Naive merge loop: repeatedly find the adjacent pair with the
        # lowest merge rank and combine them. Ties break by leftmost.
        while len(pieces) > 1:
            best_idx = None
            best_rank = None
            for i in range(len(pieces) - 1):
                l = pieces[i].decode("utf-8", errors="replace")
                r = pieces[i + 1].decode("utf-8", errors="replace")
                # Merge-lookup by concatenated surface strings. The
                # runtime keys by concatenated bytes, but for our
                # character-BPE synth the two are equivalent.
                rank = merge_rank.get((l, r))
                if rank is None:
                    continue
                if best_rank is None or rank < best_rank:
                    best_rank = rank
                    best_idx = i
            if best_idx is None:
                break
            merged = pieces[best_idx] + pieces[best_idx + 1]
            pieces = pieces[:best_idx] + [merged] + pieces[best_idx + 2:]

        for p in pieces:
            surface = p.decode("utf-8", errors="replace")
            tid = reverse_vocab.get(surface)
            if tid is not None:
                out.append(tid)
                continue
            # Byte-fallback fan-out: one reserved id per raw byte of
            # the piece, forward order.
            for b in p:
                out.append(byte_ids[b])
    return out


inputs: list[tuple[str, str]] = [
    ("empty", ""),
    ("single-letter-h", "h"),
    ("two-letters-merge-hi", "hi"),
    ("single-word-hello", "hello"),
    ("single-word-world", "world"),
    ("concat-hello-world", "helloworld"),
    ("prefix-he", "he"),
    ("prefix-hell", "hell"),
    ("no-merge-path-eh", "eh"),
    ("ascii-all-oov", "abc"),
    ("emoji-4byte-fallback", "\U0001f600"),
    ("mixed-word-plus-4byte-emoji", "hi\U0001f600"),
    ("mixed-4byte-emoji-plus-word", "\U0001f600hi"),
    ("repeated-emoji", "\U0001f600\U0001f600"),
    ("latin-2byte-utf8", "é"),
    ("snowman-3byte-utf8", "☃"),
    ("cjk-3byte-utf8", "你"),
    ("word-plus-ascii-oov", "hi?"),
    ("concat-hi-hello", "hihello"),
    ("concat-hi-hello-world", "hihelloworld"),
]
cases = [
    {"input": inp, "expected_ids": encode(inp), "note": note}
    for note, inp in inputs
]

fixture = {
    "checkpoint": "bpe-byte-fallback-synth",
    "source": (
        "hand-crafted synthetic BPE vocab; expected ids computed by "
        "gen_bpe_byte_fallback_synth.py against the vocab in "
        "tests/conformance/vocabs/bpe-byte-fallback-synth/tokenizer.json"
    ),
    "note": (
        "Synthetic Llama-2-shape character-BPE vocabulary with "
        "byte_fallback enabled; exercises the <0xXX> byte-fallback "
        "path for a mix of ASCII / Latin / CJK / emoji OOV characters "
        "alongside vocab-covered merge chains. The vocab is hand-"
        "crafted (no upstream reference tool involved) - its whole "
        "purpose is to exercise the BPE-side byte-fallback code path in "
        "the conformance runner, mirroring the Unigram-side fixture. "
        "IDs 3..=258 are the reserved <0x00>..<0xFF> byte-fallback "
        "tokens; ids 259..=266 are single-character seeds (h, i, e, l, "
        "o, w, r, d); ids 267..=275 are merged compounds (hi, ll, he, "
        "hell, hello, wo, wor, worl, world)."
    ),
    "cases": cases,
}

(HERE / "bpe_byte_fallback_synth.json").write_text(
    json.dumps(fixture, indent=2, ensure_ascii=False) + "\n",
    encoding="utf-8",
)
vocab_dir = HERE / "vocabs" / "bpe-byte-fallback-synth"
vocab_dir.mkdir(parents=True, exist_ok=True)
(vocab_dir / "tokenizer.json").write_text(
    json.dumps(tokenizer_json, indent=2, ensure_ascii=False) + "\n",
    encoding="utf-8",
)
print(f"wrote {HERE / 'bpe_byte_fallback_synth.json'}")
print(f"wrote {vocab_dir / 'tokenizer.json'}")
print(f"cases: {len(cases)}, vocab size: {len(vocab)}")
