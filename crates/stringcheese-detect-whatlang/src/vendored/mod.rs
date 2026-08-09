// This is vendored third-party code (whatlang 0.16.4, MIT-licensed —
// see LICENSE-WHATLANG at the crate root). Local lints are silenced
// here so upstream style choices don't force us to fork every file;
// the hard-shard adaptation only touches the trigram-profile layout
// and the crate-path rewrite (`crate::` → `crate::vendored::`).
#![allow(
    unexpected_cfgs,
    unused_imports,
    elided_lifetimes_in_paths,
    dead_code,
    missing_docs,
    clippy::all,
    clippy::pedantic
)]

//! Whatlang is a Rust library to detect(regonize) natural languages.
//! Apart from it, the library also recognizes scripts (writing system).
//! Every language and script are represented by determined list of enums.
//!
//! # Examples
//!
//! Using `detect` function:
//!
//! ```ignore
//! use whatlang::{detect, Lang, Script};
//!
//! let text = "Ĉu vi ne volas eklerni Esperanton? Bonvolu! Estas unu de la plej bonaj aferoj!";
//! let info = detect(text).unwrap();
//! assert_eq!(info.lang(), Lang::Epo);
//! assert_eq!(info.script(), Script::Latin);
//!
//! // Confidence is in the range from 0 to 1.
//! assert_eq!(info.confidence(), 1.0);
//! assert!(info.is_reliable());
//! ```
//!
//! Using `Detector` with specified denylist or allowlist:
//!
//! ```ignore
//! use whatlang::{Detector, Lang};
//!
//! let allowlist = vec![Lang::Eng, Lang::Rus];
//!
//! // You can also create detector using with_denylist function
//! let detector = Detector::with_allowlist(allowlist);
//! let lang = detector.detect_lang("There is no reason not to learn Esperanto.");
//! assert_eq!(lang, Some(Lang::Eng));
//! ```
//!
//! # Features
//!
//! | Feature     | Description                                                                           |
//! |-------------|---------------------------------------------------------------------------------------|
//! | `enum-map`  | `Lang` and `Script` implement `Enum` trait from [enum-map](https://docs.rs/enum-map/) |
//! | `arbitrary` | Support [Arbitrary](https://crates.io/crates/arbitrary)                               |
//! | `serde`     | Implements `Serialize` and `Deserialize` for `Lang` and `Script`                      |
//! | `dev`       | Enables `whatlang::dev` module which provides some internal API.<br/> It exists for profiling purposes and normal users are discouraged to to rely on this API.  |
//!
mod alphabets;
mod combined;
mod core;
mod error;
mod lang;
mod scripts;
mod trigrams;
mod utils;

#[cfg(feature = "dev")]
pub mod dev;

pub use crate::vendored::core::{Detector, Info, detect, detect_lang};
pub use crate::vendored::lang::Lang;
pub use crate::vendored::scripts::{Script, detect_script};
