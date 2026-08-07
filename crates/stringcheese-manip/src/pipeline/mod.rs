//! Declarative transformation pipeline — the crate's transformation IR.
//!
//! Where the other modules in this crate expose *one* transformation
//! shape per call site, [`TextPipeline`] captures an **ordered sequence
//! of transformations** as a first-class value. The pipeline is built up
//! declaratively:
//!
//! ```
//! use stringcheese_manip::pipeline::{
//!     CaseFold, CaseKind, CollapseWhitespace, TextPipeline, Trim,
//! };
//! use stringcheese_manip::trim;
//!
//! let clean = TextPipeline::new()
//!     .then(Trim(trim::Trim::whitespace()))
//!     .then(CollapseWhitespace)
//!     .then(CaseFold(CaseKind::Lower));
//!
//! assert_eq!(clean.apply("  Hello    WORLD  "), "hello world");
//! ```
//!
//! and then applied any number of times. Each stage is a
//! [`Box<dyn Operation>`] carrying its own name and `Debug` output, so a
//! pipeline is inspectable at runtime without knowing the concrete types
//! that produced it.
//!
//! # Fused execution over ping-pong buffers
//!
//! A naive "apply each stage to a `String`" loop would allocate one
//! `String` per stage, then throw the previous one away. [`TextPipeline`]
//! instead ping-pongs a pair of `String` buffers: the previous stage's
//! output is the next stage's input, and the two buffers swap roles
//! (via [`core::mem::swap`], which only rotates the three-word `String`
//! header — no bytes are copied). The result: **two allocations total**,
//! regardless of stage count, with the buffers growing to accommodate the
//! largest intermediate rather than the sum of intermediates.
//!
//! # Short-circuiting
//!
//! [`Operation::apply`] returns `bool` — a `false` return value tells
//! [`TextPipeline`] to stop iterating after appending whatever the
//! operation wrote. This is the escape hatch for **budget-limited**
//! operations: [`Truncate`] short-circuits once its byte budget is met so
//! downstream stages do not process bytes the caller has committed to
//! discard.
//!
//! # Concrete operations
//!
//! The `Operation` implementors in this module are thin wrappers around
//! the shipping modules elsewhere in this crate. The enum-tagged variants
//! ([`Normalize`], [`CaseFold`], [`Escape`]) keep the pipeline value's
//! shape small and inspectable — a `Debug`-printed pipeline reads as a
//! declarative recipe.
//!
//! | Wrapper | Delegates to | Boundary |
//! |---------|--------------|----------|
//! | [`Trim`] | [`crate::trim::Trim`] | scalar |
//! | [`Normalize`] | [`crate::normalize`] free functions | mixed (see [`NormalizeKind`]) |
//! | [`CaseFold`] | [`crate::case`] free functions | grapheme / scalar (see [`CaseKind`]) |
//! | [`CollapseWhitespace`] | [`crate::normalize::collapse_whitespace`] | scalar |
//! | [`Remove`] | [`crate::replace::remove`] | byte-substring |
//! | [`Replace`] | [`crate::replace::replace`] | byte-substring |
//! | [`Escape`] | [`crate::escape`] free functions | byte / scalar (see [`EscapeKind`]) |
//! | [`Truncate`] | *(inline; byte-budget with scalar-aligned cut)* | byte |
//!
//! Callers who need a transformation not in this list can implement
//! [`Operation`] themselves — the trait is public.
//!
//! # `no_std`
//!
//! Gated on `feature = "alloc"`. [`TextPipeline`] stores stages in a
//! `Vec<Box<dyn Operation>>` and produces owned `String` output.

#![cfg(feature = "alloc")]

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use crate::escape::PercentSet;
use crate::normalize::LineEnding;
use crate::{case, escape, normalize, replace, trim};

#[cfg(test)]
mod tests;

// =====================================================================
// Trait
// =====================================================================

/// One text transformation.
///
/// A pipeline is an ordered list of `Operation`s ([`TextPipeline`]). Each
/// operation reads an `input: &str`, appends its output to a caller-owned
/// `String`, and returns whether the pipeline should continue.
///
/// # Contract
///
/// - **Append-only.** Implementors must only `push` / `push_str` into
///   `out` — the caller may pre-fill `out` (with previous stages'
///   scratch output, for example) and expects the operation's write to
///   land at the end.
/// - **Total.** Every operation must produce a valid UTF-8 `String`; since
///   `out` is `&mut String`, this is enforced statically.
/// - **Short-circuit sparingly.** Returning `false` stops the pipeline;
///   only budget-limited operations (e.g. [`Truncate`]) should do this.
///   General-purpose operations always return `true`.
///
/// # `Send + Sync`
///
/// `Operation`s are `Send + Sync` so a [`TextPipeline`] can be shared
/// across threads. All the concrete implementations in this module
/// satisfy this trivially because they hold plain data.
///
/// # Debug
///
/// `Operation`s must implement [`Debug`](fmt::Debug) so a pipeline can be
/// printed as a declarative recipe of its stages.
pub trait Operation: Send + Sync + fmt::Debug {
    /// A short, stable name for this operation — used by [`TextPipeline`]
    /// introspection helpers and by manual debugging output.
    ///
    /// The returned string is `&'static str` because the name is a
    /// compile-time property of the operation type, not of any particular
    /// instance's data.
    fn name(&self) -> &'static str;

    /// Apply this operation to `input`, appending the result to `out`.
    ///
    /// Returns `true` if the pipeline should continue with the next
    /// stage, `false` to short-circuit (rare — used by budget-limited
    /// operations such as [`Truncate`]).
    fn apply(&self, input: &str, out: &mut String) -> bool;
}

// =====================================================================
// Pipeline
// =====================================================================

/// Ordered list of operations to apply in one pass.
///
/// See the [module documentation](self) for the full walkthrough and the
/// ping-pong buffer strategy that makes [`apply`](Self::apply) allocate
/// two `String`s regardless of stage count.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::pipeline::{
///     CaseFold, CaseKind, CollapseWhitespace, TextPipeline, Truncate,
/// };
///
/// let pipeline = TextPipeline::new()
///     .then(CollapseWhitespace)
///     .then(CaseFold(CaseKind::Upper))
///     .then(Truncate(5));
///
/// assert_eq!(pipeline.apply("  hello   world  "), "HELLO");
/// ```
pub struct TextPipeline {
    stages: Vec<Box<dyn Operation>>,
}

impl TextPipeline {
    /// Creates an empty pipeline.
    ///
    /// An empty pipeline is the identity transformation: [`apply`]
    /// returns its input verbatim.
    ///
    /// [`apply`]: Self::apply
    ///
    /// # Examples
    ///
    /// ```
    /// use stringcheese_manip::pipeline::TextPipeline;
    ///
    /// assert_eq!(TextPipeline::new().apply("hello"), "hello");
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self { stages: Vec::new() }
    }

    /// Appends `op` to this pipeline, returning `self` for chaining.
    ///
    /// # Examples
    ///
    /// ```
    /// use stringcheese_manip::pipeline::{CaseFold, CaseKind, TextPipeline};
    ///
    /// let p = TextPipeline::new().then(CaseFold(CaseKind::Lower));
    /// assert_eq!(p.stages().len(), 1);
    /// ```
    #[must_use]
    pub fn then(mut self, op: impl Operation + 'static) -> Self {
        self.stages.push(Box::new(op));
        self
    }

    /// Returns the list of stages in application order.
    ///
    /// Useful for testing, debugging, or serializing a pipeline back to a
    /// recipe. Each element's [`Operation::name`] and `Debug` output
    /// identify the stage.
    ///
    /// # Examples
    ///
    /// ```
    /// use stringcheese_manip::pipeline::{CaseFold, CaseKind, TextPipeline};
    ///
    /// let p = TextPipeline::new().then(CaseFold(CaseKind::Lower));
    /// assert_eq!(p.stages()[0].name(), "CaseFold");
    /// ```
    #[must_use]
    pub fn stages(&self) -> &[Box<dyn Operation>] {
        &self.stages
    }

    /// Runs the pipeline on `input`, returning the fully transformed
    /// `String`.
    ///
    /// Two `String` buffers are allocated internally (regardless of stage
    /// count) and swapped between stages via [`core::mem::swap`] — see
    /// the [module documentation](self) for the ping-pong strategy.
    ///
    /// If any operation short-circuits (returns `false`), execution stops
    /// after appending that operation's output; downstream stages are
    /// skipped.
    ///
    /// # Examples
    ///
    /// ```
    /// use stringcheese_manip::pipeline::{CaseFold, CaseKind, TextPipeline};
    ///
    /// let out = TextPipeline::new()
    ///     .then(CaseFold(CaseKind::Upper))
    ///     .apply("hello");
    /// assert_eq!(out, "HELLO");
    /// ```
    #[must_use]
    pub fn apply(&self, input: &str) -> String {
        if self.stages.is_empty() {
            return String::from(input);
        }
        // `current` holds the running output; `scratch` is the write
        // target for the next stage. After each stage we swap them, so
        // `current` always names the most recent output on entry to the
        // loop body.
        let mut current = String::from(input);
        let mut scratch = String::new();
        for stage in &self.stages {
            scratch.clear();
            let keep_going = stage.apply(&current, &mut scratch);
            core::mem::swap(&mut current, &mut scratch);
            if !keep_going {
                break;
            }
        }
        current
    }

    /// Runs the pipeline on `input`, appending the result to `out`.
    ///
    /// The single-stage fast path writes directly into `out`, avoiding
    /// the two-buffer dance. For multi-stage pipelines, intermediate
    /// stages ping-pong on two internal scratch buffers and the *final*
    /// stage writes directly into `out` — so `out` receives the finished
    /// text in a single append, ready to be concatenated with whatever
    /// the caller has already accumulated there.
    ///
    /// # Examples
    ///
    /// ```
    /// use stringcheese_manip::pipeline::{CaseFold, CaseKind, TextPipeline};
    ///
    /// let p = TextPipeline::new().then(CaseFold(CaseKind::Upper));
    /// let mut buf = String::from("greeting: ");
    /// p.apply_into("hello", &mut buf);
    /// assert_eq!(buf, "greeting: HELLO");
    /// ```
    pub fn apply_into(&self, input: &str, out: &mut String) {
        match self.stages.len() {
            0 => {
                out.push_str(input);
                return;
            }
            1 => {
                self.stages[0].apply(input, out);
                return;
            }
            _ => {}
        }
        // Multi-stage: ping-pong on two internal buffers, then the final
        // stage writes directly into `out`.
        let mut a = String::new();
        let mut b = String::new();
        // Stage 0: input → a.
        if !self.stages[0].apply(input, &mut a) {
            out.push_str(&a);
            return;
        }
        let mut src_is_a = true;
        // Middle stages (all but the last): ping-pong a ↔ b.
        let last = self.stages.len() - 1;
        for stage in &self.stages[1..last] {
            if src_is_a {
                b.clear();
                if !stage.apply(&a, &mut b) {
                    out.push_str(&b);
                    return;
                }
            } else {
                a.clear();
                if !stage.apply(&b, &mut a) {
                    out.push_str(&a);
                    return;
                }
            }
            src_is_a = !src_is_a;
        }
        // Final stage: source → out (avoids one copy vs. writing to
        // scratch and then push_str-ing into `out`).
        let src: &str = if src_is_a { &a } else { &b };
        self.stages[last].apply(src, out);
    }
}

impl Default for TextPipeline {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for TextPipeline {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Print as `TextPipeline { stages: [stage1, stage2, ...] }` using
        // each stage's own Debug output. `f.debug_list()` would double-
        // wrap the stages field; the manual builder keeps the shape flat.
        let mut dbg = f.debug_struct("TextPipeline");
        dbg.field("stages", &self.stages);
        dbg.finish()
    }
}

// =====================================================================
// Concrete operations
// =====================================================================

// -------- Trim -------------------------------------------------------

/// Pipeline wrapper around a configured [`crate::trim::Trim`] policy.
///
/// The inner [`trim::Trim`] captures the trim strategy (whitespace,
/// character set, or predicate) and which edges to strip. Building the
/// policy once and reusing it across pipeline invocations amortizes the
/// setup cost (a `Vec<char>` for char-set trims, a `Box<dyn Fn>` for
/// predicate trims).
///
/// # Examples
///
/// ```
/// use stringcheese_manip::pipeline::{TextPipeline, Trim};
/// use stringcheese_manip::trim;
///
/// let pipeline = TextPipeline::new().then(Trim(trim::Trim::whitespace()));
/// assert_eq!(pipeline.apply("  hi  "), "hi");
/// ```
#[derive(Debug)]
pub struct Trim(pub trim::Trim);

impl Operation for Trim {
    fn name(&self) -> &'static str {
        "Trim"
    }
    fn apply(&self, input: &str, out: &mut String) -> bool {
        out.push_str(self.0.apply(input));
        true
    }
}

// -------- Normalize --------------------------------------------------

/// Which [`crate::normalize`] transformation to apply.
///
/// See the [`normalize`] module for the semantics of each variant;
/// this enum simply names them for pipeline inclusion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NormalizeKind {
    /// Collapse whitespace runs to a single ASCII space; strip edges.
    /// See [`crate::normalize::collapse_whitespace`].
    Whitespace,
    /// Rewrite `\r\n`, `\n`, `\r` to the given line ending. See
    /// [`crate::normalize::normalize_line_endings`].
    LineEndings(LineEnding),
    /// Strip Unicode control scalars, keeping `\t`, `\n`, `\r`. See
    /// [`crate::normalize::strip_control`].
    Control,
    /// Strip ANSI escape sequences. See [`crate::normalize::strip_ansi`].
    Ansi,
    /// Replace typographic quotes with ASCII. See
    /// [`crate::normalize::normalize_quotes`].
    Quotes,
    /// Replace em/en dashes with ASCII `--` / `-`. See
    /// [`crate::normalize::normalize_dashes`].
    Dashes,
    /// Replace the horizontal ellipsis scalar with `...`. See
    /// [`crate::normalize::normalize_ellipsis`].
    Ellipsis,
    /// Apply Unicode NFC composition. See [`crate::normalize::nfc`].
    Nfc,
    /// Apply Unicode NFD decomposition. See [`crate::normalize::nfd`].
    Nfd,
    /// Apply Unicode NFKC composition. See [`crate::normalize::nfkc`].
    Nfkc,
    /// Apply Unicode NFKD decomposition. See [`crate::normalize::nfkd`].
    Nfkd,
}

/// Pipeline wrapper around the [`crate::normalize`] free functions.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::pipeline::{Normalize, NormalizeKind, TextPipeline};
///
/// let p = TextPipeline::new().then(Normalize(NormalizeKind::Quotes));
/// assert_eq!(p.apply("\u{201C}hi\u{201D}"), "\"hi\"");
/// ```
#[derive(Debug)]
pub struct Normalize(pub NormalizeKind);

impl Operation for Normalize {
    fn name(&self) -> &'static str {
        "Normalize"
    }
    fn apply(&self, input: &str, out: &mut String) -> bool {
        // Every normalize function returns a fresh `String`; we push its
        // contents into `out` rather than swap so the caller's buffer
        // retains its capacity. The one exception is `LineEndings`, which
        // we implement inline over `out` to avoid the intermediate alloc.
        match self.0 {
            NormalizeKind::Whitespace => out.push_str(&normalize::collapse_whitespace(input)),
            NormalizeKind::LineEndings(to) => {
                out.push_str(&normalize::normalize_line_endings(input, to));
            }
            NormalizeKind::Control => out.push_str(&normalize::strip_control(input)),
            NormalizeKind::Ansi => out.push_str(&normalize::strip_ansi(input)),
            NormalizeKind::Quotes => out.push_str(&normalize::normalize_quotes(input)),
            NormalizeKind::Dashes => out.push_str(&normalize::normalize_dashes(input)),
            NormalizeKind::Ellipsis => out.push_str(&normalize::normalize_ellipsis(input)),
            NormalizeKind::Nfc => out.push_str(&normalize::nfc(input)),
            NormalizeKind::Nfd => out.push_str(&normalize::nfd(input)),
            NormalizeKind::Nfkc => out.push_str(&normalize::nfkc(input)),
            NormalizeKind::Nfkd => out.push_str(&normalize::nfkd(input)),
        }
        true
    }
}

// -------- CaseFold ---------------------------------------------------

/// Which [`crate::case`] transformation to apply.
///
/// See the [`case`] module for the semantics of each variant. The
/// pipeline shortcut is [`CaseFold`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CaseKind {
    /// Full Unicode lowercase. See [`crate::case::to_lowercase`].
    Lower,
    /// Full Unicode uppercase. See [`crate::case::to_uppercase`].
    Upper,
    /// Title case (per-word uppercase-first, lowercase-rest). See
    /// [`crate::case::to_title_case`].
    Title,
    /// Uppercase only the first character. See [`crate::case::capitalize`].
    Capitalize,
    /// ASCII-only lowercase fast path. See
    /// [`crate::case::to_lowercase_ascii`].
    LowerAscii,
    /// ASCII-only uppercase fast path. See
    /// [`crate::case::to_uppercase_ascii`].
    UpperAscii,
}

/// Pipeline wrapper around the [`crate::case`] free functions.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::pipeline::{CaseFold, CaseKind, TextPipeline};
///
/// let p = TextPipeline::new().then(CaseFold(CaseKind::Title));
/// assert_eq!(p.apply("hello world"), "Hello World");
/// ```
#[derive(Debug)]
pub struct CaseFold(pub CaseKind);

impl Operation for CaseFold {
    fn name(&self) -> &'static str {
        "CaseFold"
    }
    fn apply(&self, input: &str, out: &mut String) -> bool {
        match self.0 {
            // `_into` variants exist for Lower / Upper / Title / Capitalize
            // and append directly to `out` without an intermediate `String`.
            CaseKind::Lower => case::to_lowercase_into(input, out),
            CaseKind::Upper => case::to_uppercase_into(input, out),
            CaseKind::Title => case::to_title_case_into(input, out),
            CaseKind::Capitalize => case::capitalize_into(input, out),
            // ASCII fast paths return owned Strings only; push their
            // contents into `out`. The wrapped call is already O(n) so
            // the extra copy is not asymptotically significant.
            CaseKind::LowerAscii => out.push_str(&case::to_lowercase_ascii(input)),
            CaseKind::UpperAscii => out.push_str(&case::to_uppercase_ascii(input)),
        }
        true
    }
}

// -------- CollapseWhitespace ----------------------------------------

/// Pipeline shortcut for `Normalize(NormalizeKind::Whitespace)`.
///
/// Whitespace collapse is the most common normalization step in a text
/// cleanup pipeline; the dedicated marker type keeps the recipe reading
/// naturally. It has no configuration.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::pipeline::{CollapseWhitespace, TextPipeline};
///
/// let p = TextPipeline::new().then(CollapseWhitespace);
/// assert_eq!(p.apply("  hi   there  "), "hi there");
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct CollapseWhitespace;

impl Operation for CollapseWhitespace {
    fn name(&self) -> &'static str {
        "CollapseWhitespace"
    }
    fn apply(&self, input: &str, out: &mut String) -> bool {
        out.push_str(&normalize::collapse_whitespace(input));
        true
    }
}

// -------- Remove -----------------------------------------------------

/// Pipeline wrapper: remove all non-overlapping occurrences of `.0`.
///
/// Equivalent to [`crate::replace::remove`] — an empty needle is a
/// no-op (per the [`replace`] module's empty-needle policy).
///
/// # Examples
///
/// ```
/// use stringcheese_manip::pipeline::{Remove, TextPipeline};
///
/// let p = TextPipeline::new().then(Remove(String::from("!")));
/// assert_eq!(p.apply("hi!! there!"), "hi there");
/// ```
#[derive(Debug, Clone)]
pub struct Remove(pub String);

impl Operation for Remove {
    fn name(&self) -> &'static str {
        "Remove"
    }
    fn apply(&self, input: &str, out: &mut String) -> bool {
        out.push_str(&replace::remove(input, &self.0));
        true
    }
}

// -------- Replace ----------------------------------------------------

/// Pipeline wrapper: replace every occurrence of `from` with `to`.
///
/// Equivalent to [`crate::replace::replace`] — an empty `from` is a
/// no-op (per the [`replace`] module's empty-needle policy).
///
/// # Examples
///
/// ```
/// use stringcheese_manip::pipeline::{Replace, TextPipeline};
///
/// let p = TextPipeline::new().then(Replace {
///     from: String::from("cat"),
///     to: String::from("dog"),
/// });
/// assert_eq!(p.apply("cat and cat"), "dog and dog");
/// ```
#[derive(Debug, Clone)]
pub struct Replace {
    /// The needle to search for.
    pub from: String,
    /// The replacement to substitute in.
    pub to: String,
}

impl Operation for Replace {
    fn name(&self) -> &'static str {
        "Replace"
    }
    fn apply(&self, input: &str, out: &mut String) -> bool {
        out.push_str(&replace::replace(input, &self.from, &self.to));
        true
    }
}

// -------- Escape -----------------------------------------------------

/// Which [`crate::escape`] encoding to apply.
///
/// See the [`escape`] module for the encoding rules of each variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EscapeKind {
    /// HTML text / attribute value. See [`crate::escape::escape_html`].
    Html,
    /// JSON string literal contents. See [`crate::escape::escape_json`].
    Json,
    /// POSIX shell single-quote-safe. See
    /// [`crate::escape::escape_shell_posix`].
    ShellPosix,
    /// Windows `cmd.exe` argument quoting. See
    /// [`crate::escape::escape_shell_windows`].
    ShellWindows,
    /// RFC 3986 percent-encoding for the given URI component set. See
    /// [`crate::escape::percent_encode`].
    Percent(PercentSet),
    /// C-string escaping. See [`crate::escape::escape_c_string`].
    CString,
    /// Regex meta-character escaping. See [`crate::escape::escape_regex`].
    Regex,
}

/// Pipeline wrapper around the [`crate::escape`] encoding functions.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::pipeline::{Escape, EscapeKind, TextPipeline};
///
/// let p = TextPipeline::new().then(Escape(EscapeKind::Html));
/// assert_eq!(p.apply("<b>&</b>"), "&lt;b&gt;&amp;&lt;/b&gt;");
/// ```
#[derive(Debug)]
pub struct Escape(pub EscapeKind);

impl Operation for Escape {
    fn name(&self) -> &'static str {
        "Escape"
    }
    fn apply(&self, input: &str, out: &mut String) -> bool {
        match self.0 {
            EscapeKind::Html => out.push_str(&escape::escape_html(input)),
            EscapeKind::Json => out.push_str(&escape::escape_json(input)),
            EscapeKind::ShellPosix => out.push_str(&escape::escape_shell_posix(input)),
            EscapeKind::ShellWindows => out.push_str(&escape::escape_shell_windows(input)),
            EscapeKind::Percent(set) => out.push_str(&escape::percent_encode(input, set)),
            EscapeKind::CString => out.push_str(&escape::escape_c_string(input)),
            EscapeKind::Regex => out.push_str(&escape::escape_regex(input)),
        }
        true
    }
}

// -------- Truncate ---------------------------------------------------

/// Pipeline wrapper: cap the output at `.0` bytes, splitting at the last
/// UTF-8 scalar boundary that fits.
///
/// This is the canonical budget-limited operation: when the input is
/// longer than the limit, [`Operation::apply`] writes the truncated
/// prefix and returns `false` to short-circuit the pipeline. Downstream
/// stages are skipped — there is no point normalizing bytes the caller
/// has committed to discard.
///
/// The byte limit is respected: if truncating at exactly `.0` bytes
/// would split a multi-byte scalar, the cut is moved earlier to the last
/// scalar boundary at or before the limit. The returned prefix is
/// therefore always valid UTF-8 and always at most `.0` bytes long.
///
/// A limit of `0` yields an empty output and short-circuits.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::pipeline::{TextPipeline, Truncate};
///
/// let p = TextPipeline::new().then(Truncate(5));
/// assert_eq!(p.apply("hello world"), "hello");
/// // Multi-byte scalars are respected — no partial UTF-8 in the output:
/// let p = TextPipeline::new().then(Truncate(2));
/// // "é" is two bytes; "éé" is four. Limit 2 keeps the first "é".
/// assert_eq!(p.apply("\u{00E9}\u{00E9}"), "\u{00E9}");
/// // A too-tight limit still returns valid UTF-8 (possibly empty).
/// let p = TextPipeline::new().then(Truncate(1));
/// assert_eq!(p.apply("\u{00E9}"), "");
/// ```
#[derive(Debug, Clone, Copy)]
pub struct Truncate(pub usize);

impl Operation for Truncate {
    fn name(&self) -> &'static str {
        "Truncate"
    }
    fn apply(&self, input: &str, out: &mut String) -> bool {
        if input.len() <= self.0 {
            out.push_str(input);
            return true;
        }
        // Walk back from the limit to the previous UTF-8 scalar boundary.
        // `is_char_boundary` is O(1) — the loop runs at most 3 times
        // (max UTF-8 scalar length is 4 bytes).
        let mut cut = self.0;
        while cut > 0 && !input.is_char_boundary(cut) {
            cut -= 1;
        }
        out.push_str(&input[..cut]);
        false
    }
}
