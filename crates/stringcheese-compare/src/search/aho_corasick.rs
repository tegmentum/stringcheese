//! Aho-Corasick multi-pattern search over `&[u8]`.
//!
//! # Algorithm
//!
//! An Aho-Corasick automaton is a trie over the pattern set, augmented
//! with *failure links* that generalize KMP's failure function to the
//! multi-pattern case. A single left-to-right pass over the haystack
//! reports every match of every pattern — including overlapping matches
//! from different patterns.
//!
//! Construction proceeds in three phases:
//!
//! 1. **Trie** — insert every pattern into a trie, tagging the terminal
//!    state of each pattern with that pattern's index in the input slice.
//! 2. **Failure links** — for every state `s`, `failure[s]` is the state
//!    reached by the longest proper suffix of the string spelled from the
//!    root to `s` that is also a prefix of some pattern. Computed by a
//!    breadth-first traversal from the root.
//! 3. **Output links** — for every state, the set of patterns that end at
//!    that state *or* at any state reachable by following failure links
//!    from it. These are propagated during the BFS so a single lookup at
//!    match time returns every pattern that finishes at the current byte.
//!
//! # Data structures
//!
//! Goto edges are stored as a `BTreeMap<u8, u32>` per state. A dense
//! `[u32; 256]` per state would give faster lookups but would allocate
//! `2 KiB` per state — prohibitive for even moderately large pattern
//! sets. The `BTreeMap` choice gives `O(log Σ)` transitions on states
//! that carry many out-edges (rare in practice) and `O(1)` allocation
//! amortized on typical sparse states.
//!
//! Output sets are stored as a `Vec<usize>` per state, containing the
//! indices of every pattern that terminates at that state after failure-
//! link propagation. Empty output sets are the common case; the vector
//! representation keeps them zero-cost.
//!
//! # Descriptor
//!
//! The variant slug is `"classic-1975"`, matching the algorithm as
//! published in the original Aho-Corasick paper.
//!
//! # Empty patterns
//!
//! An empty pattern would nominally match at every position, including
//! after the last byte. To stay consistent with the single-pattern
//! algorithms' "empty pattern matches at position 0 exactly once"
//! policy, empty patterns in the input to [`AhoCorasick::build`] are
//! reported once at position 0 for each occurrence in the pattern set.
//!
//! # References
//!
//! * Aho, A. V., & Corasick, M. J. (1975). "Efficient string matching: an
//!   aid to bibliographic search." *Communications of the ACM*, 18(6),
//!   333-340. <https://doi.org/10.1145/360825.360855>

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use stringcheese_core::{
    AlgorithmDescriptor, AlgorithmFamily, DefinitionSource, DescriptorVersion, VariantId,
};

use crate::search::api::{Match, SearchAlgorithm};

/// An Aho-Corasick multi-pattern search automaton (Aho & Corasick, 1975).
///
/// Built once from an arbitrary pattern set with [`AhoCorasick::build`];
/// reused across any number of haystacks with [`AhoCorasick::find_all`].
///
/// This type implements [`SearchAlgorithm`] to advertise its descriptor;
/// it does not implement `SinglePatternSearch` because its
/// preparation shape carries many patterns rather than one.
#[derive(Clone, Debug)]
pub struct AhoCorasick {
    /// The set of states. State `0` is always the root.
    states: Vec<State>,
    /// Original pattern lengths, indexed by pattern index. Needed at
    /// match time to compute `position = end - pattern_length`.
    pattern_lengths: Vec<usize>,
    /// The number of patterns registered in [`AhoCorasick::build`],
    /// preserved separately so an empty pattern doesn't get lost in the
    /// state array.
    pattern_count: usize,
}

/// A single automaton state.
#[derive(Clone, Debug, Default)]
struct State {
    /// Goto transitions from this state — a sparse map from input byte to
    /// destination state index.
    goto: BTreeMap<u8, u32>,
    /// Failure link — the state reached by the longest proper suffix of
    /// the string spelled from the root to this state that is also a
    /// prefix of some pattern. `0` (the root) is the fallback.
    failure: u32,
    /// Indices of patterns that terminate at this state after
    /// failure-link propagation.
    outputs: Vec<usize>,
}

impl AhoCorasick {
    /// The algorithm descriptor for this variant.
    pub const DESCRIPTOR: AlgorithmDescriptor = AlgorithmDescriptor {
        family: AlgorithmFamily::AhoCorasick,
        variant: VariantId("classic-1975"),
        version: DescriptorVersion::new(0, 1, 0),
        source: DefinitionSource::Paper {
            title: "Efficient string matching: an aid to bibliographic search",
            authors: "A. V. Aho, M. J. Corasick",
            year: 1975,
        },
    };

    /// Returns the algorithm descriptor for this variant.
    #[inline]
    #[must_use]
    pub const fn descriptor() -> AlgorithmDescriptor {
        Self::DESCRIPTOR
    }

    /// Builds an Aho-Corasick automaton from the given patterns.
    ///
    /// Each pattern's index in `patterns` becomes its
    /// [`Match::pattern_index`] in the reported matches. Duplicate
    /// patterns are supported — every terminal index attaches to the
    /// shared terminal state and is reported at each match.
    ///
    /// # Empty patterns
    ///
    /// Empty patterns are recorded and reported once per haystack at
    /// position 0. This mirrors the single-pattern algorithms' empty-pattern
    /// policy documented in [`crate::search::api`].
    ///
    /// # Panics
    ///
    /// Panics if the automaton's total state count would exceed
    /// [`u32::MAX`]. In practice this bound is unreachable for realistic
    /// pattern sets — it corresponds to roughly four billion trie states.
    #[must_use]
    pub fn build(patterns: &[&[u8]]) -> Self {
        let mut ac = Self {
            states: alloc::vec![State::default()],
            pattern_lengths: patterns.iter().map(|p| p.len()).collect(),
            pattern_count: patterns.len(),
        };

        // Phase 1 — insert every non-empty pattern into the trie. Empty
        // patterns are handled entirely at match time (see `find_all`)
        // so that they do not attach an output to the root state, which
        // would fire on every haystack byte.
        for (pattern_index, pattern) in patterns.iter().enumerate() {
            if pattern.is_empty() {
                continue;
            }
            let mut current: u32 = 0;
            for &b in *pattern {
                if let Some(&next) = ac.states[current as usize].goto.get(&b) {
                    current = next;
                } else {
                    let next =
                        u32::try_from(ac.states.len()).expect("automaton state count fits in u32");
                    ac.states.push(State::default());
                    ac.states[current as usize].goto.insert(b, next);
                    current = next;
                }
            }
            ac.states[current as usize].outputs.push(pattern_index);
        }

        // Phase 2 — compute failure links by BFS from the root.
        // Depth-1 states all fail back to the root; then each subsequent
        // BFS step propagates the failure of the parent through goto.
        let mut queue: Vec<u32> = Vec::new();
        // Collect depth-1 children of the root and set their failure to root.
        let root_children: Vec<(u8, u32)> =
            ac.states[0].goto.iter().map(|(&b, &s)| (b, s)).collect();
        for (_, child) in &root_children {
            ac.states[*child as usize].failure = 0;
            queue.push(*child);
        }

        let mut head = 0usize;
        while head < queue.len() {
            let r = queue[head];
            head += 1;
            // For each transition (a, s) out of r, compute s's failure.
            let transitions: Vec<(u8, u32)> = ac.states[r as usize]
                .goto
                .iter()
                .map(|(&b, &t)| (b, t))
                .collect();
            for (a, s) in transitions {
                queue.push(s);
                let mut state = ac.states[r as usize].failure;
                // Follow failure links until a state has a goto on `a`.
                // The root's fallback is itself; that terminates the loop.
                while state != 0 && !ac.states[state as usize].goto.contains_key(&a) {
                    state = ac.states[state as usize].failure;
                }
                let fail = if let Some(&t) = ac.states[state as usize].goto.get(&a) {
                    if t == s { 0 } else { t }
                } else {
                    0
                };
                ac.states[s as usize].failure = fail;

                // Propagate the failure state's outputs into s's outputs.
                // Cloning is fine — the output vectors are tiny except in
                // pathological pattern sets.
                let extra = ac.states[fail as usize].outputs.clone();
                ac.states[s as usize].outputs.extend(extra);
            }
        }

        ac
    }

    /// Returns the number of patterns registered by [`AhoCorasick::build`].
    #[inline]
    #[must_use]
    pub fn pattern_count(&self) -> usize {
        self.pattern_count
    }

    /// Returns the length of the pattern at the given index in the input
    /// slice passed to [`AhoCorasick::build`].
    ///
    /// # Panics
    ///
    /// Panics if `pattern_index >= self.pattern_count()`.
    #[inline]
    #[must_use]
    pub fn pattern_length(&self, pattern_index: usize) -> usize {
        self.pattern_lengths[pattern_index]
    }

    /// Returns whether state `s` has a goto transition on byte `b`.
    ///
    /// Exposed as a `pub(crate)` accessor so the streaming wrapper in
    /// [`crate::search::stream`] can drive the automaton without duplicating
    /// the goto/failure/output data structures. Not part of the
    /// long-term public API.
    #[inline]
    #[must_use]
    pub(crate) fn goto_contains_key(&self, s: u32, b: u8) -> bool {
        self.states[s as usize].goto.contains_key(&b)
    }

    /// Returns the goto destination from state `s` on byte `b`, if any.
    #[inline]
    #[must_use]
    pub(crate) fn goto_transition(&self, s: u32, b: u8) -> Option<u32> {
        self.states[s as usize].goto.get(&b).copied()
    }

    /// Returns the failure link of state `s`.
    #[inline]
    #[must_use]
    pub(crate) fn failure_of(&self, s: u32) -> u32 {
        self.states[s as usize].failure
    }

    /// Returns the output pattern indices attached to state `s`.
    #[inline]
    pub(crate) fn outputs_of(&self, s: u32) -> impl Iterator<Item = usize> + '_ {
        self.states[s as usize].outputs.iter().copied()
    }

    /// Streams every match of every registered pattern through
    /// `haystack`.
    ///
    /// Returned matches are in ascending `position` order. When multiple
    /// patterns end at the same haystack position, the order among them
    /// is unspecified except that the sequence overall is nondecreasing
    /// in `position`.
    ///
    /// Empty patterns from the pattern set are reported once at position
    /// `0` — see the type-level documentation.
    #[must_use]
    pub fn find_all(&self, haystack: &[u8]) -> Vec<Match> {
        let mut out = Vec::new();

        // Empty patterns fire once at position 0 per pattern.
        for (idx, &len) in self.pattern_lengths.iter().enumerate() {
            if len == 0 {
                out.push(Match::with_pattern(0, idx));
            }
        }

        let mut current: u32 = 0;
        for (i, &b) in haystack.iter().enumerate() {
            // Follow failure links until a transition on `b` exists or
            // we bottom out at the root.
            while current != 0 && !self.states[current as usize].goto.contains_key(&b) {
                current = self.states[current as usize].failure;
            }
            if let Some(&next) = self.states[current as usize].goto.get(&b) {
                current = next;
            }
            // Emit every pattern that terminates at the current state.
            for &pattern_index in &self.states[current as usize].outputs {
                let length = self.pattern_lengths[pattern_index];
                let position = i + 1 - length;
                out.push(Match::with_pattern(position, pattern_index));
            }
        }

        // Matches are inserted in ascending end-position order, but two
        // patterns ending at the same haystack position can differ in
        // length and therefore in start position. Sort by position to
        // meet the "ascending position order" contract.
        out.sort_by_key(|m| (m.position, m.pattern_index));
        out
    }
}

impl SearchAlgorithm for AhoCorasick {
    /// The prepared value for Aho-Corasick is the automaton itself.
    ///
    /// Unlike the single-pattern algorithms, `prepare` here builds an
    /// automaton over exactly one pattern — the multi-pattern entry
    /// point is [`AhoCorasick::build`]. This impl exists so
    /// [`AhoCorasick`] shares the descriptor accessor with the other
    /// algorithms.
    type Prepared = AhoCorasick;

    fn prepare(pattern: &[u8]) -> Self::Prepared {
        AhoCorasick::build(&[pattern])
    }

    fn descriptor() -> AlgorithmDescriptor {
        Self::DESCRIPTOR
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptor_pins_variant_and_source() {
        let d = AhoCorasick::descriptor();
        assert_eq!(d.family, AlgorithmFamily::AhoCorasick);
        assert_eq!(d.variant, VariantId("classic-1975"));
        assert!(matches!(
            d.source,
            DefinitionSource::Paper { year: 1975, .. }
        ));
    }

    #[test]
    fn descriptor_is_const() {
        const D: AlgorithmDescriptor = AhoCorasick::DESCRIPTOR;
        assert_eq!(D.variant.0, "classic-1975");
    }

    #[test]
    fn single_pattern_agrees_with_str_find() {
        // Sanity check: a single-pattern build should agree with
        // haystack.windows-based hand counting.
        let ac = AhoCorasick::build(&[b"abc"]);
        let matches = ac.find_all(b"xxabcxxabcxxab");
        assert_eq!(
            matches,
            alloc::vec![Match::with_pattern(2, 0), Match::with_pattern(7, 0),]
        );
    }

    #[test]
    fn multi_pattern_reports_pattern_indices() {
        // Textbook example from the 1975 paper's spirit: pattern set
        // {"he", "she", "his", "hers"} against haystack "ushers" reports
        // "she" (0), "he" (1), and "hers" (2).
        let patterns: &[&[u8]] = &[b"he", b"she", b"his", b"hers"];
        let ac = AhoCorasick::build(patterns);
        let matches = ac.find_all(b"ushers");
        // Expected matches:
        //   "she" at position 1 -> pattern_index 1
        //   "he"  at position 2 -> pattern_index 0
        //   "hers" at position 2 -> pattern_index 3
        assert_eq!(matches.len(), 3);
        // Sort/assert by (position, pattern_index) since two matches share
        // position 2.
        assert_eq!(matches[0], Match::with_pattern(1, 1));
        assert_eq!(matches[1], Match::with_pattern(2, 0));
        assert_eq!(matches[2], Match::with_pattern(2, 3));
    }

    #[test]
    fn overlapping_matches_of_the_same_pattern() {
        let ac = AhoCorasick::build(&[b"aa"]);
        let matches = ac.find_all(b"aaaa");
        assert_eq!(
            matches,
            alloc::vec![
                Match::with_pattern(0, 0),
                Match::with_pattern(1, 0),
                Match::with_pattern(2, 0),
            ]
        );
    }

    #[test]
    fn pattern_count_matches_input() {
        let ac = AhoCorasick::build(&[b"a", b"b", b"c"]);
        assert_eq!(ac.pattern_count(), 3);
    }

    #[test]
    fn empty_pattern_matches_at_zero_once() {
        let ac = AhoCorasick::build(&[b""]);
        let matches = ac.find_all(b"abc");
        assert_eq!(matches, alloc::vec![Match::with_pattern(0, 0)]);
    }

    #[test]
    fn no_matches_on_disjoint_input() {
        let ac = AhoCorasick::build(&[b"xxx", b"yyy"]);
        assert!(ac.find_all(b"abcdef").is_empty());
    }

    #[test]
    fn duplicate_patterns_report_each_index() {
        let ac = AhoCorasick::build(&[b"abc", b"abc"]);
        let matches = ac.find_all(b"abc");
        assert_eq!(matches.len(), 2);
        assert!(matches.iter().all(|m| m.position == 0));
        assert!(matches.iter().any(|m| m.pattern_index == 0));
        assert!(matches.iter().any(|m| m.pattern_index == 1));
    }
}
