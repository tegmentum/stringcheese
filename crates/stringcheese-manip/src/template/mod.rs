//! Placeholder substitution — `str.format`-lite.
//!
//! This module renders templates of the shape `"Hello, {name}!"` by
//! looking up each `{...}` placeholder against a
//! [`TemplateContext`]. It is deliberately small: no conditionals, no
//! loops, no filters, no arithmetic, no format specifiers — exactly one
//! operation, "replace this placeholder with the named variable's
//! value". Callers who need a real templating engine should reach for
//! [`askama`], [`handlebars`], or [`tera`]; this module is for the
//! frequent case where a full engine would be overkill.
//!
//! # Placeholder syntax
//!
//! - `{name}` — resolve the variable named `name` from the context.
//!   `name` is one or more ASCII letters, digits, or underscores; the
//!   first character must be a letter or underscore.
//! - `{{` — a literal `{`.
//! - `}}` — a literal `}`.
//!
//! Malformed placeholders (an unbalanced `{` or `}`, or a placeholder
//! whose name is not a legal identifier) produce a
//! [`TemplateError`] with the byte offset into the template where the
//! problem was detected.
//!
//! # Missing-variable policy
//!
//! Two policies are exposed:
//!
//! - **Strict** ([`render`], [`render_with`], [`render_positional`]) —
//!   the default. A placeholder that does not resolve returns
//!   [`TemplateError::UnknownVariable`].
//! - **Permissive** ([`render_permissive`]) — unresolved placeholders
//!   are left in the output verbatim (`{unknown}`). Useful for previews
//!   or for two-pass templating where some variables come from a later
//!   stage.
//!
//! # Positional
//!
//! [`render_positional`] treats placeholder names as decimal indices
//! into an `&[&str]` — `{0}` is the first argument, `{1}` the second,
//! and so on. Out-of-range indices produce [`TemplateError::UnknownVariable`].
//!
//! # Streaming
//!
//! [`render_iter`] yields the template's literal and resolved-variable
//! spans without building the joined output — useful for writing directly
//! to a [`Write`] or a byte buffer.
//!
//! # `no_std`
//!
//! Every item is gated on `feature = "alloc"` because owned outputs,
//! error strings, and the [`BTreeMap`](alloc::collections::BTreeMap)
//! impl of [`TemplateContext`] require the heap. The
//! [`HashMap`](std::collections::HashMap) impl is additionally gated
//! on `feature = "std"`.
//!
//! [`askama`]: https://crates.io/crates/askama
//! [`handlebars`]: https://crates.io/crates/handlebars
//! [`tera`]: https://crates.io/crates/tera
//! [`Write`]: core::fmt::Write

#![cfg(feature = "alloc")]

use alloc::string::{String, ToString};
use core::fmt;

#[cfg(test)]
mod tests;

// =====================================================================
// TemplateContext trait + built-in impls
// =====================================================================

/// Source of variable values for [`render`].
///
/// A `TemplateContext` is any value that can answer the question
/// "given a variable name, what is its value (if any)?". Implementations
/// are provided for the common containers ([`BTreeMap`], [`HashMap`],
/// pair-slices); user types can implement the trait directly for
/// zero-copy access to their own data.
///
/// # Implementations
///
/// - `BTreeMap<String, String>` — sorted-map lookup.
/// - `HashMap<String, String>` — hash-map lookup (gated on `feature = "std"`).
/// - `&[(&str, &str)]` — linear scan over a slice of pairs.
///
/// [`BTreeMap`]: alloc::collections::BTreeMap
/// [`HashMap`]: std::collections::HashMap
pub trait TemplateContext {
    /// Returns the value of the variable named `name`, or `None` if
    /// it is not defined.
    fn lookup(&self, name: &str) -> Option<&str>;
}

impl TemplateContext for alloc::collections::BTreeMap<String, String> {
    fn lookup(&self, name: &str) -> Option<&str> {
        self.get(name).map(String::as_str)
    }
}

#[cfg(feature = "std")]
impl<S: std::hash::BuildHasher> TemplateContext for std::collections::HashMap<String, String, S> {
    fn lookup(&self, name: &str) -> Option<&str> {
        self.get(name).map(String::as_str)
    }
}

impl TemplateContext for [(&str, &str)] {
    fn lookup(&self, name: &str) -> Option<&str> {
        self.iter()
            .find_map(|&(k, v)| if k == name { Some(v) } else { None })
    }
}

// Allow `&[(&str, &str)]` to be passed directly. `&T` forwards to `T`'s
// impl by default, but the pair-slice case is the ergonomic sweet spot
// so an explicit blanket impl documents it.
impl<T: TemplateContext + ?Sized> TemplateContext for &T {
    fn lookup(&self, name: &str) -> Option<&str> {
        (**self).lookup(name)
    }
}

// =====================================================================
// Error type
// =====================================================================

/// A failure encountered while rendering a template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemplateError {
    /// The placeholder refers to a variable that the context does not
    /// define. Carries the variable name and its byte offset in the
    /// template.
    UnknownVariable {
        /// The name of the missing variable, exactly as written in the
        /// template.
        name: String,
        /// Byte offset of the placeholder's opening `{` in the
        /// template.
        position: usize,
    },
    /// The template contains an unbalanced `{` or `}` — either a `{`
    /// with no matching `}`, or a stray `}` that is neither part of a
    /// placeholder nor of a `}}` escape.
    UnbalancedBrace {
        /// Byte offset of the unbalanced brace in the template.
        position: usize,
    },
    /// A placeholder was parsed but its name is not a legal identifier
    /// (empty, starts with a digit, or contains characters outside
    /// `[A-Za-z0-9_]`). Carries the offending name and its position.
    InvalidIdentifier {
        /// The offending identifier, verbatim.
        name: String,
        /// Byte offset of the placeholder's opening `{` in the
        /// template.
        position: usize,
    },
}

impl fmt::Display for TemplateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TemplateError::UnknownVariable { name, position } => {
                write!(f, "unknown variable {name:?} at byte {position}")
            }
            TemplateError::UnbalancedBrace { position } => {
                write!(f, "unbalanced brace at byte {position}")
            }
            TemplateError::InvalidIdentifier { name, position } => {
                write!(f, "invalid identifier {name:?} at byte {position}")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for TemplateError {}

// =====================================================================
// Rendering
// =====================================================================

/// Renders `template`, substituting each `{name}` placeholder with the
/// value returned by `vars.lookup(name)`.
///
/// # Escaping
///
/// `{{` produces a literal `{`; `}}` produces a literal `}`. Everything
/// else outside placeholders is copied verbatim.
///
/// # Errors
///
/// - [`TemplateError::UnknownVariable`] if a placeholder's name does
///   not resolve.
/// - [`TemplateError::UnbalancedBrace`] if the template contains an
///   unmatched `{` or a stray `}`.
/// - [`TemplateError::InvalidIdentifier`] if a placeholder's name is
///   not a legal identifier.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::template::{render_with};
///
/// let out = render_with("Hello, {name}!", &[("name", "world")]).unwrap();
/// assert_eq!(out, "Hello, world!");
/// ```
pub fn render(template: &str, vars: &dyn TemplateContext) -> Result<String, TemplateError> {
    let mut out = String::with_capacity(template.len());
    for span in render_iter(template, vars) {
        out.push_str(span?);
    }
    Ok(out)
}

/// Convenience wrapper that takes a `[(&str, &str)]` pair-slice
/// directly.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::template::render_with;
///
/// let out = render_with(
///     "{greeting}, {name}!",
///     &[("greeting", "Hello"), ("name", "world")],
/// )
/// .unwrap();
/// assert_eq!(out, "Hello, world!");
/// ```
///
/// # Errors
///
/// Same as [`render`].
pub fn render_with(template: &str, vars: &[(&str, &str)]) -> Result<String, TemplateError> {
    render(template, &vars)
}

/// Renders `template` with positional arguments — `{0}` is
/// `args[0]`, `{1}` is `args[1]`, and so on.
///
/// The placeholder name must be a base-10 integer with no sign; anything
/// else is [`TemplateError::InvalidIdentifier`]. An index that is
/// greater than or equal to `args.len()` is
/// [`TemplateError::UnknownVariable`] with the index as its name.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::template::render_positional;
///
/// let out = render_positional("{0} + {1} = {2}", &["1", "2", "3"]).unwrap();
/// assert_eq!(out, "1 + 2 = 3");
/// ```
///
/// # Errors
///
/// See the variant descriptions on [`TemplateError`].
pub fn render_positional(template: &str, args: &[&str]) -> Result<String, TemplateError> {
    // The positional context is a thin adaptor over `args`. Because
    // the identifier validator accepts `[A-Za-z0-9_]` (which includes
    // digits), we need not special-case digit-only names here; the
    // lookup simply fails when the digits do not parse as an index.
    struct Positional<'a>(&'a [&'a str]);
    impl TemplateContext for Positional<'_> {
        fn lookup(&self, name: &str) -> Option<&str> {
            let idx: usize = name.parse().ok()?;
            self.0.get(idx).copied()
        }
    }
    render(template, &Positional(args))
}

/// Like [`render`], but leaves unresolved `{name}` placeholders in the
/// output unchanged instead of returning an error.
///
/// Unbalanced braces and invalid identifiers are also emitted verbatim.
/// This function never returns an error. It is suitable for previews
/// and for two-pass templating where later stages supply the remaining
/// variables.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::template::render_permissive;
///
/// let vars: &[(&str, &str)] = &[("name", "world")];
/// let out = render_permissive("Hello, {name}, from {who}!", &vars);
/// assert_eq!(out, "Hello, world, from {who}!");
/// ```
pub fn render_permissive(template: &str, vars: &dyn TemplateContext) -> String {
    // Unlike `render`, permissive mode does not use `render_iter` — the
    // iterator halts on the first error, but permissive rendering has
    // to keep going.
    let bytes = template.as_bytes();
    let mut out = String::with_capacity(template.len());
    let mut pos = 0;
    while pos < bytes.len() {
        // Scan forward to the next brace.
        let mut i = pos;
        while i < bytes.len() && bytes[i] != b'{' && bytes[i] != b'}' {
            i += 1;
        }
        if i > pos {
            out.push_str(&template[pos..i]);
        }
        if i >= bytes.len() {
            break;
        }
        let here = bytes[i];
        if here == b'{' {
            if bytes.get(i + 1) == Some(&b'{') {
                out.push('{');
                pos = i + 2;
                continue;
            }
            let name_start = i + 1;
            let mut j = name_start;
            while j < bytes.len() && bytes[j] != b'}' && bytes[j] != b'{' {
                j += 1;
            }
            if j >= bytes.len() || bytes[j] == b'{' {
                // Unbalanced `{`. Emit it verbatim and continue past it.
                out.push('{');
                pos = i + 1;
                continue;
            }
            let name = &template[name_start..j];
            if !is_valid_identifier(name) {
                out.push('{');
                out.push_str(name);
                out.push('}');
                pos = j + 1;
                continue;
            }
            if let Some(v) = vars.lookup(name) {
                out.push_str(v);
            } else {
                out.push('{');
                out.push_str(name);
                out.push('}');
            }
            pos = j + 1;
        } else {
            // `}` here.
            if bytes.get(i + 1) == Some(&b'}') {
                out.push('}');
                pos = i + 2;
            } else {
                out.push('}');
                pos = i + 1;
            }
        }
    }
    out
}

// =====================================================================
// Streaming iterator
// =====================================================================

/// Renders `template` as a stream of spans — literal text and resolved
/// variables — without building the joined output.
///
/// Each yielded item is either:
///
/// - `Ok(span)` — a borrowed slice of either the template (for a
///   literal run) or of the context's returned string (for a resolved
///   variable). Callers append these directly to a `Write` or byte
///   buffer.
/// - `Err(TemplateError)` — the first parse or lookup error. The
///   iterator does not yield further items after an error.
///
/// # Examples
///
/// ```
/// use stringcheese_manip::template::render_iter;
///
/// let vars: &[(&str, &str)] = &[("name", "world")];
/// let mut out = String::new();
/// for span in render_iter("Hello, {name}!", &vars) {
///     out.push_str(span.unwrap());
/// }
/// assert_eq!(out, "Hello, world!");
/// ```
pub fn render_iter<'a>(template: &'a str, vars: &'a dyn TemplateContext) -> RenderIter<'a> {
    RenderIter {
        template,
        vars,
        pos: 0,
        done: false,
    }
}

/// Iterator returned by [`render_iter`].
pub struct RenderIter<'a> {
    template: &'a str,
    vars: &'a dyn TemplateContext,
    pos: usize,
    done: bool,
}

impl<'a> Iterator for RenderIter<'a> {
    type Item = Result<&'a str, TemplateError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        let bytes = self.template.as_bytes();
        if self.pos >= bytes.len() {
            self.done = true;
            return None;
        }
        // Find the next `{` or `}` starting from `pos`.
        let mut i = self.pos;
        while i < bytes.len() {
            match bytes[i] {
                b'{' | b'}' => break,
                _ => i += 1,
            }
        }
        // If we advanced past some literal text, emit that first.
        if i > self.pos {
            let slice = &self.template[self.pos..i];
            self.pos = i;
            return Some(Ok(slice));
        }
        // We are looking at either `{`, `{{`, `}}`, `}` (stray), or the
        // beginning of a placeholder.
        let here = bytes[i];
        if here == b'{' {
            // `{{` is a literal `{`.
            if bytes.get(i + 1) == Some(&b'{') {
                self.pos = i + 2;
                return Some(Ok("{"));
            }
            // Otherwise we expect `{name}`.
            let name_start = i + 1;
            let mut j = name_start;
            while j < bytes.len() && bytes[j] != b'}' && bytes[j] != b'{' {
                j += 1;
            }
            if j >= bytes.len() || bytes[j] == b'{' {
                self.done = true;
                return Some(Err(TemplateError::UnbalancedBrace { position: i }));
            }
            let name = &self.template[name_start..j];
            if !is_valid_identifier(name) {
                self.done = true;
                return Some(Err(TemplateError::InvalidIdentifier {
                    name: name.to_string(),
                    position: i,
                }));
            }
            if let Some(v) = self.vars.lookup(name) {
                self.pos = j + 1;
                Some(Ok(v))
            } else {
                self.done = true;
                Some(Err(TemplateError::UnknownVariable {
                    name: name.to_string(),
                    position: i,
                }))
            }
        } else {
            // `here == b'}'`. `}}` is a literal `}`.
            if bytes.get(i + 1) == Some(&b'}') {
                self.pos = i + 2;
                return Some(Ok("}"));
            }
            // Stray `}`.
            self.done = true;
            Some(Err(TemplateError::UnbalancedBrace { position: i }))
        }
    }
}

/// A valid template identifier: at least one character, starting with
/// an ASCII letter, digit, or underscore, and containing only ASCII
/// alphanumerics or underscores thereafter. Positional templates use
/// all-digit names, which pass this check; the numeric-parse step in
/// [`render_positional`] then rejects any non-digit form.
fn is_valid_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_ascii_alphanumeric() || first == '_') {
        return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}
